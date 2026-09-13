//! XBIOS screen group (Phase 10): `Physbase` / `Logbase` / `Getrez` /
//! `Setscreen` / `Setpalette` / `Setcolor`. Mirrors the API surface of
//! EmuTOS `bios/screen.c` on top of the host framebuffer
//! ([`crate::host::ScreenInfo`]).
//!
//! Spec: `docs/api/xbios/{02-physbase,03-logbase,04-getrez,05-setscreen,
//! 06-setpalette,07-setcolor}.md`.
//!
//! The logical and physical screen bases are **not** software shadows: they
//! live where EmuTOS keeps them — the logical one in the display's own
//! `v_bas_ad` (our framebuffer's draw pointer, which the VDI backend
//! actually draws through) and the physical one in the hardware, read back
//! from it (`bios/screen.c:939-952,1030,1058-1071`). `Setscreen` moves them
//! for real: the VideoCore's screen memory holds two whole screens and the
//! set-virtual-offset mailbox tag selects which one is scanned out
//! (`docs/hardware/mailbox-framebuffer.md` §3b), so a program can draw into
//! the page it cannot see and then show it.
//!
//! Resolution is **not** a shadow: per ADR-017 rev 2 (config.txt-driven,
//! no in-system switch), `Getrez` reports a fixed honest code and
//! `Setscreen`'s `rez` is accepted-but-ignored — there is no in-system
//! resolution change (the firmware `config.txt` owns the HDMI mode).
//!
//! The palette (`Setpalette` / `Setcolor`) keeps an ST-format shadow for
//! the read-back contract *and* programs the live VDI colour LUT the
//! framebuffer displays through (the host's [`RasterBackend`], the same
//! CLUT the VDI `vs_color` uses), so an XBIOS palette write changes the
//! on-screen colour.
//!
//! [`RasterBackend`]: crate::vdi::backend::RasterBackend

use core::sync::atomic::{AtomicU8, Ordering};

use crate::bdos::process::{self, usermem};
use crate::host;
use crate::trap::TrapArgs;
use tos_types::{Long, ProtMode};

/// Atari resolution code `TT_MEDIUM` (`bios/screen.h`): 640×480, 16
/// colours — the classic code whose colour depth matches NewTOS's native
/// 16-colour framebuffer. `Getrez` reports this as an honest "not
/// monochrome, 16 colours" indicator; the actual geometry is
/// config.txt-driven and read via VDI `DEV_TAB` / `Physbase`, not this
/// coarse code (ADR-017 rev 2).
const TT_MEDIUM: i64 = 4;

/// High-half kernel VA offset — the whole of DRAM is mapped at
/// `phys + VA_OFFSET` for EL1. Mirrored here the way
/// [`crate::bdos::process::arena`], `loader` and `usermem` mirror it.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

/// The whole screen memory, expressed **in the calling process's address
/// space**, plus the caller-space address of `kva` inside it.
///
/// The framebuffer is mapped for EL1 at `phys + VA_OFFSET`, high-half and
/// EL1-only, so returning that pointer to an EL0 program satisfies the letter
/// of `Physbase`'s contract ("a pointer to the screen") and violates its
/// substance: the program faults on the first store. ADR-023 settles the
/// class — what the kernel hands out, the caller must be able to reach — so
/// an EL0 caller gets the framebuffer published into its own window
/// ([`usermem::expose_attr`]) and the **user VA** back. An EL1 caller (the
/// desktop, the boot self-tests, the VDI) gets the kernel VA, which is what
/// it can reach.
///
/// **The whole screen memory is published, not just the visible screen.**
/// The VideoCore buffer holds [`host::ScreenInfo::mem_bytes`] bytes — two
/// stacked screens — and a program that wants to double-buffer has to reach
/// both: it computes the hidden page as `Logbase() + size_bytes` and hands
/// that back to `Setscreen`. Publishing the pair as **one** mapping is what
/// makes that arithmetic valid in the process's own address space.
///
/// Two decisions are attached to that mapping and neither is the program's
/// business:
///
/// * **`PROT_G`, not `PROT_PR`.** `Physbase`/`Logbase` exist so a program can
///   *write* pixels; a read-only screen would be a pointer that still cannot
///   be used for its purpose. `PROT_G` is FreeMiNT's "global — any access
///   OK" mode (`references/freemint/sys/mint/mem.h:130-134`).
/// * **Normal Non-Cacheable** ([`usermem::MemAttr::Uncached`]). The pixels
///   are read out by the VideoCore display engine, a DMA master that scans
///   DRAM from outside the ARM caches
///   (`docs/hardware/mailbox-framebuffer.md` §6). The kernel's own alias is
///   Write-Back and the VDI cleans every drawn rectangle to the point of
///   coherency itself; an EL0 program has no such hook, and requiring one
///   would mean inventing a cache-maintenance call the TOS API does not
///   have. Uncached pages make a plain store land in DRAM — the program
///   writes pixels and they appear, exactly as on the flat machine the API
///   was written for.
///
/// Returns `0` when the process window cannot take the mapping — the same
/// value the call already produces on a display-less machine, and never an
/// address the caller would fault on.
fn caller_view(info: &host::ScreenInfo, kva: u64) -> u64 {
    match process::slot_for(process::current()) {
        // EL1 caller: the kernel high-half VA *is* the reachable address.
        None => kva,
        // SAFETY: BDOS lock held (single-CPU SVC context). The framebuffer
        // is a permanently-mapped high-half region that outlives every
        // process, and `mem_bytes` is the extent of its pixel store.
        Some(slot) => match unsafe {
            usermem::expose_attr(
                slot,
                info.mem_base as *const u8,
                info.mem_bytes as usize,
                ProtMode::G,
                usermem::MemAttr::Uncached,
            )
        } {
            Some(window) => window + (kva - info.mem_base),
            None => 0,
        },
    }
}

/// `Physbase` (0x02) — physical (displayed) screen base.
///
/// Reads the **hardware** back, as EmuTOS' `physbase()` does
/// (`bios/screen.c:1030` → `atari_physbase()`, which reconstructs the address
/// from the video-base registers rather than from a variable), so the value
/// a program gets is where the display engine is really scanning from — not
/// where it was last asked to scan from.
pub fn physbase() -> i64 {
    let Some(info) = host::host().and_then(|h| h.screen()) else {
        return 0;
    };
    caller_view(&info, info.phys_base) as i64
}

/// `Logbase` (0x03) — logical screen base (where drawing goes).
///
/// EmuTOS returns `v_bas_ad` (`bios/screen.c:1071`). Ours lives in the
/// display itself — it is the pointer the VDI raster backend draws through —
/// so this reports the address that is genuinely the drawing target.
pub fn logbase() -> i64 {
    let Some(info) = host::host().and_then(|h| h.screen()) else {
        return 0;
    };
    caller_view(&info, info.base) as i64
}

/// Is a process currently holding a direct, uncached mapping of the
/// framebuffer handed out by [`physbase`] / [`logbase`]?
///
/// The kernel's drawing backend asks once per primitive: while the answer is
/// `true`, the same physical pixels are reachable through two aliases with
/// **mismatched cacheability** — the VDI's Write-Back one and the program's
/// Non-Cacheable one — and the architecture only guarantees coherency if
/// software keeps the region out of the caches. The backend therefore cleans
/// *and invalidates* each drawn rectangle instead of merely cleaning it, so
/// no line of the framebuffer survives a primitive to be written back later
/// over the program's own stores (`kernel/src/vdi_backend.rs`).
pub fn direct_screen_active() -> bool {
    usermem::uncached_shared_active()
}

/// `Getrez` (0x04) — current resolution code.
///
/// NewTOS is a native 16-colour framebuffer, so `Getrez` reports the
/// classic 16-colour code `TT_MEDIUM` (4) as an honest colour-depth
/// indicator (the one thing this coarse code conveys that VDI `DEV_TAB` /
/// `Physbase` don't: colour depth / *not monochrome*). It is **not** a
/// geometry claim — the live geometry is config.txt-driven and read from
/// VDI `DEV_TAB` / `Physbase`, not from this value (ADR-017 rev 2). Revisit
/// at the truecolor VDI step, when a wider-depth code may be more honest.
pub fn getrez() -> i64 {
    TT_MEDIUM
}

/// Translate a screen base as the **caller** wrote it into the kernel
/// high-half VA the display speaks.
///
/// An EL0 program can only have got a screen address from `Physbase` /
/// `Logbase`, i.e. a VA in its own window (ADR-023), possibly plus a whole
/// number of screens — so the process's own block table is the authority
/// ([`usermem::resolve_kva`]). An EL1 caller holds kernel VAs; a value below
/// [`VA_OFFSET`] from EL1 is read as a physical address, which is the other
/// spelling of the same DRAM.
///
/// `None` means "the caller named memory this process has no mapping for",
/// which is neither a screen page nor anything the kernel may dereference.
fn caller_base_to_kva(addr: u64) -> Option<u64> {
    match process::slot_for(process::current()) {
        None if addr >= VA_OFFSET => Some(addr),
        None => Some(addr.wrapping_add(VA_OFFSET)),
        Some(slot) => usermem::resolve_kva(slot, addr),
    }
}

/// `Setscreen` (0x05) — set logical/physical base; `rez` is
/// accepted-but-ignored.
/// `x0` = logLoc, `x1` = physLoc (each `> 0` to apply), `x2` = rez,
/// `x3` = videlmode (ignored — no Falcon).
///
/// Each base is applied only when strictly **positive** — `0` and any
/// negative value mean "leave it alone", the convention EmuTOS spells as
/// `if ((LONG)logLoc > 0)` / `if ((LONG)physLoc > 0)`
/// (`bios/screen.c:1106,1110`).
///
/// **Both moves are real.** `logLoc` becomes the drawing target (EmuTOS
/// `v_bas_ad = logLoc`): the VDI raster backend, the VT52 console and the
/// desktop all draw through the display's draw pointer, so after this call
/// they draw into the named screen. `physLoc` goes to the display engine
/// (EmuTOS `setphys(physLoc)`): on the BCM2711 that is the mailbox
/// set-virtual-offset tag selecting which of the stacked screens the
/// VideoCore scans out (`docs/hardware/mailbox-framebuffer.md` §3b).
///
/// A base the display cannot use — outside its screen memory, or not on a
/// screen-page boundary — leaves that base where it was. The hardware simply
/// has no way to represent it, exactly as a plain ST's video-base register
/// cannot represent the low byte of an address and silently drops it
/// (`atari_setphys`, `bios/screen.c:939-947`); and, as there, the following
/// `Physbase`/`Logbase` reports what the hardware really does, so a program
/// that checks is told the truth rather than its own request echoed back.
///
/// There is no in-system resolution switch (ADR-017 rev 2: the firmware
/// `config.txt` owns the HDMI mode). The `rez` parameter is therefore
/// accepted for *any* value and ignored — no `REZ` shadow, no out-of-range
/// rejection — mirroring EmuTOS silently ignoring a hardware-unsupported
/// mode (`bios/screen.c:990-1013`). Always returns `0` (E_OK).
///
/// # KNOWN DEFECT — the physical base is applied at once, never latched
///
/// **A page flip through this call is not atomic, and no application can
/// work around it.** `physLoc` takes effect the moment it is asked for:
/// `set_screen_physical` reaches `vdi_backend::set_scanout_base`, which
/// reaches `drivers::bcm2711::framebuffer::set_scanout_base` and issues a
/// *synchronous* VideoCore mailbox property write (`set_virtual_offset`,
/// tag `0x0004_8009`). Nothing anywhere holds the new base back until the
/// next vertical blank — there is no pending-base register and no retrace
/// interrupt to install one.
///
/// The consequence is structural rather than intermittent. `Vsync` only
/// reports that the blank has *started*; what remains of it is short, and
/// the mailbox round trip is not bounded inside it. So the scan-out address
/// changes part-way down the visible picture and the frame is shown half
/// from one page and half from the other. Note that this does not depend on
/// the mailbox being slow: **even a free write applied immediately would
/// tear**, because there is no latch at all. The latency only decides how
/// far down the seam falls.
///
/// Observed on real hardware (Raspberry Pi 4, 2026-09-09) with the
/// bouncing-ball demo's `F2` diagnostic build, which holds the ball still
/// and turns only its texture: the changed region is then a fixed rectangle,
/// so the seam stands still and is unmistakable. With the ball travelling
/// the seam moves with the dirty rectangle every frame, which is why it went
/// unremarked. The demo's platform half documents the same finding from the
/// caller's side (the demo left this tree on 2026-09-09; see its
/// `plat-newtos.c`, `plat_present`), and its log
/// no longer claims a whole flip.
///
/// # What a repair needs, and what must be settled first
///
/// The shape is clear: remember the requested base and let the retrace
/// install it, so `Setscreen` queues rather than applies. That is a
/// kernel/XBIOS change — a pending-base slot, a vertical-blank interrupt
/// that writes it, and a decision about what `Physbase` reports between the
/// request and the blank.
///
/// Two things must come from the reference rather than from this symptom,
/// and are deliberately **not** decided here:
///
/// 1. The exact timing contract TOS offers. On Atari hardware the video base
///    registers are read by the shifter once per frame, which is what makes
///    ST double buffering whole; the precise latching rule, and what
///    `Physbase` is required to return while a flip is pending, must be
///    taken from EmuTOS (`bios/screen.c`, `atari_setphys`) and the spec
///    under `docs/api/xbios/05-setscreen.md`, not inferred from this bug.
/// 2. Whether the BCM2711 can latch a scan-out change at the blank at all
///    through the mailbox interface, or whether it needs the VideoCore's own
///    vsync tag. That is a `docs/hardware/mailbox-framebuffer.md` question.
///
/// Nothing about the behaviour below is changed by this note; it records
/// what the call does so the next person starts from the defect rather than
/// from the symptom.
pub fn setscreen(args: &TrapArgs) -> i64 {
    let log = args.arg_u64(0);
    let phys = args.arg_u64(1);
    let Some(host) = host::host() else {
        return 0;
    };

    if (log as i64) > 0 {
        if let Some(kva) = caller_base_to_kva(log) {
            host.set_screen_logical(kva);
        }
    }
    if (phys as i64) > 0 {
        if let Some(kva) = caller_base_to_kva(phys) {
            host.set_screen_physical(kva);
        }
    }
    0
}

/// `Setpalette` (0x06) — load a 16-entry ST palette. Our framebuffer is
/// truecolor (no hardware CLUT), so NewTOS emulates the 16 STe palette
/// registers with the VDI's 16-entry CLUT (`vdi_backend::PALETTE`). Each
/// caller word is kept in the ST-format shadow (contract, read back by
/// `Setcolor(n, -1)`) *and* pushed into that live CLUT so the colour
/// change is displayed. `x0` = palette ptr.
///
/// Register index → CLUT index is identity: NewTOS's VDI `MAP_COL[]` is
/// the identity 16-entry map (see `tos-api/src/vdi/inquiry.rs`), so the
/// STe hardware-register index and the VDI pen coincide. Spec:
/// `docs/api/xbios/06-setpalette.md` (Pi/BCM2711 binding).
pub fn setpalette(args: &TrapArgs) -> i64 {
    let ptr = args.arg_u64(0);
    if ptr == 0 {
        return 0;
    }
    // SAFETY: caller-supplied 16-WORD palette (kernel pointer at EL1).
    let p = ptr as *const u16;
    let pal = palette();
    for (i, slot) in pal.iter_mut().enumerate() {
        let word = unsafe { p.add(i).read() } & 0x0FFF; // STe: 0x0RGB, 4 bits/gun
        *slot = word;
        apply_to_clut(i as u16, word);
    }
    0
}

/// `Setcolor` (0x07) — set or query one palette entry. `x0` = index
/// (0..15), `x1` = ST color value, or `-1` to query. Returns the old
/// value. A write updates both the ST-format shadow (contract) and the
/// live VDI CLUT (`vdi_backend::PALETTE`), so the colour changes on
/// screen; a query (`value < 0`) leaves both untouched. Spec:
/// `docs/api/xbios/07-setcolor.md` (Pi/BCM2711 binding).
pub fn setcolor(args: &TrapArgs) -> i64 {
    let index = (args.arg_u16(0) & 0x0F) as usize;
    let value = args.arg_i16(1);
    let pal = palette();
    let old = pal[index] as Long as i64;
    if value >= 0 {
        let word = (value as u16) & 0x0FFF;
        pal[index] = word;
        apply_to_clut(index as u16, word);
    }
    old
}

/// Push one ST/STe palette word into the live VDI colour LUT the backend
/// displays through, so an XBIOS palette write actually changes the
/// on-screen colour (the same CLUT `vs_color` programs, via the host's
/// `RasterBackend::set_color`).
///
/// The STe packed word is `0x0RGB` — bits 11-8 red, 7-4 green, 3-0 blue,
/// 4 significant bits per gun (`docs/api/xbios/07-setcolor.md` §"Packed
/// colour word"). Each nibble (0..=15) expands to the VDI's 0..=1000 gun
/// range by `n * 1000 / 15` (full-range: nibble `0xF` → `1000`); the
/// backend then does the 0..1000 → 8-bit framebuffer expansion itself
/// (`kernel/src/vdi_backend.rs`), matching the spec's "4→8-bit" note.
fn apply_to_clut(index: u16, word: u16) {
    let Some(raster) = host::host().and_then(|h| h.raster()) else {
        return; // No display up (hostless / display-less): shadow only.
    };
    let gun = |nibble: u16| nibble * 1000 / 15;
    let r = gun((word >> 8) & 0x0F);
    let g = gun((word >> 4) & 0x0F);
    let b = gun(word & 0x0F);
    raster.set_color(index, r, g, b);
}

// ── Cursconf (VT52 text-cursor config) ───────────────────────────────────
//
// `v_stat_0` flags (EmuTOS `vt52.h`): the cursor visible / flash-enable
// bits. NewTOS' GUI-oriented framebuffer console has no *blinking* text
// caret, so hide/show/flash track their state without a visible cursor —
// but the blink-rate get/set are honoured (a real, usable value), so this
// is a thin state model, not a stub.
const M_CFLASH: u8 = 0x02;
const M_CVIS: u8 = 0x10;
/// `v_stat_0` cursor bits; default = visible + flashing (EmuTOS).
static V_STAT_0: AtomicU8 = AtomicU8::new(M_CVIS | M_CFLASH);
/// `v_period` — cursor blink period in VBLs (EmuTOS default 30).
static V_PERIOD: AtomicU8 = AtomicU8::new(30);

/// `Cursconf` (0x15) — configure the VT52 text cursor (EmuTOS `cursconf`):
/// `function` 0 hide, 1 show, 2 disable flash, 3 enable flash, 4 set the
/// blink period (`operand` low byte, VBLs), 5 return the period; ≥ 6 no-op.
pub fn cursconf(args: &TrapArgs) -> i64 {
    let function = args.arg_i16(0);
    let operand = args.arg_i16(1);
    match function {
        0 => {
            V_STAT_0.fetch_and(!M_CVIS, Ordering::Relaxed);
        }
        1 => {
            V_STAT_0.fetch_or(M_CVIS, Ordering::Relaxed);
        }
        2 => {
            V_STAT_0.fetch_and(!M_CFLASH, Ordering::Relaxed);
        }
        3 => {
            V_STAT_0.fetch_or(M_CFLASH, Ordering::Relaxed);
        }
        4 => V_PERIOD.store(operand as u8, Ordering::Relaxed),
        5 => return V_PERIOD.load(Ordering::Relaxed) as i64,
        _ => {}
    }
    0
}

/// 16-entry ST palette shadow (0x0RGB, 3 bits per gun).
fn palette() -> &'static mut [u16; 16] {
    use core::cell::UnsafeCell;
    struct Pal(UnsafeCell<[u16; 16]>);
    // SAFETY: single-CPU, IRQ-off trap path.
    unsafe impl Sync for Pal {}
    static PAL: Pal = Pal(UnsafeCell::new([0u16; 16]));
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut *PAL.0.get() }
}
