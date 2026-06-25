//! EL0 entry — the asm sequence that drops the CPU into user mode.
//!
//! Phase 06 D5 consolidates the EL0-transition code that the
//! Phase-05 EC4 spike and the Phase-06 D3 ELF-loader self-test had
//! independently duplicated. Phase 06 D6 (`Pterm`) reuses the same
//! helper — production and test callers all want the same five
//! steps before `eret`-ing into user space:
//!
//!   1. Capture an EL1 resume PC into a caller-supplied atomic so
//!      the trap handler that fields the user's exit (`Pterm` SVC,
//!      EC4 sentinel SVC, …) knows where to redirect.
//!   2. Save the kernel's current SP into another caller-supplied
//!      atomic so the handler can restore `frame.sp_el0` to it.
//!   3. Switch `SPSel = 1` so writes to `SP_EL0` are well-defined.
//!   4. Configure `SP_EL0` / `ELR_EL1` / `SPSR_EL1` for the eret.
//!   5. `eret` to EL0; on the handler-redirected return, land at the
//!      captured EL1 resume PC.
//!
//! Sentinel-specific bookkeeping (the EC4 "running flag" that gates
//! `0xEC4`-immediate redirects) is the test driver's responsibility:
//! the driver writes the flag in Rust *before* calling this helper.
//! Pterm has no analogous flag — the trap dispatcher recognises it
//! by the SVC immediate + GEMDOS function ID directly.
//!
//! ## Why the resume-capture sits *inside* the same asm block
//!
//! `adr {pc}, 1f` resolves the address of the `1:` label using a
//! PC-relative `ADR` (±1 MiB range). The label must therefore live
//! in the same translation unit *and* the same asm block as the
//! `adr` — splitting the prelude and the eret into two `asm!` blocks
//! makes `1:` invisible to the prelude (inline-asm local labels are
//! per-block). Keeping everything in one block also matches what the
//! caller actually wants: the handler's eret lands one instruction
//! past the eret-to-EL0, so a single linear asm sequence is the
//! natural shape.
//!
//! ## Why this is `unsafe`
//!
//! * `install_ttbr0` rewrites the per-CPU low-half translation
//!   regime; concurrent EL0 access on another CPU is undefined.
//! * The asm reads / writes raw atomic pointers; the caller must
//!   guarantee they're valid for the duration of the round-trip.
//! * After eret to EL0 control depends on the handler honouring the
//!   sentinel; a buggy handler would leave the CPU spinning in the
//!   user code's `b .` safety net (or worse, faulting in EL0 with
//!   an indirectly-wrong frame).

use core::sync::atomic::{AtomicU64, Ordering};

use tos_api::bdos::process::mmu::install_ttbr0;
use tos_api::bdos::process::ParentFrame;

// ─── Stage-1 boot-table phys (linker symbol) ────────────────────────
//
// `__boot_l0_phys` is a linker-script absolute symbol whose *value*
// is the low-half phys of the Stage-1 boot L0 table (`__BOOT_L0 -
// VA_OFFSET` in the link script). Resolving it via `&raw const` from
// high-half kernel code would emit `ADRP+ADD` (`R_AARCH64_ADR_PREL_PG_HI21`,
// ±4 GiB range) which cannot bridge the high-half→low-half distance.
// `ldr =sym` goes through a literal pool — 64-bit absolute, works
// regardless of the kernel's link address.

unsafe extern "C" {
    static __boot_l0_phys: u8;
}

#[inline]
fn boot_l0_phys() -> u64 {
    let v: u64;
    // SAFETY: pure register load of a link-time constant.
    unsafe {
        core::arch::asm!(
            "ldr {out}, ={sym}",
            out = out(reg) v,
            sym = sym __boot_l0_phys,
            options(nomem, nostack, preserves_flags)
        );
    }
    v
}

/// Restore `TTBR0_EL1` to the Stage-1 boot tables installed by
/// `boot._start`. Includes a `tlbi vmalle1` so any cached entries
/// from the dying slot's L3 are dropped before a future per-process
/// install.
///
/// # Safety
///
/// Caller must be the boot CPU (single-CPU contract). The kernel
/// must not be holding a low-half VA reference — between this call
/// and the next per-process TTBR0 install, low-half VAs do not
/// resolve through any user mapping (only the boot identity range
/// does, and only for the boot-table-mapped phys frames).
pub unsafe fn restore_boot_ttbr0() {
    let boot_l0 = boot_l0_phys();
    // SAFETY: writing TTBR0_EL1 + TLBI is permitted at EL1.
    unsafe {
        core::arch::asm!(
            "dsb ishst",
            "msr ttbr0_el1, {ttbr0}",
            "tlbi vmalle1",
            "dsb ish",
            "isb",
            ttbr0 = in(reg) boot_l0,
            options(nostack, preserves_flags),
        );
    }
}

/// Drop to EL0 at `entry_va`, with the per-process L0 at `l0_pa`
/// installed in `TTBR0_EL1` and `SP_EL0 = sp_el0`. The function
/// returns when the trap handler that fields the user's exit
/// (`Pterm` SVC, EC4 sentinel SVC, …) rewrites the trap frame's
/// `elr_el1` / `sp_el0` / `spsr` to the EL1 resume PC captured by
/// this helper, so the *Rust* caller observes a normal return.
///
/// `parent_frame` is the slot's [`ParentFrame`] snapshot. The asm
/// captures the EL1 resume PC into `parent_frame.elr()` and the
/// current kernel SP into `parent_frame.sp_el0()`; the Rust code
/// before the asm sets `parent_frame.spsr() = 0b0100` (EL1t) so
/// the Pterm handler's full-frame restore brings us back to EL1
/// on the kernel stack we left. The remaining fields (`gpr`, `lr`)
/// stay zero — the kernel-resume path doesn't depend on them
/// because the Rust function epilogue restores its callee-saved
/// regs from the local stack frame.
///
/// Sentinel-specific bookkeeping (e.g. the EC4 "running" flag that
/// gates `0xEC4`-immediate redirects) is the caller's responsibility:
/// the test driver sets the flag in Rust before calling this helper.
/// Pterm needs no equivalent — the trap dispatcher matches on the
/// SVC immediate + GEMDOS function ID directly.
///
/// # Safety
///
/// * `l0_pa` must be the phys of a 4-KiB-aligned, fully-initialised
///   per-process L0 page table.
/// * `parent_frame` must point at the [`ParentFrame`] of a slot
///   whose `parent_l0_pa` is already set to the kernel-resume
///   target (typically `0` for the boot path).
/// * Single-CPU contract — no other CPU is allowed to touch
///   `TTBR0_EL1` while this function is in flight.
/// * `entry_va` must lie within the slot's mapped low-half VA range
///   (otherwise the eret faults at the user PC).
pub unsafe fn enter_el0_with_resume_capture(
    entry_va: u64,
    l0_pa: u64,
    sp_el0: u64,
    parent_frame: &ParentFrame,
) {
    // Pre-set the SPSR to EL1t so the handler-side eret returns
    // here on the kernel stack with `SPSel = 0` (active SP =
    // SP_EL0 = saved kernel SP).
    parent_frame
        .spsr()
        .store(0b0100, Ordering::Release);

    // SAFETY: caller contract — single-CPU, valid l0_pa.
    unsafe { install_ttbr0(l0_pa) };

    // DIAG: read TTBR0 right after install to confirm it stuck.
    let ttbr0_after: u64;
    unsafe {
        core::arch::asm!(
            "mrs {out}, ttbr0_el1",
            out = out(reg) ttbr0_after,
            options(nomem, nostack, preserves_flags),
        );
    }
    crate::kprintln!(
        "[ee0/cachefix] install_ttbr0(0x{:x}) → mrs reads 0x{:x}",
        l0_pa, ttbr0_after,
    );

    let elr_ptr: *const AtomicU64 = parent_frame.elr();
    let sp_ptr: *const AtomicU64 = parent_frame.sp_el0();

    // SAFETY: parent_frame outlives the round-trip; the asm uses
    // raw pointers into the atomics, with single-CPU exclusion
    // honoured by the BDOS lock.
    unsafe {
        core::arch::asm!(
            // Capture EL1 resume PC into parent_frame.elr.
            "adr   {tmp}, 1f",
            "str   {tmp}, [{resume_pc}]",

            // Save the current SP (= SP_EL0 since SPSel=0) via
            // `mov xN, sp` — the only legal way while SPSel=0.
            "mov   {kstack}, sp",
            "str   {kstack}, [{kernel_sp}]",

            // Switch SPSel = 1 so the subsequent `MSR SP_EL0, …` is
            // well-defined (writing SP_EL0 while it's the active SP
            // is UNPREDICTABLE — ARM ARM D17.2.130).
            "msr   spsel, #1",

            // Configure the eret target.
            "msr   sp_el0, {sp}",
            "msr   elr_el1, {entry}",
            "mov   {tmp}, #0",          // SPSR.M = EL0t, DAIF = 0
            "msr   spsr_el1, {tmp}",

            // Drop to EL0.
            "eret",

            // Resume here. The Pterm handler restored
            // `parent_frame.spsr.M = EL1t` and `parent_frame.sp_el0
            // = saved kernel SP` into the trap frame, so the
            // handler-side eret lands us at EL1 with `SP_EL0 = our
            // original kernel stack`. Active SP is SP_EL0 again.
            "1:",
            entry      = in(reg) entry_va,
            sp         = in(reg) sp_el0,
            resume_pc  = in(reg) elr_ptr,
            kernel_sp  = in(reg) sp_ptr,
            kstack     = out(reg) _,
            tmp        = out(reg) _,
            options(nostack)
        );
    }
}
