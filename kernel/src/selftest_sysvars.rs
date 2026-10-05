//! B1 — per-process low-memory sysvars + OSHEADER + `$5A0` cookie-jar
//! self-test (Option 1, `docs/analysis/bios-audit/b1-lowmem-feasibility.md`).
//!
//! Loads a minimal EL0 TPA through the real loader (so
//! [`tos_api::bdos::process::loader`]'s `populate_sysvars` runs), then proves
//! the three things a 1:1-ported TOS app depends on:
//!
//!   (a) `*(long*)0x4F2` (`_sysbase`) points into the OSHEADER instance whose
//!       `os_version` (`+0x02`) reads `0x0404` — the exact field whose
//!       absence crashed TeraDesk (`main.c:56`, `gettos.c:41`).
//!   (b) `*(long*)0x5A0` (`_p_cookies`) points at a well-formed cookie jar
//!       (first slot mirrors the kernel jar's `NTOS` cookie).
//!   (c) `Setexc(0x5A0/4 == 0x168, -1)` returns that jar base — the
//!       TeraDesk `library/utility/cookie.c:56` idiom, now EmuTOS-faithful
//!       in [`tos_api::bios::vectors::setexc`].
//!   (f) the mirror stays **live**: a cookie added to the kernel jar through
//!       `Ssystem(S_SETCOOKIE)` after the process was loaded reaches its
//!       `$5A0` array, `S_DELCOOKIE` removes it again, and the classic
//!       `{0, capacity}` terminator survives both.
//!   (e) the memory sysvars an app reads to size the machine agree with the
//!       BDOS pools and with EmuTOS' meaning of each field (BB2,
//!       `docs/analysis/memory-layout-and-reported-free-memory.md` §8.1):
//!       `os_beg`/`os_end` bound the **OS**, not the pool; `end_os`/`membot`
//!       carry the same value at boot (`bios/biosmem.c:66-67`);
//!       `phystop`/`memtop` the pool end; `ramtop`/`ramvalid` the alt-RAM top
//!       and its magic (`bios/memory2.c:115-129`).
//!
//! (a)/(b) are checked through the kernel high-half alias of the TPA (they
//! verify what `populate_sysvars` wrote); (c) exercises the real BIOS
//! `svc #13` trap with the loaded process's `TTBR0` installed, so the
//! `setexc` peek reads the app's own EL0 low memory.


use tos_api::bdos::process::arena;
use tos_api::bdos::process::loader::load_to_slot;
use tos_api::bdos::process::mmu::switch_ttbr0_kernel;

use crate::arch::aarch64::user_entry::restore_boot_ttbr0;
use crate::kprintln;

/// Process-table slot the B1 test claims. Runs before any AUTO/desktop
/// `Pexec`, so all slots are free; the test frees it again on completion.
const B1_TEST_SLOT: usize = 0;

// Per-process low-memory layout the loader populates (Option 1). Kept in
// sync with `tos_api::bdos::process::loader::populate_sysvars`.
const SYSBASE_ADDR: usize = 0x4F2; // _sysbase  -> OSHEADER
const P_COOKIES_ADDR: usize = 0x5A0; // _p_cookies -> jar
const CONTERM_ADDR: usize = 0x484; // conterm (BYTE) — ikbd.c:1100 default 7
const CMDLOAD_ADDR: usize = 0x482; // cmdload (WORD) — ROM-shell default 0
const DUMPFLG_ADDR: usize = 0x4EE; // dumpflg (WORD) — bios.c:357 default -1
const OSHEADER_VA: u32 = 0x5C0;
const COOKIE_JAR_VA: u32 = 0x5F0;
const OS_VERSION_OFF: usize = 0x02; // OSHEADER + 0x02
const OS_VERSION: u16 = 0x0404; // Falcon TOS 4.04 (user-pinned)
// (e) BB2 memory sysvars.
const PHYSTOP_ADDR: usize = 0x42E; // phystop  — top of ST-RAM
const MEMBOT_ADDR: usize = 0x432; //  membot   — bottom of the TPA region
const MEMTOP_ADDR: usize = 0x436; //  memtop   — top of the TPA region
const END_OS_ADDR: usize = 0x4FA; //  end_os   — end of the OS's static ST-RAM
const RAMTOP_ADDR: usize = 0x5A4; //  ramtop   — top of alt-RAM, or NULL
const RAMVALID_ADDR: usize = 0x5A8; // ramvalid — validates ramtop
const OS_BEG_OFF: usize = 0x08; //     OSHEADER + 0x08 — base of the OS
const OS_END_OFF: usize = 0x0C; //     OSHEADER + 0x0C — end of OS ST-RAM
/// `ramvalid` magic (`references/emutos/include/tosvars.h:78`).
const RAMVALID_MAGIC: u32 = 0x1357_BD13;
/// `NTOS` cookie tag — the kernel jar's identity cookie, first mirrored slot.
const NTOS_TAG: u32 = u32::from_be_bytes(*b"NTOS");
/// Scratch tag for (f): set + deleted again inside this test, so the kernel
/// jar is left exactly as it was found.
const B1TS_TAG: u32 = u32::from_be_bytes(*b"B1TS");
/// Value (f) parks under [`B1TS_TAG`].
const B1TS_VAL: u32 = 0x1357_2468;
/// Mirror capacity in `{tag, value}` slots — `loader::COOKIE_JAR_SLOTS`.
const COOKIE_JAR_SLOTS: usize = 16;

// GEMDOS `Ssystem` cookie subfunction modes (FreeMiNT `sys/mint/ssystem.h`).
const S_SETCOOKIE: u64 = 9;
const S_DELCOOKIE: u64 = 26;

// BIOS `Setexc` (0x05) and the `p_cookies` vector number ($5A0 / 4).
const BIOS_SETEXC: u64 = 0x05;
const SETEXC_P_COOKIES_NUM: u64 = 0x5A0 / 4; // == 0x168

// ─── Minimal AArch64 ELF64, image at VA 0x1000 (post-B1 base) ───────────
//
// One PT_LOAD: p_vaddr = 0x1000 (>= RESERVED_LOW_END, so it clears
// `validate_segment`), p_offset = 0x80 (decoupled from vaddr — the parser
// does not require them equal), a single `b .` word. The test never drops
// to EL0 in this image, so the code is only a valid loadable segment.
//
//   0x000..0x040  ELF64 Ehdr   (e_entry = 0x1000)
//   0x040..0x078  1× Phdr      (PT_LOAD, p_vaddr = 0x1000, p_offset = 0x80)
//   0x078..0x080  padding
//   0x080..0x084  code: `b .`  (0x14000000)

/// Hand-crafted minimal AArch64 ELF64, linked at the post-B1 base (0x1000).
#[rustfmt::skip]
static B1_TEST_ELF: [u8; 0x84] = [
    // ─── ELF64 header (64 bytes) ─────────────────────────────────
    0x7F, b'E', b'L', b'F',  // magic
    2,                       // ELFCLASS64
    1,                       // ELFDATA2LSB
    1,                       // EV_CURRENT
    0, 0,                    // OSABI, ABIVERSION
    0, 0, 0, 0, 0, 0, 0,     // padding (7)
    0x03, 0x00,              // e_type    = ET_DYN (PIE; ET_EXEC is refused since ARCH03 0d)
    0xB7, 0x00,              // e_machine = EM_AARCH64
    0x01, 0x00, 0x00, 0x00,  // e_version
    // e_entry = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_phoff = 0x40
    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // e_shoff = 0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,  // e_flags
    0x40, 0x00,              // e_ehsize    = 64
    0x38, 0x00,              // e_phentsize = 56
    0x01, 0x00,              // e_phnum     = 1
    0x40, 0x00,              // e_shentsize = 64 (unused)
    0x00, 0x00,              // e_shnum     = 0
    0x00, 0x00,              // e_shstrndx  = 0

    // ─── Program header (56 bytes at 0x40) ───────────────────────
    0x01, 0x00, 0x00, 0x00,  // p_type  = PT_LOAD
    0x05, 0x00, 0x00, 0x00,  // p_flags = PF_R | PF_X
    // p_offset = 0x80
    0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_vaddr  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_paddr  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_filesz = 4
    0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_memsz  = 4
    0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_align  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // ─── padding 0x78..0x80 (8 bytes) ────────────────────────────
    0, 0, 0, 0, 0, 0, 0, 0,

    // ─── code at 0x80 (4 bytes): b . ─────────────────────────────
    0x00, 0x00, 0x00, 0x14,
];

#[inline]
fn rd_long(page: &[u8; 4096], off: usize) -> u32 {
    u32::from_le_bytes([page[off], page[off + 1], page[off + 2], page[off + 3]])
}
#[inline]
fn rd_u64(page: &[u8; 4096], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&page[off..off + 8]);
    u64::from_le_bytes(b)
}
#[inline]
fn rd_word(page: &[u8; 4096], off: usize) -> u16 {
    u16::from_le_bytes([page[off], page[off + 1]])
}

/// Issue a BIOS call: `svc #13`, function id in `x8`, args in `x0`/`x1`.
#[inline(never)]
fn bios(func: u64, a0: u64, a1: u64) -> i64 {
    let ret: i64;
    // SAFETY: BIOS trap ABI; the handler reads x0/x1/x8 and returns x0.
    unsafe {
        core::arch::asm!(
            "svc #13",
            in("x8") func,
            inout("x0") a0 => ret,
            in("x1") a1,
            options(nostack),
        );
    }
    ret
}

/// Scan a loaded slot's `$5A0` mirror the way a 1:1-ported application does
/// (TeraDesk `library/utility/cookie.c:28-36`): walk `{tag, value}` LONG pairs
/// from the jar base and stop at `tag == 0`. Returns the value of `want`, or
/// `None` if the scan met the terminator (or ran out of window) first.
fn jar_find(slot: usize, want: u32) -> Option<u32> {
    // SAFETY: boot CPU, single-threaded; the slot holds its loaded TPA for
    // the whole self-test.
    let page = unsafe { arena::page_bytes_mut(slot)? };
    for i in 0..COOKIE_JAR_SLOTS {
        let off = COOKIE_JAR_VA as usize + i * 8;
        let tag = rd_long(page, off);
        if tag == 0 {
            return None;
        }
        if tag == want {
            return Some(rd_long(page, off + 4));
        }
    }
    None
}

/// The `value` word of the `{0, capacity}` terminator the scan above stops at,
/// or `None` when the mirror carries no terminator inside its window — which
/// would send an application's scan off the end of the jar.
fn jar_terminator(slot: usize) -> Option<u32> {
    // SAFETY: as in `jar_find`.
    let page = unsafe { arena::page_bytes_mut(slot)? };
    for i in 0..COOKIE_JAR_SLOTS {
        let off = COOKIE_JAR_VA as usize + i * 8;
        if rd_long(page, off) == 0 {
            return Some(rd_long(page, off + 4));
        }
    }
    None
}

/// Issue an `Ssystem` call: `svc #1`, GEMDOS function id `0x154` in `x8`,
/// `mode` in `x0`, `arg1` in `x1`, `arg2` in `x2`.
#[inline(never)]
fn ssystem(mode: u64, arg1: u64, arg2: u64) -> i64 {
    let ret: i64;
    // SAFETY: GEMDOS trap ABI; the handler reads x0/x1/x2/x8 and returns x0.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") 0x154u64,
            inout("x0") mode => ret,
            in("x1") arg1,
            in("x2") arg2,
            options(nostack),
        );
    }
    ret
}

/// Run the B1 sysvars/OSHEADER/cookie-jar self-test.
pub fn run() {
    kprintln!("");
    kprintln!("[b1] === B1 low-memory sysvars + OSHEADER + $5A0 test ===");

    // 1. Load the minimal image; the loader runs `populate_sysvars`.
    // SAFETY: boot CPU, single-threaded; the slot is dedicated to B1.
    let loaded = match unsafe { load_to_slot(&B1_TEST_ELF, B1_TEST_SLOT) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[b1] === FAIL: loader rejected image: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[b1] loaded slot {}: entry_va=0x{:x} l0_pa=0x{:016x}",
        loaded.slot, loaded.entry_va, loaded.l0_pa,
    );

    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[b1]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // 2. Read the populated sysvar area through the kernel high-half alias
    //    of the slot's first TPA page (independent of TTBR0).
    // SAFETY: pre-test, single-CPU; the slot is exclusively ours here.
    let page = match unsafe { arena::page_bytes_mut(loaded.slot) } {
        Some(p) => p,
        None => {
            kprintln!("[b1] === FAIL: no TPA page for slot ===");
            cleanup(loaded.slot);
            return;
        }
    };

    // (a) _sysbase -> OSHEADER, os_version == 0x0404.
    let sysbase = rd_long(page, SYSBASE_ADDR);
    let sysbase64 = rd_u64(page, SYSBASE_ADDR); // TeraDesk's LP64 `ldr x8,[x8]`
    let osver = rd_word(page, OSHEADER_VA as usize + OS_VERSION_OFF);
    check(
        sysbase == OSHEADER_VA,
        "(a) _sysbase@0x4F2 == OSHEADER VA 0x5C0",
    );
    check(
        sysbase64 == OSHEADER_VA as u64,
        "(a) _sysbase 8-byte read == 0x5C0 (LP64 idiom)",
    );
    check(osver == OS_VERSION, "(a) OSHEADER.os_version == 0x0404");

    // (b) _p_cookies -> jar; first mirrored slot is NTOS.
    let p_cookies = rd_long(page, P_COOKIES_ADDR);
    let jar_tag0 = rd_long(page, COOKIE_JAR_VA as usize);
    check(
        p_cookies == COOKIE_JAR_VA,
        "(b) _p_cookies@0x5A0 == jar VA 0x5F0",
    );
    check(
        jar_tag0 == NTOS_TAG,
        "(b) jar[0].tag == NTOS (kernel jar mirrored)",
    );

    // (d) B2 safe-static sysvars written by populate_sysvars: conterm@0x484
    //     == 7 (EmuTOS ikbd.c:1100 — the byte Bconin's conterm & 8 fold
    //     reads), cmdload@0x482 == 0 (ROM-shell default), dumpflg@0x4EE == -1
    //     (bios.c:357). Read through the kernel high-half alias, like (a)/(b).
    let conterm = page[CONTERM_ADDR];
    check(conterm == 7, "(d) conterm@0x484 == 7 (ikbd.c:1100)");
    let cmdload = rd_word(page, CMDLOAD_ADDR);
    check(cmdload == 0, "(d) cmdload@0x482 == 0 (ROM-shell default)");
    let dumpflg = rd_word(page, DUMPFLG_ADDR);
    check(dumpflg == 0xFFFF, "(d) dumpflg@0x4EE == -1 (bios.c:357)");

    // (e) BB2 memory sysvars — each against the live pool extents, so the
    //     test states the *relationship* EmuTOS defines rather than a baked
    //     constant that would drift with every kernel build.
    let (stram_start, stram_end) = tos_api::bdos::mem::init::stram_range();
    let (_ttram_start, ttram_end) = tos_api::bdos::mem::init::ttram_range();
    // os_beg = the OSHEADER's own address (EmuTOS `_os_header`,
    // bios/startup.S:88-89) — the same value `_sysbase` holds.
    check(
        rd_long(page, OSHEADER_VA as usize + OS_BEG_OFF) == OSHEADER_VA,
        "(e) OSHEADER.os_beg == OSHEADER VA (startup.S:88-89)",
    );
    // os_end / end_os / membot = end of the OS's static ST-RAM = pool start
    // (EmuTOS `__endvdibss` / `_end_os_stram`, biosmem.c:66-67).
    check(
        rd_long(page, OSHEADER_VA as usize + OS_END_OFF) == stram_start as u32,
        "(e) OSHEADER.os_end == ST-RAM pool start (biosdefs.h:52)",
    );
    check(
        rd_long(page, END_OS_ADDR) == stram_start as u32,
        "(e) end_os@0x4FA == ST-RAM pool start (biosmem.c:66)",
    );
    check(
        rd_long(page, MEMBOT_ADDR) == stram_start as u32,
        "(e) membot@0x432 == end_os (biosmem.c:67)",
    );
    // phystop / memtop = top of ST-RAM; no video carve-out, so they agree.
    check(
        rd_long(page, MEMTOP_ADDR) == stram_end as u32,
        "(e) memtop@0x436 == ST-RAM pool end (biosmem.c:72)",
    );
    check(
        rd_long(page, PHYSTOP_ADDR) == stram_end as u32,
        "(e) phystop@0x42E == memtop (no screen carve-out)",
    );
    // The classic "free ST-RAM = memtop - os_end" must come out as the pool
    // size — it read 0 before BB2, because both fields held the pool end.
    check(
        rd_long(page, MEMTOP_ADDR).wrapping_sub(rd_long(page, END_OS_ADDR))
            == (stram_end - stram_start) as u32,
        "(e) memtop - end_os == ST-RAM pool size",
    );
    // ramtop = top of alt-RAM (0 == EmuTOS' NULL), ramvalid always the magic.
    check(
        rd_long(page, RAMTOP_ADDR) == ttram_end as u32,
        "(e) ramtop@0x5A4 == TT-RAM pool end (memory2.c:119)",
    );
    check(
        rd_long(page, RAMVALID_ADDR) == RAMVALID_MAGIC,
        "(e) ramvalid@0x5A8 == RAMVALID_MAGIC (memory2.c:124)",
    );

    // (f) The mirror is NOT a load-time snapshot. A cookie added to the
    //     kernel jar through `Ssystem(S_SETCOOKIE)` *after* this process was
    //     loaded must reach its `$5A0` array, and `S_DELCOOKIE` must take it
    //     out again — the gap `docs/api/bios/_cookie-jar.md` §OFFEN records,
    //     and the reason ADR-012's "kernel-owned jar, `$5A0` unused" is not
    //     the whole story once a ported app reads `$5A0` (TeraDesk
    //     `library/utility/cookie.c:28,56`). The trap runs in kernel context
    //     and the mirror is written through the slot's high-half TPA alias,
    //     so this also proves the refresh reaches a process that is *not* the
    //     current one.
    let set_rc = ssystem(S_SETCOOKIE, B1TS_TAG as u64, B1TS_VAL as u64);
    let after_set = jar_find(loaded.slot, B1TS_TAG);
    let del_rc = ssystem(S_DELCOOKIE, B1TS_TAG as u64, 0);
    let after_del = jar_find(loaded.slot, B1TS_TAG);
    let term = jar_terminator(loaded.slot);
    check(set_rc == 0, "(f) Ssystem(S_SETCOOKIE, 'B1TS') == E_OK");
    check(
        after_set == Some(B1TS_VAL),
        "(f) $5A0 mirror carries a cookie set after load",
    );
    check(del_rc == 0, "(f) Ssystem(S_DELCOOKIE, 'B1TS') == E_OK");
    check(
        after_del.is_none(),
        "(f) $5A0 mirror drops a cookie deleted after load",
    );
    check(
        jar_find(loaded.slot, NTOS_TAG) == Some(0x0000_0100),
        "(f) the refresh leaves the seeded NTOS cookie intact",
    );
    check(
        term == Some(COOKIE_JAR_SLOTS as u32),
        "(f) mirror still ends in the {0, capacity} terminator",
    );

    // 3. (c) Setexc(0x5A0/4, -1) returns the jar base. Must run with the
    //    loaded process's TTBR0 installed so the `setexc` peek reads the
    //    app's own $5A0. `switch_ttbr0_kernel` is safe here: we stay wholly
    //    at EL1 (the svc erets EL1->EL1, not to EL0), so the QUIRK-001
    //    TLBI-before-eret-to-EL0 wedge does not apply. No console I/O
    //    between the switch and the restore (low-VA state is the app's).
    // SAFETY: single-CPU; `l0_pa` is the slot's fully-built L0.
    unsafe { switch_ttbr0_kernel(loaded.l0_pa) };
    let setexc_jar = bios(BIOS_SETEXC, SETEXC_P_COOKIES_NUM, (-1i64) as u64);
    // SAFETY: restore the no-process TTBR0 before any further kernel work.
    unsafe { restore_boot_ttbr0() };
    check(
        setexc_jar == COOKIE_JAR_VA as i64,
        "(c) Setexc(0x168, -1) == jar base 0x5F0",
    );

    // 4. Release the slot's TPA + free the slot (the image never ran, so no
    //    Pterm swept it) — same cleanup as the D3 loader test.
    cleanup(loaded.slot);

    if fails == 0 {
        kprintln!("[b1] === B1 PASS ===");
    } else {
        kprintln!("[b1] === B1 FAILED ({}) ===", fails);
    }
}

/// Free the slot's owned TPA and mark the slot free.
fn cleanup(slot: usize) {
    let bp = tos_api::bdos::process::pd_of(slot);
    if !bp.is_null() {
        // SAFETY: single-CPU pre-test; `bp` owns the slot's TPA block.
        unsafe {
            tos_api::bdos::mem::free_all_owned::free_all_owned(bp.cast());
        }
    }
    tos_api::bdos::process::free_slot(slot);
}
