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
//!
//! (a)/(b) are checked through the kernel high-half alias of the TPA (they
//! verify what `populate_sysvars` wrote); (c) exercises the real BIOS
//! `svc #13` trap with the loaded process's `TTBR0` installed, so the
//! `setexc` peek reads the app's own EL0 low memory.

use core::sync::atomic::Ordering;

use tos_api::bdos::process::arena;
use tos_api::bdos::process::loader::load_to_slot;
use tos_api::bdos::process::mmu::switch_ttbr0_kernel;
use tos_api::bdos::process::PROCESS_TABLE;

use crate::arch::aarch64::user_entry::restore_boot_ttbr0;
use crate::kprintln;

/// Process-table slot the B1 test claims. Runs before any AUTO/desktop
/// `Pexec`, so all slots are free; the test frees it again on completion.
const B1_TEST_SLOT: usize = 0;

// Per-process low-memory layout the loader populates (Option 1). Kept in
// sync with `tos_api::bdos::process::loader::populate_sysvars`.
const SYSBASE_ADDR: usize = 0x4F2; // _sysbase  -> OSHEADER
const P_COOKIES_ADDR: usize = 0x5A0; // _p_cookies -> jar
const OSHEADER_VA: u32 = 0x5C0;
const COOKIE_JAR_VA: u32 = 0x5F0;
const OS_VERSION_OFF: usize = 0x02; // OSHEADER + 0x02
const OS_VERSION: u16 = 0x0404; // Falcon TOS 4.04 (user-pinned)
/// `NTOS` cookie tag — the kernel jar's identity cookie, first mirrored slot.
const NTOS_TAG: u32 = u32::from_be_bytes(*b"NTOS");

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
    0x02, 0x00,              // e_type    = ET_EXEC
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
    let bp = PROCESS_TABLE[slot].basepage_kva.load(Ordering::Acquire);
    if !bp.is_null() {
        // SAFETY: single-CPU pre-test; `bp` owns the slot's TPA block.
        unsafe {
            tos_api::bdos::mem::free_all_owned::free_all_owned(bp.cast());
        }
    }
    tos_api::bdos::process::free_slot(slot);
}
