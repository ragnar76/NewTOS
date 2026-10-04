//! Phase 06 D3 — `Pexec(PE_LOADGO)` ELF-loader self-test.
//!
//! Validates the **entire load path** from a memory-resident ELF
//! blob to a running EL0 program:
//!
//! 1. The blob below is a hand-crafted minimal AArch64 ELF64
//!    executable containing two instructions at file offset
//!    `0x100`:
//!
//!    ```text
//!      svc #0xEC4    ; trap to kernel — handler redirects via the
//!                    ; same Phase-05 EC4 mechanism, since Pterm is
//!                    ; D6 and not yet implemented
//!      b .           ; safety net
//!    ```
//!
//! 2. [`tos_api::bdos::process::loader::load_to_slot`] parses the
//!    ELF, copies the segment into [`D3_TEST_SLOT`]'s static user
//!    page, initialises the slot's page tables, and maps
//!    `USER_VA_BASE` to the page with [`PageFlags::UserRwExec`].
//!
//! 3. The driver claims the slot via the same SP-juggling /
//!    EL1-resume-PC capture as the EC4 spike, then `eret`s to
//!    [`LoadedImage::entry_va`] (= `e_entry` from the ELF header).
//!
//! 4. The user `svc #0xEC4` fires; the existing EC4 sentinel
//!    handler redirects to the captured EL1 resume PC.
//!
//! 5. The driver validates that:
//!    - `LoadedImage::entry_va` was the ELF's `e_entry` (`0x1000`).
//!    - The handler reported `SP_EL0 == USER_STACK_TOP_EXCLUSIVE`
//!      (D6 wiring still works post-D3).
//!    - The handler reported `ELR_EL1 == entry_va + 4` (the next
//!      instruction after the SVC at the user PC).
//!    - The handler reported `SPSR.M == 0b0000` (trap came from
//!      EL0, proving the eret to EL0 actually crossed the EL
//!      boundary).
//!
//! ## Why hand-crafted ELF bytes?
//!
//! Building a real ELF would require either an additional build
//! step (`build.rs` running an AArch64 cross-assembler) or a
//! committed pre-built artefact. For the D3 self-test the goal is
//! to **exercise the parser**, so hand-crafting the smallest
//! possible valid ELF is both more direct and more obviously
//! self-contained than depending on an external toolchain.
//!
//! The blob is annotated below in detail; every byte traces back
//! to the ELF gABI § "ELF Header" / "Program Header" tables.
//!
//! ## Phase-06 D6 hand-off
//!
//! Once `Pterm` exists, the user's exit path becomes
//! `svc #1, x8 = 0x4C` (Pterm) instead of `svc #0xEC4`. The Pterm
//! handler will free the slot's allocations, switch back to the
//! parent's basepage / TTBR0, and resume EL1 at the parent's
//! saved PC — replacing the EC4 redirect that this D3 test
//! piggy-backs on.

use core::sync::atomic::Ordering;

use tos_api::bdos::process::arena;
use tos_api::bdos::process::loader::{load_to_slot, LoadedImage};
use tos_api::bdos::process::PROCESS_TABLE;
use tos_types::{BasePage, PF_STANDARD};

use crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
use crate::kprintln;
use crate::selftest_ec4::{
    EC4_CAPTURED_ELR, EC4_CAPTURED_SP_EL0, EC4_CAPTURED_SPSR_M,
    EC4_SENTINEL_SLOT, EC4_TEST_RUNNING,
};

/// Process-table slot the D3 test claims for its mapping. Slot 1 so
/// it doesn't interfere with EC4's slot 0 (and so the per-slot table
/// arena's slot-0 contents from EC4 stay untouched as a regression
/// canary).
pub const D3_TEST_SLOT: usize = 1;

// ─── Hand-crafted minimal AArch64 ELF64 ─────────────────────────────
//
// File layout (264 bytes total):
//
//   0x000..0x040 : ELF64 Ehdr  (64 bytes)
//   0x040..0x078 : 1× ELF64 Phdr (PT_LOAD, 56 bytes)
//   0x078..0x100 : padding (zero)
//   0x100..0x108 : code (8 bytes — `svc #0xEC4` + `b .`)
//
// Encodings:
//   svc #0xEC4 : 0xD400_0001 | (0xEC4 << 5) = 0xD401_D881
//                little-endian: 81 d8 01 d4
//   b .        : 0x1400_0000
//                little-endian: 00 00 00 14

/// Hand-crafted minimal AArch64 ELF64 executable. Loaded by the D3
/// self-test via [`load_to_slot`].
#[rustfmt::skip]
pub static D3_TEST_ELF: [u8; 264] = [
    // ─── ELF64 header (Ehdr, 64 bytes) ───────────────────────────
    0x7F, b'E', b'L', b'F',  // e_ident[EI_MAG0..3]: ELF magic
    2,                       // e_ident[EI_CLASS]: ELFCLASS64
    1,                       // e_ident[EI_DATA]: ELFDATA2LSB
    1,                       // e_ident[EI_VERSION]: EV_CURRENT
    0,                       // e_ident[EI_OSABI]: SYSV
    0,                       // e_ident[EI_ABIVERSION]: 0
    0, 0, 0, 0, 0, 0, 0,     // e_ident padding (7 bytes)

    0x03, 0x00,              // e_type:    ET_DYN (PIE; ET_EXEC is refused since ARCH03 0d)
    0xB7, 0x00,              // e_machine: EM_AARCH64
    0x01, 0x00, 0x00, 0x00,  // e_version: EV_CURRENT

    // e_entry = 0x1000 (entry point — VA of the SVC instruction; B1's linker
    // bump requires segments at/above RESERVED_LOW_END = 0x1000).
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // e_phoff = 0x40 (program header table immediately follows Ehdr)
    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // e_shoff = 0 (no section headers — we don't need them)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    0x00, 0x00, 0x00, 0x00,  // e_flags
    0x40, 0x00,              // e_ehsize:    64
    0x38, 0x00,              // e_phentsize: 56
    0x01, 0x00,              // e_phnum:     1
    0x40, 0x00,              // e_shentsize: 64 (unused)
    0x00, 0x00,              // e_shnum:     0
    0x00, 0x00,              // e_shstrndx:  0

    // ─── Program header (Phdr, 56 bytes at offset 0x40) ──────────
    0x01, 0x00, 0x00, 0x00,  // p_type:  PT_LOAD
    0x05, 0x00, 0x00, 0x00,  // p_flags: PF_R | PF_X (= 4 | 1 = 5)

    // p_offset = 0x100 (file offset where segment data starts)
    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_vaddr  = 0x1000 (target VA; ≥ RESERVED_LOW_END so validate_segment OKs it)
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_paddr  = 0x1000 (we don't differentiate)
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_filesz = 8 bytes of code
    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_memsz  = 8 bytes (no BSS)
    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_align  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // ─── Padding from 0x78 to 0x100 (136 bytes = 4 × 32 + 8) ─────
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,

    // ─── Code at file offset 0x100 (8 bytes) ─────────────────────
    0x81, 0xD8, 0x01, 0xD4,  // svc  #0xEC4
    0x00, 0x00, 0x00, 0x14,  // b    .
];

/// Run the D3 ELF-load + EL0-entry self-test.
pub fn run_d3_elf_loader_test() {
    kprintln!("");
    kprintln!("[d3] === Phase 06 D3 ELF loader test ===");

    // 1. Parse + load.
    // SAFETY: we are the boot CPU, no other observer of the slot or
    // the loader statics; the BDOS giant lock is implicit.
    let loaded = match unsafe { load_to_slot(&D3_TEST_ELF, D3_TEST_SLOT) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[d3] === FAIL: loader rejected blob: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[d3] loaded slot {}: entry_va=0x{:016x} l0_pa=0x{:016x}",
        loaded.slot, loaded.entry_va, loaded.l0_pa,
    );

    // Sanity: the parser must have read e_entry = 0x1000.
    if loaded.entry_va != 0x1000 {
        kprintln!(
            "[d3] === FAIL: entry_va=0x{:x} expected 0x1000 ===",
            loaded.entry_va,
        );
        return;
    }

    // 2. Drop to EL0 at the loaded entry. Reuses EC4's
    // SP-juggling + sentinel-redirect mechanism (Pterm is D6).
    kprintln!("[d3] eret → EL0 at entry_va …");
    // SAFETY: pre-test, single-CPU, EC4 sentinel handler is the
    // pre-arranged return path; the same SP juggling rules apply
    // as in `selftest_ec4::drop_to_el0_and_back`.
    unsafe { drop_to_el0_via_loaded(&loaded) };
    kprintln!("[d3] returned from EL0 to EL1 resume PC");

    // Flush the slot's TTBR0 so any subsequent test (D6 Pterm)
    // starts with an empty EL0 TLB. `install_ttbr0` deliberately
    // skips the TLBI on the eret path (QEMU cortex-a72 wedges); we
    // do it here, between tests, where the eret-adjacency hazard
    // doesn't apply.
    // SAFETY: post-test cleanup, single-CPU, no kernel code holds
    // a low-half VA reference across this point.
    unsafe { crate::arch::aarch64::user_entry::restore_boot_ttbr0() };

    // 3. Validate the captured trap state.
    let sp_el0 = EC4_CAPTURED_SP_EL0.load(Ordering::Acquire);
    let elr = EC4_CAPTURED_ELR.load(Ordering::Acquire);
    let spsr_m = EC4_CAPTURED_SPSR_M.load(Ordering::Acquire);

    let sp_ok = sp_el0 == USER_STACK_TOP_EXCLUSIVE;
    let elr_ok = elr == loaded.entry_va + 4;
    let from_el0 = spsr_m == 0;
    let running_cleared = !EC4_TEST_RUNNING.load(Ordering::Acquire);

    kprintln!(
        "[d3]   captured SP_EL0 = 0x{:016x}  (expected 0x{:016x})  {}",
        sp_el0, USER_STACK_TOP_EXCLUSIVE,
        if sp_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   captured ELR    = 0x{:016x}  (expected 0x{:016x})  {}",
        elr, loaded.entry_va + 4,
        if elr_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   SPSR.M (saved)  = 0x{:x}  (expected 0x0 = EL0t)  {}",
        spsr_m,
        if from_el0 { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   running flag cleared by handler:  {}",
        if running_cleared { "OK" } else { "FAIL" },
    );

    // 4. Validate the populated basepage. Read directly from the
    //    slot's user page via its high-half kernel VA — we don't go
    //    through TTBR0 because (a) we just `eret`'d back from EL0
    //    and the kernel's TTBR0 still points at the slot, but more
    //    importantly (b) kernel statics live in the TTBR1 regime,
    //    so this read is independent of the slot's TTBR0 install.
    let bp_ok = validate_basepage(&loaded);

    // The D3 user code exits via the EC4 sentinel (not `Pterm`), so nothing
    // has run `free_all_owned` for this slot. Release its pool-backed TPA
    // (owned by the basepage cookie) and free the slot so the loader test
    // leaves no leaked TT-RAM behind.
    let bp = PROCESS_TABLE[loaded.slot]
        .basepage_kva
        .load(Ordering::Acquire);
    if !bp.is_null() {
        // SAFETY: single-CPU pre-test; `bp` owns the slot's TPA.
        unsafe {
            tos_api::bdos::mem::free_all_owned::free_all_owned(bp.cast());
        }
    }
    tos_api::bdos::process::free_slot(loaded.slot);

    if sp_ok && elr_ok && from_el0 && running_cleared && bp_ok {
        kprintln!("[d3] === D3 PASS ===");
    } else {
        kprintln!("[d3] === D3 FAIL ===");
    }
}

/// Read the basepage written by [`load_to_slot`] back from the
/// slot's user page and verify every loader-populated field. Mirror
/// of the D4 expectations: the test ELF has a single 8-byte text
/// segment at vaddr=0x1000, no data, no BSS, so the data/BSS slots
/// must be zero.
fn validate_basepage(loaded: &LoadedImage) -> bool {
    // SAFETY: pre-test, single-CPU; the slot is exclusively owned
    // by this test for the duration of the function and not touched
    // by any other code path.
    let page = match unsafe { arena::page_bytes_mut(loaded.slot) } {
        Some(p) => p,
        None => {
            kprintln!("[d3]   basepage:                 FAIL (no page)");
            return false;
        }
    };
    // SAFETY: the loader wrote a `BasePage` at offset 0 of the page.
    // We read it back as `&BasePage` while no other code holds a
    // borrow on the page (the user has finished executing and we're
    // single-CPU).
    let bp: &BasePage = unsafe { &*(page.as_ptr() as *const BasePage) };

    // p_hitpa is now the multi-page TPA top (the loader hands each
    // process a pool-backed TPA of `arena::TPA_SIZE`), not the old
    // single 4-KiB page top. Derive it from the loader so the two stay
    // in sync if the TPA size changes.
    let expected_hitpa: i32 =
        tos_api::bdos::process::loader::USER_TPA_TOP as i32;
    let expected_xdta: i32 = 0x80;
    let expected_uft = [-1i8; 6];

    let p_lowtpa = bp.p_lowtpa;
    let p_hitpa = bp.p_hitpa;
    let p_tbase = bp.p_tbase;
    let p_tlen = bp.p_tlen;
    let p_dbase = bp.p_dbase;
    let p_dlen = bp.p_dlen;
    let p_bbase = bp.p_bbase;
    let p_blen = bp.p_blen;
    let p_xdta = bp.p_xdta;
    let p_flags = bp.p_flags;
    let p_uft = bp.p_uft;

    let lowtpa_ok = p_lowtpa == 0;
    let hitpa_ok = p_hitpa == expected_hitpa;
    let tbase_ok = p_tbase == 0x1000;
    let tlen_ok = p_tlen == 8;
    let dbase_ok = p_dbase == 0;
    let dlen_ok = p_dlen == 0;
    let bbase_ok = p_bbase == 0;
    let blen_ok = p_blen == 0;
    let xdta_ok = p_xdta == expected_xdta;
    let flags_ok = p_flags == PF_STANDARD;
    let uft_ok = p_uft == expected_uft;

    kprintln!(
        "[d3]   bp.p_lowtpa = 0x{:08x} p_hitpa  = 0x{:08x}  {}",
        p_lowtpa, p_hitpa,
        if lowtpa_ok && hitpa_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   bp.p_tbase  = 0x{:08x} p_tlen   = 0x{:08x}  {}",
        p_tbase, p_tlen,
        if tbase_ok && tlen_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   bp.p_dbase  = 0x{:08x} p_dlen   = 0x{:08x}  {}",
        p_dbase, p_dlen,
        if dbase_ok && dlen_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   bp.p_bbase  = 0x{:08x} p_blen   = 0x{:08x}  {}",
        p_bbase, p_blen,
        if bbase_ok && blen_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   bp.p_xdta   = 0x{:08x} p_flags  = 0x{:08x}  {}",
        p_xdta, p_flags,
        if xdta_ok && flags_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[d3]   bp.p_uft    = [-1; 6]                        {}",
        if uft_ok { "OK" } else { "FAIL" },
    );

    lowtpa_ok && hitpa_ok && tbase_ok && tlen_ok && dbase_ok && dlen_ok
        && bbase_ok && blen_ok && xdta_ok && flags_ok && uft_ok
}

/// Drop to EL0 at `loaded.entry_va`, with `loaded.l0_pa` installed
/// in TTBR0_EL1. Wraps the shared
/// [`crate::arch::aarch64::user_entry::enter_el0_with_resume_capture`]
/// helper that EC4 also uses. The EC4 sentinel handler drives the
/// return path; Phase 06 D6 (Pterm) will introduce a parent-resume
/// mechanism that replaces the sentinel for production callers.
unsafe fn drop_to_el0_via_loaded(img: &LoadedImage) {
    // The D3 user code exits via `svc #0xEC4`; arm the sentinel
    // handler just like `selftest_ec4` does, and point it at our
    // own slot so the redirect reads the resume PC / kernel SP we
    // are about to capture (EC4 spike's default points at slot 0).
    EC4_SENTINEL_SLOT.store(img.slot, Ordering::Release);
    EC4_TEST_RUNNING.store(true, Ordering::Release);

    let parent_frame = &PROCESS_TABLE[img.slot].parent_frame;

    // SAFETY: pre-test, single-CPU; PROCESS_TABLE is a static and
    // its parent_frame atomics outlive the round-trip.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            img.entry_va,
            img.basepage_va,
            img.l0_pa,
            USER_STACK_TOP_EXCLUSIVE,
            parent_frame,
        );
    }
}
