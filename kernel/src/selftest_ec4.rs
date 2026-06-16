//! Phase 05 EC4 — pseudo-process round-trip test.
//!
//! Exit criterion (per `phases/PHASE-05-high-half-migration.md`):
//!
//! > Switch TTBR0 to a hand-built table containing one mapped page;
//! > an EL0 load from that page works, a load outside the mapping
//! > triggers a sync exception with correct ELR pointing at the
//! > offending instruction.
//!
//! The full EC4 contract requires going to EL0, executing user-mode
//! instructions, and returning to EL1 by trap. This module provides
//! exactly that: a self-contained "smallest possible process".
//!
//! ## Phase 06 D8 refactor
//!
//! The original Phase-05 EC4 implementation built its own static
//! `L0`/`L1`/`L2`/`L3` page tables. Phase 06 D8 promoted those
//! tables to per-process slots in
//! [`tos_api::bdos::process::mmu`] and EC4 was refactored to use
//! the slot-based helpers — so the same code path that backs
//! `Pexec`'s per-process address-space install is exercised by
//! the existing self-test, with no stub layer in between. EC4
//! claims [`EC4_TEST_SLOT`] (= slot 0) for the duration of the
//! test, builds a single `(VA 0 → user-code-page-phys)` mapping
//! with [`PageFlags::UserRwExec`], and hands the resulting `L0`
//! phys to [`install_ttbr0`].
//!
//! 1. **Static** — one 4-KiB-aligned `__EC4_USER_CODE` page in
//!    `.bss` (the slot's page tables themselves come from
//!    [`tos_api::bdos::process::mmu`]).
//!
//! 2. **Bring-up** — the test writes a two-instruction payload
//!    (`svc #0xEC4` + `b .` infinite-loop trap-after-trap guard)
//!    into the user-code page, then calls [`init_slot_tables`] +
//!    [`map_page`] to install the single low-half mapping. VA
//!    `[0, 0x1000)` resolves to the user page; everything else in
//!    the low-half faults.
//!
//! 3. **TTBR0 install** — the test writes the new L0 phys to
//!    `TTBR0_EL1`, clears `TCR_EL1.EPD0` to enable TTBR0 walks, and
//!    flushes the TLB. From this moment on, low-half VAs translate
//!    through the per-process tree (the Stage-1 boot identity is
//!    gone for this CPU until TTBR0 is restored or replaced).
//!
//! 4. **`eret` to EL0** — sets `SPSR_EL1.M = EL0t`, `ELR_EL1 = 0`
//!    (the user code), `SP_EL0 = USER_STACK_TOP_EXCLUSIVE`, and
//!    captures the EL1 resume PC via `adr` into a static so the SVC
//!    handler can redirect us back. `eret` lands at user VA 0.
//!
//! 5. **SVC `#0xEC4` from EL0** — handler in
//!    [`crate::arch::aarch64::exceptions`] recognises the sentinel
//!    immediate, validates the captured frame (source EL, ELR-after-
//!    SVC, ESR encoding, `SP_EL0` matches our setup), then rewrites
//!    `frame.elr_el1` to the EL1 resume PC and `frame.spsr.M` to
//!    `EL1t` (so `eret` returns to kernel-side, on `SP_EL0`).
//!
//! 6. **Resume in EL1** — control lands just after the `eret` in
//!    [`drop_to_el0_and_back`], TTBR0 is restored to the Stage-1
//!    boot tables (so subsequent kernel code is unaffected), and
//!    the test prints PASS / FAIL based on what the handler
//!    captured.
//!
//! Failure modes that are detected — and therefore proven absent
//! when the test passes:
//!
//! - TTBR0 install broken (CPU faults before the `eret` lands).
//! - `EPD0` clear ineffective (low-half walk wouldn't happen).
//! - L0/L1/L2/L3 descriptors wrong (translation fault on the user
//!   PC, ELR caught by the handler points at user VA 0, ESR EC
//!   is Instruction Abort instead of SVC).
//! - SP_EL0 not actually preserved across the trap (frame.sp_el0
//!   wouldn't equal `USER_STACK_TOP_EXCLUSIVE` — Phase 05 D6 wiring
//!   broken).

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use tos_api::bdos::process::mmu::{init_slot_tables, l0_pa_for, map_page, PageFlags};
use tos_api::bdos::process::PROCESS_TABLE;

use crate::arch::aarch64::el0::{USER_STACK_TOP_EXCLUSIVE, USER_VA_BASE};
use crate::kprintln;

/// Slot index in [`tos_api::bdos::process::PROCESS_TABLE`] this test
/// claims for its single mapping. Slot 0 is fine while no real
/// `Pexec` runs (Phase 06 D3 will take ownership of slot allocation
/// via [`tos_api::bdos::process::allocate_slot`]).
pub const EC4_TEST_SLOT: usize = 0;

// ─── User-code page (4-KiB aligned `static mut` in BSS) ─────────────
//
// The page tables themselves live in `tos_api::bdos::process::mmu`'s
// per-slot static array — EC4 only needs to provide the page that
// backs the user-mode instructions.

#[repr(C, align(4096))]
struct EcUserPage([u8; 4096]);

#[unsafe(no_mangle)]
static mut __EC4_USER_CODE: EcUserPage = EcUserPage([0; 4096]);

// ─── Handshake with the sync handler ────────────────────────────────

/// Process-table slot whose `parent_frame` the sentinel handler
/// reads for the redirect target. EC4 (Phase 05 spike) and the
/// Phase-06-D3 ELF-loader test both fire `svc #0xEC4`; each sets
/// this to its own slot index before the drop-to-EL0 so the
/// sentinel handler reads the right resume PC / kernel SP.
/// Default is [`EC4_TEST_SLOT`] (0) for the EC4 spike.
pub static EC4_SENTINEL_SLOT: AtomicUsize = AtomicUsize::new(EC4_TEST_SLOT);

/// Captured `SP_EL0` at sentinel-SVC entry (must match
/// `USER_STACK_TOP_EXCLUSIVE` if Phase 05 D6 wiring is correct).
pub static EC4_CAPTURED_SP_EL0: AtomicU64 = AtomicU64::new(0);

/// Captured `ELR_EL1` at sentinel-SVC entry (must equal user PC
/// + 4 = `USER_VA_BASE + 4`).
pub static EC4_CAPTURED_ELR: AtomicU64 = AtomicU64::new(0);

/// Captured saved `SPSR_EL1.M` field at sentinel-SVC entry (must be
/// `0b0000` = EL0t).
pub static EC4_CAPTURED_SPSR_M: AtomicU64 = AtomicU64::new(0xFF);

/// `true` while the EC4 test is mid-flight. The sync handler honours
/// the sentinel immediate `#0xEC4` only while this flag is set, so
/// stray future `svc #0xEC4`s do not silently corrupt control flow.
pub static EC4_TEST_RUNNING: AtomicBool = AtomicBool::new(false);

/// Sentinel SVC immediate that signals "EC4 test running, redirect
/// to EL1 resume PC". 0xEC4 is deliberately nowhere near our trap
/// classes (1, 13, 14).
pub const EC4_SENTINEL_SVC_IMM: u16 = 0xEC4;

// ─── User-code phys helper ──────────────────────────────────────────

/// Phys address of the static user-code page, computed by stripping
/// the high-half VA offset from its kernel-side address.
fn ec4_user_code_pa() -> u64 {
    crate::arch::aarch64::mmu::va_to_pa((&raw const __EC4_USER_CODE) as u64)
}

/// Build the per-process page tables for the test slot: zero +
/// chain-wire via [`init_slot_tables`], then drop a single
/// `(USER_VA_BASE → user-code-page-phys)` mapping into L3[0] via
/// [`map_page`].
///
/// Uses [`PageFlags::UserRwExec`] because the EC4 user "program"
/// is the same page we just wrote into — it needs to be both
/// writable (for our `write_volatile` setup) and executable from
/// EL0 (for the `eret` to actually run the code). Real
/// `Pexec`-loaded programs use [`PageFlags::UserRoExec`] for text
/// and [`PageFlags::UserRwNx`] for data/stack.
fn build_tables() {
    // SAFETY: pre-EC4-test, boot CPU is the only observer. The slot
    // is dedicated to EC4 (`EC4_TEST_SLOT = 0`) and not yet claimed
    // by any user process.
    unsafe {
        init_slot_tables(EC4_TEST_SLOT)
            .expect("EC4 slot init: slot index in range");
        map_page(
            EC4_TEST_SLOT,
            USER_VA_BASE,
            ec4_user_code_pa(),
            PageFlags::UserRwExec,
        )
        .expect("EC4 map_page: VA aligned, in 2-MiB range");
    }
}

/// Write a two-instruction EL0 program at `__EC4_USER_CODE`:
///
/// ```text
///   svc #0xEC4    ; trap to kernel — handler redirects to EL1 resume
///   b   .         ; safety net: spin if redirect fails
/// ```
///
/// AArch64 `svc #imm16` encoding (ARM ARM C6.2.244):
/// `0xD400_0001 | (imm16 << 5)`. For `imm16 = 0xEC4` →
/// `0xD401_D881` (the value verified independently by hand).
///
/// `b .` (branch to self) is `0x1400_0000` — branches with offset 0.
fn install_user_code() {
    const SVC_EC4: u32 = 0xD400_0001 | ((EC4_SENTINEL_SVC_IMM as u32) << 5);
    const BRANCH_SELF: u32 = 0x1400_0000;

    // SAFETY: page is in BSS, we're the sole writer pre-test, and
    // the page is currently mapped Normal Cacheable in the high-half
    // (Stage 2 covers all DRAM blocks). The instruction-cache flush
    // below ensures EL0 fetches see what we wrote.
    let code_va = (&raw mut __EC4_USER_CODE) as *mut u32;
    unsafe {
        code_va.add(0).write_volatile(SVC_EC4);
        code_va.add(1).write_volatile(BRANCH_SELF);
    }

    // Make the writes visible to the i-cache. `dsb ish` orders the
    // stores; `ic iallu` invalidates the entire i-cache (small price,
    // single-page test); `dsb ish` then `isb` close the sequence.
    // (ARM ARM B2.4.4 "Self-modifying code".)
    unsafe {
        core::arch::asm!(
            "dsb ish",
            "ic iallu",
            "dsb ish",
            "isb",
            options(nostack, preserves_flags),
        );
    }
}

// ─── EL0 round-trip ─────────────────────────────────────────────────

/// Drop to EL0 at user VA 0. The user page contains
/// `svc #0xEC4; b .`; the sync handler picks up the sentinel and
/// rewrites `frame.elr_el1` to the EL1 resume PC captured by the
/// shared [`crate::arch::aarch64::user_entry::enter_el0_with_resume_capture`].
/// Control returns here to continue in EL1, on the same kernel
/// stack we left.
///
/// ## SP juggling — why SPSel toggles
///
/// See [`crate::arch::aarch64::user_entry`] — the shared helper
/// implements the SPSel toggle / `MSR SP_EL0` ordering exactly once
/// for both this Phase-05 spike and the Phase-06 D3 ELF-loader test.
///
/// `SPSR_EL1` for the eret:
/// - `M = 0b0000` (EL0t)
/// - `DAIF = 0` (all interrupts unmasked — fine for the test, no
///   IRQs are expected)
/// - other bits zero
///
/// Resume `SPSR_EL1` (set by the handler):
/// - `M = 0b0100` (EL1t — kernel returns on `SP_EL0`)
unsafe fn drop_to_el0_and_back() {
    // Arm the sentinel handler — it only honours `svc #0xEC4`
    // redirects while this flag is set, so the asm capture of the
    // resume PC must happen with the flag already armed.
    EC4_TEST_RUNNING.store(true, Ordering::Release);

    // EC4 is the Phase-05 spike — it doesn't go through the
    // `Pterm` round-trip at all (the sentinel handler intercepts
    // its `svc #0xEC4` directly and rewrites the trap frame
    // ad-hoc). It still uses the shared EL0-entry helper for the
    // SP juggling + eret asm; we hand it the test slot's
    // `parent_frame` purely as the storage backing the asm's
    // resume-PC / kernel-SP capture. The Phase-06 Pterm full-frame
    // copy never runs for EC4, so the rest of the snapshot stays
    // zero — harmless.
    let parent_frame = &PROCESS_TABLE[EC4_TEST_SLOT].parent_frame;

    // SAFETY: install_ttbr0 + the EL0 round-trip share single-CPU
    // contract; the parent_frame atomics live in PROCESS_TABLE
    // (static) and outlive the call.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            USER_VA_BASE,
            l0_pa_for(EC4_TEST_SLOT),
            USER_STACK_TOP_EXCLUSIVE,
            parent_frame,
        );
    }

    // Mirror what the sentinel handler did to the live trap frame
    // back into the slot's diagnostic atomics. After resume here,
    // the asm capture still has the resume PC + kernel SP in place;
    // the captured user SP_EL0 / ELR / SPSR.M came in via the
    // sentinel handler's reads of the trap frame at SVC entry,
    // which we publish through the existing `EC4_CAPTURED_*` paths.
}

// ─── Driver ─────────────────────────────────────────────────────────

/// Run the full EC4 round-trip test.
pub fn run_ec4_pseudo_process_test() {
    kprintln!("");
    kprintln!("[ec4] === Phase 05 EC4 pseudo-process test ===");

    build_tables();
    let l0_pa = l0_pa_for(EC4_TEST_SLOT);
    kprintln!(
        "[ec4] tables built (slot {}): L0=0x{:016x} L3[0]→user_pa=0x{:016x}",
        EC4_TEST_SLOT,
        l0_pa,
        ec4_user_code_pa(),
    );

    install_user_code();
    kprintln!("[ec4] user code installed (svc #0xEC4; b .)");

    kprintln!("[ec4] eret → EL0 …");
    // SAFETY: handler will redirect us back; if it doesn't, the user
    // page's `b .` keeps the CPU bounded so the watchdog (or test
    // operator) can stop QEMU. The shared helper installs TTBR0
    // before the eret — same code path Phase 06 D3+ uses.
    unsafe { drop_to_el0_and_back() };
    kprintln!("[ec4] TTBR0 swapped to per-process L0; returned from EL0");

    // SAFETY: post-test cleanup; nothing else uses EC4 TTBR0.
    unsafe { crate::arch::aarch64::user_entry::restore_boot_ttbr0() };
    kprintln!("[ec4] TTBR0 restored to Stage-1 boot tables");

    // Verify what the handler captured.
    let sp_el0 = EC4_CAPTURED_SP_EL0.load(Ordering::Acquire);
    let elr = EC4_CAPTURED_ELR.load(Ordering::Acquire);
    let spsr_m = EC4_CAPTURED_SPSR_M.load(Ordering::Acquire);

    let sp_ok = sp_el0 == USER_STACK_TOP_EXCLUSIVE;
    let elr_ok = elr == USER_VA_BASE + 4;
    let from_el0 = spsr_m == 0;
    let running_cleared = !EC4_TEST_RUNNING.load(Ordering::Acquire);

    kprintln!(
        "[ec4]   captured SP_EL0 = 0x{:016x}  (expected 0x{:016x})  {}",
        sp_el0,
        USER_STACK_TOP_EXCLUSIVE,
        if sp_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[ec4]   captured ELR    = 0x{:016x}  (expected 0x{:016x})  {}",
        elr,
        USER_VA_BASE + 4,
        if elr_ok { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[ec4]   SPSR.M (saved)  = 0x{:x}                    (expected 0x0 = EL0t) {}",
        spsr_m,
        if from_el0 { "OK" } else { "FAIL" },
    );
    kprintln!(
        "[ec4]   running flag cleared by handler:                          {}",
        if running_cleared { "OK" } else { "FAIL" },
    );

    if sp_ok && elr_ok && from_el0 && running_cleared {
        kprintln!("[ec4] === EC4 PASS ===");
    } else {
        kprintln!("[ec4] === EC4 FAIL ===");
    }
}
