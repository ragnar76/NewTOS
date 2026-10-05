//! Cooperative-kernel building block 1 — generic EL0→EL1 upcall / signal
//! delivery, end-to-end self-test.
//!
//! Spec: `docs/analysis/coop-kernel/10-upcall-signal-design.md` (§4 trampoline,
//! §5 Psigreturn, §6 saved context + nesting, §7 delivery point, §10.2 step 1).
//!
//! There is **no real consumer yet** (etv_*, vex, G_USERDEF, eralert land
//! later); this proves the mechanism in isolation, exactly as the design's
//! implementation-order step 1 prescribes. It is gated behind `pub mod` and is
//! **not** wired into normal boot (mirrors [`crate::selftest_sysvars`]).
//!
//! ## What it exercises
//!
//! It loads a tiny EL0 TPA through the real loader (so
//! [`tos_api::bdos::process::loader`]'s trampoline writer runs at
//! [`tos_api::bdos::process::loader::TRAMPOLINE_VA`] = 0x700), pre-posts an
//! upcall to the process ([`tos_api::bdos::process::post_upcall`]), and drops
//! to EL0. The EL0 program does a GEMDOS `Sversion` syscall; the SVC handler's
//! delivery check ([`crate::arch::aarch64::exceptions`], design §7.1) fires the
//! pending upcall, redirecting into a tiny EL0 handler. The handler stores a
//! marker (proving it ran at EL0 with the correct arguments) and `ret`s into
//! the 0x700 trampoline, which calls `Psigreturn` (GEMDOS 0x11A). The kernel
//! restores the interrupted context; the EL0 code resumes right after its
//! `Sversion`, saves the (intact) `x0`, and traps back to EL1.
//!
//! ## The EL0 program (loaded at VA 0x1000, one PT_LOAD)
//!
//! ```text
//!   entry (0x1000):
//!     0x1000  mov  x8, #0x30      ; GEMDOS Sversion
//!     0x1004  svc  #1             ; original syscall — upcall delivered on return
//!     ; --- Psigreturn resumes here, x0 = Sversion result ---
//!     0x1008  mov  x9, #0x910     ; RESUMED_X0_ADDR
//!     0x100C  str  x0, [x9]       ; save the resumed x0 for the kernel to verify
//!     0x1010  svc  #0xEC4         ; sentinel — redirect back to EL1 (resume capture)
//!     0x1014  b    .              ; safety net
//!   handler (0x1020):
//!     0x1020  str  x1, [x0]       ; *(args[0]) = args[1]  (flag + arg check in one)
//!     0x1024  ret                 ; -> LR = TRAMPOLINE_VA (0x700)
//! ```
//!
//! The return-to-EL1 reuses the shared EC4 sentinel path
//! ([`crate::selftest_ec4`]'s `svc #0xEC4` + resume-capture in
//! [`crate::arch::aarch64::user_entry::enter_el0_with_resume_capture`]) — the
//! generic "an EL0 trap redirects to the captured EL1 resume PC" mechanism,
//! independent of the upcall machinery under test.

use core::sync::atomic::Ordering;

use tos_api::bdos::process::arena;
use tos_api::bdos::process::loader::load_to_slot;
use tos_api::bdos::process::{
    post_upcall, set_current, Disposition, State, UpcallDesc, PROCESS_TABLE,
};

use crate::arch::aarch64::user_entry::{
    enter_el0_with_resume_capture, restore_boot_ttbr0,
};
use crate::kprintln;
use crate::selftest_ec4::{
    EC4_CAPTURED_ELR, EC4_CAPTURED_SPSR_M, EC4_SENTINEL_SLOT, EC4_TEST_RUNNING,
};

/// Process-table slot the test claims. Runs before any AUTO/desktop `Pexec`,
/// so all slots are free; the test frees it again on completion. Slot 0 is
/// also the EC4 sentinel default ([`crate::selftest_ec4::EC4_TEST_SLOT`]), so
/// the shared sentinel resume-capture reads this slot's `parent_frame`.
const SLOT: usize = 0;

// EL0 program link VAs (must match the ELF below; entry = `e_entry` 0x1000).
// The image runs relocated at its TPA block, so the handler executes at
// `block + HANDLER_VA` (ARCH03 stage 1).
const HANDLER_VA: u64 = 0x1020;
/// ELR captured at the `svc #0xEC4` sentinel = the instruction after it
/// (`b .` at 0x1014). Reaching it proves the interrupted EL0 code resumed
/// through the trampoline + `Psigreturn` and ran to the sentinel.
const SENTINEL_RESUME_ELR: u64 = 0x1014;

// TPA data cells the EL0 code writes, both in the reserved low gap
// `[0x670, 0x1000)` (zeroed at load, mapped `UserRwExec`, disjoint from the
// 0x700 trampoline). Read back by the kernel via the high-half TPA alias.
const FLAG_ADDR: u64 = 0x900;
const RESUMED_X0_ADDR: u64 = 0x910;

/// Marker the handler stores at [`FLAG_ADDR`] (passed as `args[1]`). Any
/// nonzero 64-bit pattern; a match proves the handler ran at EL0 *and*
/// received the correct arguments.
const FLAG_MARKER: u64 = 0x5555_AAAA_DEAD_BEEF;

/// The mask bit the upcall carries (design: `UpcallDesc.mask_bit`). Bit 0 is
/// arbitrary — the test only checks it is set on delivery and cleared again on
/// `Psigreturn` (via `upcall_mask` returning to 0).
const MASK_BIT: u32 = 0x1;

/// GEMDOS `Sversion` (0x30) return value NewTOS reports (Falcon TOS 4.0x),
/// kept in sync with `tos_api::gemdos::version::GEMDOS_VERSION`. The original
/// syscall's `x0`; it must survive the upcall round-trip intact (design §7.2).
const SVERSION_RESULT: u64 = 0x0000_3000;

// ─── Minimal AArch64 ELF64, image at VA 0x1000 ──────────────────────────
//
// One PT_LOAD: p_vaddr = 0x1000 (>= RESERVED_LOW_END), p_offset = 0x80,
// p_filesz = p_memsz = 0x28. Code bytes were assembled + verified with
// `aarch64-elf-as` (see the module-level disassembly).

#[rustfmt::skip]
static UPCALL_TEST_ELF: [u8; 0xA8] = [
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
    // p_filesz = 0x28
    0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_memsz  = 0x28
    0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // p_align  = 0x1000
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

    // ─── padding 0x78..0x80 (8 bytes) ────────────────────────────
    0, 0, 0, 0, 0, 0, 0, 0,

    // ─── entry code at 0x80 (VA 0x1000) ──────────────────────────
    0x08, 0x06, 0x80, 0xD2,  // mov x8, #0x30
    0x21, 0x00, 0x00, 0xD4,  // svc #1
    0x09, 0x22, 0x81, 0xD2,  // mov x9, #0x910
    0x20, 0x01, 0x00, 0xF9,  // str x0, [x9]
    0x81, 0xD8, 0x01, 0xD4,  // svc #0xEC4
    0x00, 0x00, 0x00, 0x14,  // b .
    0x00, 0x00, 0x00, 0x00,  // padding (VA 0x1018)
    0x00, 0x00, 0x00, 0x00,  // padding (VA 0x101C)

    // ─── handler at 0xA0 (VA 0x1020) ─────────────────────────────
    0x01, 0x00, 0x00, 0xF9,  // str x1, [x0]
    0xC0, 0x03, 0x5F, 0xD6,  // ret
];

#[inline]
fn rd_u64(page: &[u8; 4096], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&page[off..off + 8]);
    u64::from_le_bytes(b)
}

/// Run the end-to-end EL0 upcall self-test.
pub fn run() {
    kprintln!("");
    kprintln!("[upcall] === EL0 upcall / signal-delivery test (building block 1) ===");

    // 1. Load the tiny image through the real loader — this also writes the
    //    0x700 trampoline (loader `write_upcall_trampoline`).
    // SAFETY: boot CPU, single-threaded; the slot is dedicated to this test.
    let loaded = match unsafe { load_to_slot(&UPCALL_TEST_ELF, SLOT) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[upcall] === FAIL: loader rejected image: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[upcall] loaded slot {}: entry_va=0x{:x} l0_pa=0x{:016x} sp_el0=0x{:x}",
        loaded.slot, loaded.entry_va, loaded.l0_pa, loaded.sp_el0,
    );

    // 2. Make this process the running one so the SVC handler's delivery check
    //    (`slot_for(current())`) targets it, then pre-post the upcall.
    // `load_to_slot` publishes `basepage_kva` but leaves the slot `Free`
    // (it loads into a caller-chosen slot, bypassing `allocate_slot`). Mark
    // it `Running` so the SVC handler's `slot_for(current())` matches it —
    // the process really is the running one during the EL0 excursion.
    PROCESS_TABLE[SLOT].set_state(State::Running);
    set_current(tos_api::bdos::process::pd_of(SLOT));
    // The image is relocated for its TPA block (ARCH03 stage 1): a link VA
    // `v` executes at `block + v`.
    let handler_va = loaded.basepage_va + HANDLER_VA;
    post_upcall(
        SLOT,
        UpcallDesc {
            handler_va,
            args: [FLAG_ADDR, FLAG_MARKER, 0, 0],
            mask_bit: MASK_BIT,
            // Step-1 behaviour: resume the interrupted context after the
            // handler (cooperative-kernel step 2 disposition).
            disposition: Disposition::Resume,
        },
    );
    kprintln!(
        "[upcall] posted upcall: handler_va=0x{:x} args=[0x{:x},0x{:x},0,0] mask=0x{:x}",
        handler_va, FLAG_ADDR, FLAG_MARKER, MASK_BIT,
    );

    // 3. Arm the shared EC4 sentinel resume-capture (our return-to-EL1 path),
    //    then drop to EL0. The handler redirects back here when the EL0 code
    //    finally executes `svc #0xEC4`.
    EC4_SENTINEL_SLOT.store(SLOT, Ordering::Release);
    EC4_TEST_RUNNING.store(true, Ordering::Release);
    let parent_frame = &PROCESS_TABLE[SLOT].parent_frame;

    kprintln!("[upcall] eret -> EL0 (Sversion; upcall delivered on return) …");
    // SAFETY: the loaded slot's L0 is fully built; single-CPU; the parent_frame
    // atomics live in the static PROCESS_TABLE and outlive the round-trip.
    unsafe {
        enter_el0_with_resume_capture(
            loaded.entry_va,
            loaded.basepage_va,
            loaded.l0_pa,
            loaded.sp_el0,
            parent_frame,
        );
    }
    // SAFETY: post-excursion; restore the no-process TTBR0 before more kernel
    // work touches low-half VAs.
    unsafe { restore_boot_ttbr0() };
    kprintln!("[upcall] returned from EL0; TTBR0 restored");

    // 4. Read back what EL0 wrote, via the kernel high-half TPA alias
    //    (independent of TTBR0), plus the sentinel capture + slot bookkeeping.
    // SAFETY: single-CPU; the slot is exclusively ours here.
    let page = match unsafe { arena::page_bytes_mut(SLOT) } {
        Some(p) => p,
        None => {
            kprintln!("[upcall] === FAIL: no TPA page for slot ===");
            cleanup(SLOT);
            return;
        }
    };
    let flag = rd_u64(page, FLAG_ADDR as usize);
    let resumed_x0 = rd_u64(page, RESUMED_X0_ADDR as usize);
    let captured_elr = EC4_CAPTURED_ELR.load(Ordering::Acquire);
    let captured_spsr_m = EC4_CAPTURED_SPSR_M.load(Ordering::Acquire);
    let depth = PROCESS_TABLE[SLOT].upcall_depth.load(Ordering::Acquire);
    let mask = PROCESS_TABLE[SLOT].upcall_mask.load(Ordering::Acquire);

    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[upcall]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // (a) The handler ran at EL0 with the delivered arguments (it stored
    //     args[1] at args[0]).
    check(
        flag == FLAG_MARKER,
        "(a) handler ran at EL0 with correct args (flag@0x900 == marker)",
    );
    // (b) It returned via the 0x700 trampoline + Psigreturn and the interrupted
    //     EL0 code resumed to the sentinel (ELR captured = 0x1014).
    // The image runs relocated at its TPA block (ARCH03 stage 1): link VA
    // 0x1014 executes at `block + 0x1014`.
    check(
        captured_elr == loaded.basepage_va + SENTINEL_RESUME_ELR,
        "(b) resumed via trampoline+Psigreturn to sentinel (ELR == block + 0x1014)",
    );
    check(
        captured_spsr_m == 0,
        "(b) sentinel saw an EL0 return (SPSR.M == EL0t)",
    );
    // (c) The original syscall's x0 survived the upcall.
    check(
        resumed_x0 == SVERSION_RESULT,
        "(c) original syscall x0 intact after resume (== 0x3000)",
    );
    // Bookkeeping: the nesting stack and mask are balanced back to empty.
    check(depth == 0, "    upcall_depth back to 0 (push/pop balanced)");
    check(mask == 0, "    upcall_mask cleared by Psigreturn");

    cleanup(SLOT);

    if fails == 0 {
        kprintln!("[upcall] === upcall PASS ===");
    } else {
        kprintln!("[upcall] === upcall FAILED ({}) ===", fails);
    }
}

/// Free the slot's owned TPA, mark the slot free, and drop the running-process
/// pointer + sentinel arming.
fn cleanup(slot: usize) {
    EC4_TEST_RUNNING.store(false, Ordering::Release);
    set_current(core::ptr::null_mut());
    let bp = tos_api::bdos::process::pd_of(slot);
    if !bp.is_null() {
        // SAFETY: single-CPU pre-test; `bp` owns the slot's TPA block.
        unsafe {
            tos_api::bdos::mem::free_all_owned::free_all_owned(bp.cast());
        }
    }
    tos_api::bdos::process::free_slot(slot);
}
