//! XBIOS screen group (Phase 10): `Physbase` / `Logbase` / `Getrez` /
//! `Setscreen` / `Setpalette` / `Setcolor`. Mirrors the API surface of
//! EmuTOS `bios/screen.c` on top of the host framebuffer
//! ([`crate::host::ScreenInfo`]).
//!
//! Spec: `docs/api/xbios/{02-physbase,03-logbase,04-getrez,05-setscreen,
//! 06-setpalette,07-setcolor}.md`.
//!
//! The logical / physical screen base is kept as a software shadow
//! (EmuTOS' `v_bas_ad`). A shadow base of `0` means "use the live
//! hardware framebuffer". Setting the physical base only updates the
//! shadow — re-pointing the VideoCore scan-out at an arbitrary buffer is
//! a later refinement; software that double-buffers by drawing into its
//! own buffer and reading `Logbase` works.
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

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

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

/// Shadow logical / physical screen base (`0` = use the live FB).
static LOGBASE: AtomicU64 = AtomicU64::new(0);
static PHYSBASE: AtomicU64 = AtomicU64::new(0);

/// The live hardware framebuffer, expressed **in the calling process's
/// address space** — or `0` if no display is up.
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
/// Returns `0` when no display is up, or when the process window cannot take
/// the mapping — the same value the call already produces on a display-less
/// machine, and never an address the caller would fault on.
fn fb_base() -> u64 {
    let Some(info) = host::host().and_then(|h| h.screen()) else {
        return 0;
    };
    match process::slot_for(process::current()) {
        // EL1 caller: the kernel high-half VA *is* the reachable address.
        None => info.base,
        // SAFETY: BDOS lock held (single-CPU SVC context). The framebuffer
        // is a permanently-mapped high-half region that outlives every
        // process, and `size_bytes` is the extent of its pixel store.
        Some(slot) => unsafe {
            usermem::expose_attr(
                slot,
                info.base as *const u8,
                info.size_bytes as usize,
                ProtMode::G,
                usermem::MemAttr::Uncached,
            )
        }
        .unwrap_or(0),
    }
}

/// `Physbase` (0x02) — physical (displayed) screen base.
pub fn physbase() -> i64 {
    match PHYSBASE.load(Ordering::Relaxed) {
        0 => fb_base() as i64,
        p => p as i64,
    }
}

/// `Logbase` (0x03) — logical screen base (where drawing goes).
pub fn logbase() -> i64 {
    match LOGBASE.load(Ordering::Relaxed) {
        0 => fb_base() as i64,
        l => l as i64,
    }
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

/// `Setscreen` (0x05) — set logical/physical base; `rez` is
/// accepted-but-ignored.
/// `x0` = logLoc, `x1` = physLoc (each `> 0` to apply), `x2` = rez,
/// `x3` = videlmode (ignored — no Falcon).
///
/// There is no in-system resolution switch (ADR-017 rev 2: the firmware
/// `config.txt` owns the HDMI mode). The `rez` parameter is therefore
/// accepted for *any* value and ignored — no `REZ` shadow, no out-of-range
/// rejection — mirroring EmuTOS silently ignoring a hardware-unsupported
/// mode (`bios/screen.c:990-1013`). The logical/physical base handling is a
/// separate concern and is unchanged: each base is stored only when `> 0`
/// (`screen.c:1106,1110`). Always returns `0` (E_OK).
pub fn setscreen(args: &TrapArgs) -> i64 {
    let log = args.arg_u64(0);
    let phys = args.arg_u64(1);

    if (log as i64) > 0 {
        LOGBASE.store(log, Ordering::Relaxed);
    }
    if (phys as i64) > 0 {
        PHYSBASE.store(phys, Ordering::Relaxed);
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
