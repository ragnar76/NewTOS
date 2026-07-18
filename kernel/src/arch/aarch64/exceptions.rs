//! AArch64 exception entry: vector table, register save/restore, and
//! Rust-level dispatch.
//!
//! ## Vector layout (`exception_vectors.S`)
//!
//! AArch64 mandates a 2 KiB aligned table with sixteen 128-byte slots
//! grouped into four sets of four entries: synchronous, IRQ, FIQ,
//! SError, replicated for each entry source (current EL/SP0,
//! current EL/SPx, lower-EL/AArch64, lower-EL/AArch32). See ARM ARM
//! D1.10.2 "Exception vectors".
//!
//! Phase 02 only meaningfully implements the "Synchronous, current EL
//! with SPx" entry (used while we still run with EL1 and SP_EL1 ==
//! SP), all other entries trap into [`unhandled`] for diagnostics.
//!
//! ## ABI
//!
//! Synchronous handler stack frame (built by the assembly stub —
//! Phase 05 D6 expanded the frame from 272 B to 288 B for `SP_EL0`):
//!
//! ```text
//!   sp+ 0:   x0   .. x29 (30 × 8 B = 240 B)
//!   sp+240:  lr  (x30)
//!   sp+248:  ELR_EL1
//!   sp+256:  SPSR_EL1
//!   sp+264:  ESR_EL1
//!   sp+272:  SP_EL0   (user-space stack pointer at trap entry)
//!   sp+280:  pad      (reserved — keeps the frame 16-B aligned;
//!                      future TPIDR_EL0 / process tag slot)
//! ```
//!
//! The 16-byte-aligned SP is required by AAPCS64 §5.2.2.1 at every
//! public function call boundary.

use core::arch::global_asm;
use core::sync::atomic::{AtomicBool, Ordering};

use tos_api::trap::{TrapArgs, TrapClass};

global_asm!(include_str!("exception_vectors.s"));

/// Saved register frame layout (matches `exception_vectors.s`). Total
/// 288 bytes = 18 × 16 B (16-byte SP-aligned per AAPCS64 §5.2.2).
///
/// Phase 06 will read [`Self::sp_el0`] in the SVC dispatcher to
/// access user-stack-passed parameters (e.g. m68k-style TOS calls
/// after the marshaling shim, see `docs/ARCHITECTURE.md` § "ABI
/// rules — m68k callers"). Until then, traps originate from EL1
/// and `sp_el0` carries the kernel's main stack pointer (set by the
/// `kernel_main` shim, [`crate::kernel_main`]).
#[repr(C)]
pub struct ExceptionFrame {
    /// `x0..x29` at exception entry.
    pub gpr: [u64; 30],
    /// `x30` (link register).
    pub lr: u64,
    /// Exception link register — the address to return to.
    pub elr: u64,
    /// Saved program status register.
    pub spsr: u64,
    /// Exception syndrome.
    pub esr: u64,
    /// User-space stack pointer (`SP_EL0`) at trap entry.
    pub sp_el0: u64,
    /// Reserved 8-byte pad (keeps `stp`/`ldp` pair-aligned; future
    /// TPIDR_EL0 or per-process tag slot).
    _pad: u64,
}

const _: () = assert!(core::mem::size_of::<ExceptionFrame>() == 288);
const _: () = assert!(core::mem::align_of::<ExceptionFrame>() == 8);

// ---------------------------------------------------------------------------
// ESR_EL1 decoding (ARM ARM D17.2.37)
// ---------------------------------------------------------------------------

/// `ESR_EL1.EC` field for `SVC` from AArch64 EL0 (`0x15`) — used after
/// PHASE-06 introduces user-mode processes.
const EC_SVC_AARCH64: u64 = 0x15;

/// `ESR_EL1.EC` field for `SVC` from the same EL (`0x15` shares the
/// encoding with EL0; bit 0 of EC distinguishes — see ARM ARM table
/// D17-32). PHASE-02 runs in EL1 and uses SVC for self-tests, so we
/// accept both encodings.
const EC_SVC_FROM_EL1: u64 = 0x15;

/// `ESR_EL1.EC` for an Instruction Abort taken from a lower EL.
const EC_IABT_LOWER: u64 = 0x20;
/// `ESR_EL1.EC` for an Instruction Abort taken from the same EL.
const EC_IABT_SAME: u64 = 0x21;
/// `ESR_EL1.EC` for a Data Abort taken from a lower EL.
const EC_DABT_LOWER: u64 = 0x24;
/// `ESR_EL1.EC` for a Data Abort taken from the same EL — what a
/// kernel-side stack overflow into the guard page produces.
const EC_DABT_SAME: u64 = 0x25;

#[inline]
const fn esr_ec(esr: u64) -> u64 {
    (esr >> 26) & 0x3F
}

#[inline]
const fn esr_iss(esr: u64) -> u64 {
    esr & 0x01FF_FFFF
}

#[inline]
const fn is_abort(ec: u64) -> bool {
    matches!(ec, EC_IABT_LOWER | EC_IABT_SAME | EC_DABT_LOWER | EC_DABT_SAME)
}

#[inline]
fn read_far_el1() -> u64 {
    let v: u64;
    // SAFETY: `mrs` of FAR_EL1 is permitted at EL1 and has no side
    // effects.
    unsafe {
        core::arch::asm!(
            "mrs {0}, far_el1",
            out(reg) v,
            options(nomem, nostack, preserves_flags)
        );
    }
    v
}

#[inline]
fn ec_name(ec: u64) -> &'static str {
    match ec {
        EC_SVC_AARCH64 => "SVC (AArch64)",
        EC_IABT_LOWER => "Instruction Abort (lower EL)",
        EC_IABT_SAME => "Instruction Abort (same EL)",
        EC_DABT_LOWER => "Data Abort (lower EL)",
        EC_DABT_SAME => "Data Abort (same EL)",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// VBAR_EL1 setup
// ---------------------------------------------------------------------------

unsafe extern "C" {
    /// Symbol exported by `exception_vectors.s` — the start of the
    /// 2 KiB vector table.
    fn __exception_vectors();
}

/// Install the exception vector table by writing its address into
/// `VBAR_EL1`.
///
/// Must be called once during boot before any SVC is executed.
pub fn init() {
    // SAFETY: writing VBAR_EL1 is permitted at EL1 and the symbol
    // address is provided by the linker; the table is `.align 11`.
    unsafe {
        core::arch::asm!(
            "msr vbar_el1, {0}",
            "isb",
            in(reg) __exception_vectors as *const () as usize,
            options(nomem, nostack, preserves_flags)
        );
    }
}

// ---------------------------------------------------------------------------
// Recoverable MMIO probe (optional / possibly-absent controllers)
// ---------------------------------------------------------------------------

/// Set while [`probe_read32`] is doing its single guarded MMIO read, so the
/// synchronous handler knows a Data Abort in that window is expected.
static PROBE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Set by the handler when the guarded read took a Data Abort — the target
/// device did not respond.
static PROBE_FAULTED: AtomicBool = AtomicBool::new(false);

/// Read a 32-bit MMIO register that may belong to a **possibly-absent**
/// controller, surviving a synchronous external abort. Returns `Some(value)`
/// if the read completed, or `None` if the access faulted — i.e. nothing
/// responds at `va` (e.g. the BCM2711 PCIe root complex under QEMU `raspi4b`,
/// which is not modelled). EL1, single-CPU: the read is a single 4-byte load,
/// and on a Data Abort the handler advances `ELR` past it and sets
/// [`PROBE_FAULTED`]. `va` must be a mapped device VA (a translation fault is
/// **not** recovered — only external aborts on a mapped MMIO page).
pub fn probe_read32(va: u64) -> Option<u32> {
    PROBE_FAULTED.store(false, Ordering::SeqCst);
    PROBE_ACTIVE.store(true, Ordering::SeqCst);
    core::sync::atomic::compiler_fence(Ordering::SeqCst);
    // SAFETY: `va` is a mapped device VA; a fault here is caught and recovered
    // by `rust_sync_handler` (advances ELR, sets PROBE_FAULTED), so the read
    // never escalates to `unhandled`. The loaded value is ignored on fault.
    let v = unsafe { core::ptr::read_volatile(va as *const u32) };
    core::sync::atomic::compiler_fence(Ordering::SeqCst);
    PROBE_ACTIVE.store(false, Ordering::SeqCst);
    if PROBE_FAULTED.load(Ordering::SeqCst) {
        None
    } else {
        Some(v)
    }
}

// ---------------------------------------------------------------------------
// Rust-level handlers (called from exception_vectors.s)
// ---------------------------------------------------------------------------

/// Synchronous-exception handler. Decodes ESR_EL1; if it is an SVC
/// from a recognized class, dispatches into `tos-api`. Updates
/// `frame.gpr[0]` so the assembly stub returns the value in `x0`.
#[unsafe(no_mangle)]
pub extern "C" fn rust_sync_handler(frame: &mut ExceptionFrame) {
    let ec = esr_ec(frame.esr);
    if ec != EC_SVC_AARCH64 && ec != EC_SVC_FROM_EL1 {
        // Recoverable MMIO probe: a same-EL Data Abort while a `probe_read32`
        // is in flight means the probed device did not respond. Flag it and
        // step `ELR` past the faulting load (all A64 instructions are 4 bytes)
        // so the probe returns `None` instead of panicking. Every other abort
        // still falls through to `unhandled`.
        if ec == EC_DABT_SAME && PROBE_ACTIVE.load(Ordering::SeqCst) {
            PROBE_FAULTED.store(true, Ordering::SeqCst);
            frame.elr = frame.elr.wrapping_add(4);
            return;
        }
        unhandled(frame, "synchronous (non-SVC)");
    }

    let svc_imm = (esr_iss(frame.esr) & 0xFFFF) as u16;

    // Phase 05 EC4 sentinel — redirect a from-EL0 `svc #0xEC4` back
    // to the EL1 resume PC captured by `selftest_ec4`. The redirect
    // is gated on `EC4_TEST_RUNNING` so a stray sentinel SVC outside
    // the test window is harmless. The redirect target lives in
    // `PROCESS_TABLE[EC4_SENTINEL_SLOT].parent_frame`, populated by
    // `enter_el0_with_resume_capture` just before the `eret`-to-EL0.
    if svc_imm == crate::selftest_ec4::EC4_SENTINEL_SVC_IMM
        && crate::selftest_ec4::EC4_TEST_RUNNING.load(Ordering::Acquire)
    {
        // Capture diagnostic state for the test driver to assert on.
        crate::selftest_ec4::EC4_CAPTURED_SP_EL0
            .store(frame.sp_el0, Ordering::Release);
        crate::selftest_ec4::EC4_CAPTURED_ELR
            .store(frame.elr, Ordering::Release);
        crate::selftest_ec4::EC4_CAPTURED_SPSR_M
            .store(frame.spsr & 0xF, Ordering::Release);

        // Mark test inactive *before* the redirect so a logic bug
        // that re-fires the sentinel doesn't recurse.
        crate::selftest_ec4::EC4_TEST_RUNNING
            .store(false, Ordering::Release);

        // Redirect: rewrite frame so the post-handler `eret` lands
        // at the EL1 resume PC, on the original kernel SP_EL0.
        let slot_idx = crate::selftest_ec4::EC4_SENTINEL_SLOT
            .load(Ordering::Acquire);
        let pf = &tos_api::bdos::process::PROCESS_TABLE[slot_idx]
            .parent_frame;
        let resume_pc = pf.elr().load(Ordering::Acquire);
        let kernel_sp = pf.sp_el0().load(Ordering::Acquire);
        frame.elr = resume_pc;
        // Restore the kernel's SP_EL0 (the user-stack capture above
        // already saved the diagnostic value); the asm restore will
        // write this back into `SP_EL0`, undoing the user-stack
        // overwrite the test installed before `eret`-ing to EL0.
        frame.sp_el0 = kernel_sp;
        // SPSR.M[3:0] = 0b0100 = EL1t (EL1, SP_EL0 active). Kernel
        // resumes on its own stack.
        frame.spsr = 0b0100;
        return;
    }

    let class = match TrapClass::from_imm(svc_imm) {
        Some(c) => c,
        None => {
            // Unknown trap class → EINVFN.
            frame.gpr[0] = tos_types::error::EINVFN as i64 as u64;
            return;
        }
    };

    // GEMDOS Pterm (0x4C) / Pterm0 (0x00) / Ptermres (0x31) and Pexec
    // (0x4B) need the trap frame mutated rather than just an `x0`
    // return — the dispatcher table can't express that, so we
    // special-case them here.
    if matches!(class, TrapClass::Gemdos) {
        let func = frame.gpr[8] as u16;
        if func == 0x4C || func == 0x00 || func == 0x31 {
            handle_pterm(frame, func);
            return;
        }
        if func == 0x4B {
            handle_pexec(frame);
            return;
        }
        if func == 0x4D {
            handle_pwait(frame);
            return;
        }
    }

    let args = TrapArgs {
        regs: [
            frame.gpr[0],
            frame.gpr[1],
            frame.gpr[2],
            frame.gpr[3],
            frame.gpr[4],
            frame.gpr[5],
            frame.gpr[6],
            frame.gpr[7],
        ],
        func: frame.gpr[8],
        svc_imm,
    };

    // Result is the full 64-bit `x0` value: GEMDOS returns a sign-
    // extended `Long`, but BIOS `Getbpb` returns a native 64-bit pointer.
    let result: i64 = match class {
        TrapClass::Gemdos => tos_api::gemdos::dispatch(&args) as i64,
        // GEM trap (#2): shared by the VDI and the AES. `x0` is the
        // selector (115 = VDI, 200 = AES), `x1` the parameter block.
        // Neither returns a value in a register, so leave `x0 = 0`.
        TrapClass::Gem => {
            match args.regs[0] {
                // SAFETY: `x1` is the caller-supplied parameter block.
                115 => unsafe { tos_api::vdi::dispatch(args.regs[1]) },
                200 => unsafe { tos_api::aes::dispatch(args.regs[1]) },
                _ => {} // unknown selector → no-op (GEM trap is trusted)
            }
            0
        }
        // BIOS (Trap 13): block-device + console functions.
        TrapClass::Bios => tos_api::bios::dispatch(&args),
        // XBIOS (Trap 14): screen group (Physbase/Logbase/Getrez/...).
        TrapClass::Xbios => tos_api::xbios::dispatch(&args),
    };

    // A blocking console input function (Cconin / Cnecin / Cconrs) may
    // have seen a Ctrl-C and recorded a deferred `Pterm(-32)`. A leaf
    // GEMDOS function returns a value in `x0` and cannot redirect the
    // trap frame; consume the request here and terminate the process
    // exactly like an explicit `Pterm`, discarding `result`.
    if let Some(rc) = tos_api::bdos::process::take_pending_terminate() {
        terminate_and_redirect(frame, rc);
        return;
    }

    // Write the 64-bit result into `x0` (error codes are already
    // sign-extended; `Getbpb` returns a full pointer).
    frame.gpr[0] = result as u64;
}

/// `Pwait` (GEMDOS 0x4D) — cooperative yield. Snapshot the running peer's
/// full EL0 context into its slot, then round-robin to the next runnable
/// peer (a context switch like `Pexec`/`Pterm`, but peer→peer). With no
/// other runnable peer it is a no-op that returns 0.
fn handle_pwait(frame: &mut ExceptionFrame) {
    use core::sync::atomic::Ordering;
    use tos_api::bdos::process::{sched, MAX_PROCESSES, PROCESS_TABLE};

    let cur = sched::current();
    if cur >= MAX_PROCESSES {
        frame.gpr[0] = 0; // not a scheduler peer → harmless no-op
        return;
    }

    // Save the running peer's full frame so the round-robin can resume it.
    let sf = &PROCESS_TABLE[cur].saved_frame;
    for i in 0..30 {
        sf.gpr(i).store(frame.gpr[i], Ordering::Release);
    }
    sf.lr().store(frame.lr, Ordering::Release);
    sf.elr().store(frame.elr, Ordering::Release);
    sf.spsr().store(frame.spsr, Ordering::Release);
    sf.sp_el0().store(frame.sp_el0, Ordering::Release);
    sched::mark_yielded(cur);

    match sched::on_yield(cur) {
        Some(sw) => apply_switch(frame, &sw),
        None => frame.gpr[0] = 0, // only runnable peer → continue
    }
}

/// Install the next peer's address space and rewrite the trap frame so the
/// post-handler `eret` lands in it — fresh at its entry point, or resumed
/// from its saved EL0 context. Shared by `Pwait` and scheduler-aware
/// `Pterm`.
fn apply_switch(frame: &mut ExceptionFrame, sw: &tos_api::bdos::process::sched::Switch) {
    use core::sync::atomic::Ordering;
    use tos_api::bdos::process::{sched, State, PROCESS_TABLE};

    // TTBR0 switch with the QUIRK-001 bracket (QEMU cortex-a72 TCG): a
    // TLBI (boot-restore) then UART-bearing prints then the no-TLBI
    // install, so the `eret`-to-EL0 later does not wedge. Harmless on
    // real hardware. See `handle_pexec` / `finish_terminate`.
    // SAFETY: trap context, single-CPU; no kernel low-half VA held.
    unsafe {
        crate::arch::aarch64::user_entry::restore_boot_ttbr0();
    }
    crate::kprintln!("[sched] switch -> slot {}", sw.slot);
    unsafe {
        tos_api::bdos::process::mmu::install_ttbr0(sw.l0_pa);
    }
    crate::kprintln!("[sched] running");

    if sw.fresh {
        for i in 0..30 {
            frame.gpr[i] = 0;
        }
        frame.gpr[0] = tos_api::bdos::process::loader::USER_VA_BASE;
        frame.lr = 0;
        frame.elr = sw.entry_va;
        frame.spsr = 0; // SPSR.M = EL0t
        frame.sp_el0 = crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
    } else {
        let sf = &PROCESS_TABLE[sw.slot].saved_frame;
        for i in 0..30 {
            frame.gpr[i] = sf.gpr(i).load(Ordering::Acquire);
        }
        frame.lr = sf.lr().load(Ordering::Acquire);
        frame.elr = sf.elr().load(Ordering::Acquire);
        frame.spsr = sf.spsr().load(Ordering::Acquire);
        frame.sp_el0 = sf.sp_el0().load(Ordering::Acquire);
        frame.gpr[0] = 0; // the resumed `Pwait` returns 0
    }

    let bp = PROCESS_TABLE[sw.slot].basepage_kva.load(Ordering::Acquire);
    tos_api::bdos::process::set_current(bp);
    PROCESS_TABLE[sw.slot].set_state(State::Running);
    sched::set_running(sw.slot);
}

/// Pterm (GEMDOS 0x4C) / Pterm0 (0x00) handler. Rewrites the trap
/// frame to redirect the post-handler `eret` at the dying process's
/// captured parent resume PC, after freeing every BDOS allocation
/// the process owned.
///
/// On error the frame is left unchanged and `x0` is set to
/// [`tos_types::error::EINVFN`] — Pterm has no error code in the
/// EmuTOS ABI, so this is purely a debugging affordance for
/// the D6 selftest.
fn handle_pterm(frame: &mut ExceptionFrame, func: u16) {
    // Capture the user's exit code BEFORE termination runs. ABI per
    // function:
    //   * `Pterm0`   (0x00): rc fixed at 0, no args.
    //   * `Pterm`    (0x4C): rc in `x0` (= `frame.gpr[0]`).
    //   * `Ptermres` (0x31): `Ptermres(blkln, rc)` — `blkln` in `x0`,
    //     `rc` in `x1`. `blkln` (keep-resident size) is a no-op under
    //     the static-arena TPA model (see `pterm::terminate_resident`).
    //
    // The exit code is a **UWORD** (D-005): EmuTOS `xterm(UWORD rc)`
    // (`bdos/proc.c:601,616`) stores `run->p_dreg[0] = rc`, i.e. the
    // low 16 bits **zero-extended** into the LONG the parent reads back
    // from `Pexec`. Mask to 16 bits here so `Pterm(-1)` delivers 65535,
    // `Pterm(0x12345)` delivers 0x2345 — never a sign-extended value.
    let rc: i32 = match func {
        0x00 => 0,
        0x31 => (frame.gpr[1] & 0xFFFF) as i32,
        _ => (frame.gpr[0] & 0xFFFF) as i32,
    };

    // SAFETY: trap-context single-CPU contract; the BDOS giant lock
    // is held implicitly while we execute the handler.
    let result = unsafe {
        match func {
            0x00 => tos_api::bdos::process::pterm::terminate_zero(),
            0x31 => tos_api::bdos::process::pterm::terminate_resident(rc),
            _ => tos_api::bdos::process::pterm::terminate(rc),
        }
    };

    finish_terminate(frame, rc, result);
}

/// Terminate the running process with exit code `rc` (cleanup via the
/// normal [`pterm::terminate`]) and redirect the frame to the parent —
/// used by the Ctrl-C self-terminate from blocking console input,
/// which records its request via `request_terminate` rather than a
/// `Pterm` SVC.
fn terminate_and_redirect(frame: &mut ExceptionFrame, rc: i32) {
    // SAFETY: trap context, single-CPU, BDOS lock held implicitly.
    let result = unsafe { tos_api::bdos::process::pterm::terminate(rc) };
    finish_terminate(frame, rc, result);
}

/// Common tail of every process-termination path (`Pterm` / `Pterm0` /
/// `Ptermres`, and the Ctrl-C self-terminate): copy the parent's frame
/// snapshot back over the trap frame, restore the parent's `TTBR0`,
/// release the slot, and let the post-handler `eret` resume the parent.
fn finish_terminate(
    frame: &mut ExceptionFrame,
    rc: i32,
    result: Result<
        tos_api::bdos::process::pterm::PtermResume,
        tos_api::bdos::process::pterm::PtermError,
    >,
) {
    use core::sync::atomic::Ordering;

    let resume = match result {
        Ok(r) => r,
        Err(_) => {
            // Misuse — leave the frame alone and report EINVFN so
            // the user's SVC observes a recoverable error.
            frame.gpr[0] = tos_types::error::EINVFN as i64 as u64;
            return;
        }
    };

    let pf = &tos_api::bdos::process::PROCESS_TABLE[resume.parent_frame_slot]
        .parent_frame;
    let pf_spsr = pf.spsr().load(Ordering::Acquire);
    let from_el0 = (pf_spsr & 0xF) == 0;

    // Copy the parent's frame snapshot over the current trap frame.
    // After this, the post-handler asm restore + eret resumes the
    // parent at exactly the state captured when `Pexec` redirected
    // EL0 (or, for the kernel-launch path, at the EL1 resume PC the
    // EL0-entry helper stamped into `parent_frame.elr`).
    //
    // SPSR.M discriminates the two cases:
    //   * `EL0t` (= 0b0000) — Phase 06 D3 `Pexec`: the parent is a
    //     real EL0 process. Full register restore so the parent's
    //     `x0..x29 / lr / sp_el0` are exactly what they were at the
    //     `Pexec` SVC, with `gpr[0]` overwritten to the child's
    //     exit code.
    //   * `EL1t` (= 0b0100) — kernel-launch (Phase 05 EC4 / D3 / D6
    //     selftests): the "parent" is the test driver function.
    //     **Only** restore `elr` / `sp_el0` / `spsr`. The kernel
    //     resume function's epilogue restores its callee-saved
    //     regs (x29, x30, …) from the local stack frame; copying
    //     zeros into `gpr` would clobber `x29` and crash the
    //     post-resume frame-pointer chain.
    if from_el0 {
        for i in 0..30 {
            frame.gpr[i] = pf.gpr(i).load(Ordering::Acquire);
        }
        frame.lr = pf.lr().load(Ordering::Acquire);
        // Exit code lands in parent's `x0` per the GEMDOS Pexec
        // ABI. Overwrite gpr[0] *after* the bulk copy so the
        // parent's pre-trap value doesn't shadow it.
        frame.gpr[0] = rc as i64 as u64;
    }
    frame.elr = pf.elr().load(Ordering::Acquire);
    frame.spsr = pf_spsr;
    frame.sp_el0 = pf.sp_el0().load(Ordering::Acquire);

    // Done with the slot's atomics — release it for re-use.
    // SAFETY: see `terminate` — caller-side BDOS-lock contract.
    unsafe {
        tos_api::bdos::process::pterm::release_slot(resume.parent_frame_slot);
    }

    // Bring the parent's address space back online before the eret.
    //   * `parent_l0_pa == 0` — the dying process was launched by the
    //     kernel boot path (Phase 06 D6 selftest, no real parent).
    //     Restore the Stage-1 boot tables; the resume is to EL1, so
    //     no QEMU wedge applies regardless.
    //   * `parent_l0_pa != 0` — a real parent called `Pexec`. Flush
    //     the TLB via boot-restore (TLBI is safe in kernel context),
    //     then `install_ttbr0` (no TLBI) so the parent's mappings are
    //     fresh-walked.
    //
    // SAFETY: trap context, single-CPU; no kernel code holds a
    // low-half VA reference across this point.
    //
    // The `kprintln!` between the boot-restore (TLBI) and the parent
    // install is INTERIM, QEMU-only, and LOAD-BEARING — see QUIRK-001
    // in docs/MEMORY.md. Parent and child share identical EL0 VAs
    // (both TPAs map at VA 0x100) under a shared ASID 0, so QEMU's
    // cortex-a72 TCG model reuses the *child's* cached translation
    // block for the parent's same-VA code across this TTBR0 switch —
    // the post-handler `eret`-to-EL0 then makes no forward progress
    // and the model wedges. A line of UART traffic (Device-memory
    // writes) between the TLBI and the install forces TCG to end the
    // translation block; a single Device *read* is NOT sufficient
    // (verified empirically), nor is pure arithmetic "distance". This
    // mirrors what `handle_pexec` already relies on. On real hardware
    // it is a harmless diagnostic line. Per-process ASIDs were tried
    // as a "proper" fix and made this QEMU model *worse* (it wedges on
    // the ASID-tagged TTBR0 write itself) — see QUIRK-001. The wedge
    // is a pure QEMU artifact, so this barrier IS the correct
    // long-term solution for the QEMU target; keep it.
    unsafe {
        crate::arch::aarch64::user_entry::restore_boot_ttbr0();
        if resume.parent_l0_pa != 0 {
            tos_api::bdos::process::mmu::install_ttbr0(resume.parent_l0_pa);
        }
    }
}

/// Read a NUL-terminated string from a user-space VA into `buf` using
/// **unprivileged** byte loads (`ldtrb`), returning the number of
/// bytes read (excluding the terminator). The unprivileged load makes
/// the access permission-checked as if EL0 issued it, which is the
/// correct contract for a TOS syscall pointer (it is a *user*
/// pointer) and mirrors [`tos_api::gemdos::console::cconws`]. The
/// caller's `TTBR0_EL1` must be active. The read is bounded by
/// `buf.len()`; a missing NUL simply truncates. A genuinely bad
/// pointer faults as an EL0 access — the caller's bug, just as a
/// ported program passing a wild `path` to `Pexec` would fault.
/// Read a NUL-terminated string from a **kernel** pointer (ordinary
/// loads). Used when an EL1 caller passes `Pexec` a kernel-VA path.
fn read_kernel_cstr(va: u64, buf: &mut [u8]) -> usize {
    let p = va as *const u8;
    let mut n = 0;
    while n < buf.len() {
        // SAFETY: bounded by buf.len(); provenance is the caller's path.
        let b = unsafe { p.add(n).read_volatile() };
        if b == 0 {
            break;
        }
        buf[n] = b;
        n += 1;
    }
    n
}

fn read_user_cstr(user_va: u64, buf: &mut [u8]) -> usize {
    let mut p = user_va;
    let mut n = 0;
    while n < buf.len() {
        let b: u8;
        // SAFETY: single unprivileged byte load; `readonly` + bounded
        // by `buf.len()`. Provenance is the user's `path` pointer.
        unsafe {
            core::arch::asm!(
                "ldtrb {b:w}, [{p}]",
                p = in(reg) p,
                b = out(reg) b,
                options(nostack, preserves_flags, readonly),
            );
        }
        if b == 0 {
            break;
        }
        buf[n] = b;
        n += 1;
        p += 1;
    }
    n
}

/// Byte length of the Pexec environment scratch buffer. Bounds a
/// caller-supplied environment block; generous versus real Atari
/// environments (a handful of `KEY=value` records). A larger block is
/// truncated to a double-NUL-terminated prefix.
const ENV_BUF_LEN: usize = 512;

/// Read a `Pexec` environment block (a sequence of NUL-terminated
/// records ended by a **double NUL**) from a pointer into `buf`,
/// returning the byte count **including** the terminating double NUL.
/// `from_el0` selects unprivileged (`ldtrb`, user pointer) vs ordinary
/// (kernel pointer) loads, exactly like the `path`/tail readers. On
/// overflow the last two bytes are forced to NUL so the copied env is
/// always double-NUL terminated.
fn read_env_block(va: u64, buf: &mut [u8], from_el0: bool) -> usize {
    let mut p = va;
    let mut n = 0;
    let mut prev_nul = false;
    while n < buf.len() {
        let b: u8 = if from_el0 {
            let v: u8;
            // SAFETY: single unprivileged byte load; bounded by buf.len().
            unsafe {
                core::arch::asm!(
                    "ldtrb {v:w}, [{p}]",
                    p = in(reg) p,
                    v = out(reg) v,
                    options(nostack, preserves_flags, readonly),
                );
            }
            v
        } else {
            // SAFETY: bounded by buf.len(); provenance is the caller's env.
            unsafe { (p as *const u8).read_volatile() }
        };
        buf[n] = b;
        n += 1;
        p += 1;
        if b == 0 {
            if prev_nul {
                return n; // double NUL: env complete, both NULs included
            }
            prev_nul = true;
        } else {
            prev_nul = false;
        }
    }
    // Overflow without a natural double-NUL terminator: force one so the
    // downstream verbatim copy stays well-formed.
    if n >= 2 {
        buf[n - 1] = 0;
        buf[n - 2] = 0;
    }
    n
}

/// Pexec (GEMDOS 0x4B) handler. Sets up a child process and
/// rewrites the trap frame so the post-handler `eret` lands in the
/// child at EL0 instead of returning to the parent.
///
/// On error the frame is left untouched except for `gpr[0]` which
/// receives the GEMDOS error code; the parent's SVC observes a
/// negative `Long`, just like every other GEMDOS error path.
fn handle_pexec(frame: &mut ExceptionFrame) {
    use core::sync::atomic::Ordering;

    use tos_api::bdos::process::loader::EnvSpec;
    use tos_api::bdos::process::pexec::{PE_LOAD, PE_LOADGO};

    let mode = frame.gpr[0] as u16;
    let path_user_va = frame.gpr[1];
    let tail_arg = frame.gpr[2];
    let env_arg = frame.gpr[3];
    let from_el0 = frame.spsr & 0xF == 0;

    // Read the NUL-terminated `path` string from the parent's user
    // space. The parent's TTBR0 is still active (we switch to the
    // child only after a successful load), so its user VAs resolve —
    // but we read through **unprivileged loads** (`ldtrb`, see
    // [`read_user_cstr`]) because `path` is a *user* pointer per the
    // TOS ABI: the access must be permission-checked as EL0, exactly
    // like `Cconws` reads its string (a kernel-VA pointer here would
    // correctly fault, and the future m68k emulator passes user VAs
    // too). A missing NUL is bounded by the buffer size; a bad
    // pointer faults as the caller's bug (Phase 09 adds the TPA
    // bounds check).
    let mut path_buf = [0u8; 128];
    // A user (EL0) caller passes a user-VA `path` read with unprivileged
    // loads; a kernel (EL1) caller — e.g. the built-in shell — passes a
    // kernel pointer that must be read with ordinary loads (an `ldtrb`
    // would fault as EL0 on a kernel page).
    let path_len = if from_el0 {
        read_user_cstr(path_user_va, &mut path_buf)
    } else {
        read_kernel_cstr(path_user_va, &mut path_buf)
    };
    let path_bytes = &path_buf[..path_len];

    // Command tail + environment. Only the **load** modes consume them;
    // for `PE_GO` the `x2` argument is the opaque PD handle (a value,
    // not a pointer) so we must not dereference it. `x2` = command-tail
    // image (`init_pd_fields`), `x3` = environment (`env == 0` → inherit
    // `run->p_env`). Both are read from the caller's address space with
    // the same EL0/EL1 discipline as `path`.
    let mut tail_buf = [0u8; 128];
    let mut env_buf = [0u8; ENV_BUF_LEN];
    let (tail_bytes, env_spec): (&[u8], EnvSpec) =
        if mode == PE_LOADGO || mode == PE_LOAD {
            let tlen = if tail_arg == 0 {
                0
            } else if from_el0 {
                read_user_cstr(tail_arg, &mut tail_buf)
            } else {
                read_kernel_cstr(tail_arg, &mut tail_buf)
            };
            let env_spec = if env_arg == 0 {
                EnvSpec::Inherit
            } else {
                let elen = read_env_block(env_arg, &mut env_buf, from_el0);
                EnvSpec::Explicit(&env_buf[..elen])
            };
            (&tail_buf[..tlen], env_spec)
        } else {
            (&[][..], EnvSpec::None)
        };

    // SAFETY: trap context, BDOS giant lock implicit, single-CPU.
    let result = unsafe {
        tos_api::bdos::process::pexec::dispatch(
            mode, path_bytes, tail_arg, tail_bytes, env_spec,
        )
    };

    let child = match result {
        Ok(c) => c,
        Err(e) => {
            use tos_api::bdos::process::loader::LoadError;
            use tos_api::bdos::process::pexec::PexecError;
            let err = match e {
                PexecError::UnknownMode | PexecError::UnsupportedMode => {
                    tos_types::error::EINVFN
                }
                PexecError::NotFound => tos_types::error::EFILNF,
                PexecError::NoSlot => tos_types::error::ENSMEM,
                PexecError::BadHandle => tos_types::error::EIMBA,
                // A *memory/size* shortfall maps to `ENSMEM` (EmuTOS
                // `bdos/proc.c:295-300`: no TPA → `ENSMEM`); a malformed
                // image / bad header maps to `EPLFMT` (`kpgmld.c:47-63`).
                // `ProgramTooLarge` is the image+BSS overrunning the TPA
                // image region — a size shortfall, so `ENSMEM`. A failed
                // TPA pool allocation (`NoTpaMemory`) and the env
                // allocation shortfall (`alloc_env`, `proc.c:284-288`)
                // are likewise `ENSMEM`.
                PexecError::LoadFailed(le) => match le {
                    LoadError::ProgramTooLarge
                    | LoadError::NoEnvMemory
                    | LoadError::NoTpaMemory => tos_types::error::ENSMEM,
                    _ => tos_types::error::EPLFMT,
                },
            };
            frame.gpr[0] = err as i64 as u64;
            return;
        }
    };

    // `PE_LOAD` (D4): the program is loaded but **not** launched. The
    // parent keeps running; it receives the opaque PD handle (the
    // child's `basepage_pa`) as the `Pexec` return value and hands it
    // back later to `PE_GO`. No TTBR0 switch, no frame redirect — just
    // return to the parent. The slot stays `Loaded` until `PE_GO`
    // launches it (or the parent frees it).
    if !child.launch {
        frame.gpr[0] = child.handle;
        return;
    }

    // Launch path (PE_LOADGO / PE_GO): redirect EL0 to the child.
    // Snapshot the parent's full trap frame into the child slot's
    // `parent_frame`. Pterm's full-frame restore (when SPSR.M=EL0t)
    // brings the parent back to exactly this state, with `gpr[0]`
    // overwritten to the child's exit code.
    let pf = &tos_api::bdos::process::PROCESS_TABLE[child.slot]
        .parent_frame;
    for i in 0..30 {
        pf.gpr(i).store(frame.gpr[i], Ordering::Release);
    }
    pf.lr().store(frame.lr, Ordering::Release);
    pf.elr().store(frame.elr, Ordering::Release);
    pf.spsr().store(frame.spsr, Ordering::Release);
    pf.sp_el0().store(frame.sp_el0, Ordering::Release);

    // Stamp the child slot's `parent_l0_pa` with the parent's
    // current `TTBR0_EL1`. Pterm's TTBR0 restore lands here on
    // exit so the parent's user mappings come back online.
    let parent_ttbr0: u64;
    // SAFETY: read-only system register access at EL1.
    unsafe {
        core::arch::asm!(
            "mrs {out}, ttbr0_el1",
            out = out(reg) parent_ttbr0,
            options(nomem, nostack, preserves_flags),
        );
    }
    tos_api::bdos::process::PROCESS_TABLE[child.slot]
        .parent_l0_pa
        .store(parent_ttbr0, Ordering::Release);

    // Record the kernel-side parent linkage (EmuTOS `proc_go`,
    // bdos/proc.c:541 — `p->p_parent = run`). `RUN` still points at
    // the parent here (we set it to the child below), so `current()`
    // yields the parent's basepage. `Pterm` reads this slot index to
    // restore `RUN = parent` when the child terminates (xterm's
    // `run = run->p_parent`).
    let parent_bp = tos_api::bdos::process::current();
    match tos_api::bdos::process::slot_for(parent_bp) {
        Some(parent_slot_idx) => {
            tos_api::bdos::process::PROCESS_TABLE[child.slot]
                .parent_slot
                .store(parent_slot_idx as u32, Ordering::Release);
        }
        None => {
            // No parent slot (boot-launched parent). Leave the
            // child's `parent_slot` at `NO_PARENT`; Pterm then clears
            // `RUN` to null on the child's exit.
        }
    }

    // Set RUN to the child's basepage so subsequent BDOS-pool
    // allocations from the child get tagged with the right owner
    // cookie (D9 wiring).
    let child_basepage =
        tos_api::bdos::process::pexec::child_basepage_kva(child.slot);
    tos_api::bdos::process::set_current(child_basepage);

    // Switch TTBR0 from parent to child *with* a TLBI. We are in
    // kernel context here; the eret to EL0 is many instructions
    // away (handler return + EXC_BODY restore), so the QEMU
    // cortex-a72 wedge condition does not apply.
    //
    // Switch TTBR0 from parent to child. We can NOT use
    // [`switch_ttbr0_kernel`] here even though we're in kernel
    // context: this handler returns through EXC_BODY which `eret`s
    // to EL0 ~50 instructions later, and the QEMU cortex-a72
    // wedge condition (TLBI followed by eret-to-EL0) still triggers
    // at that distance. Two-step instead:
    //   1. `restore_boot_ttbr0()` — flushes TLB while installing
    //      the (otherwise-unused) boot tables. Safe: the boot
    //      install path was the original "TLBI works in kernel
    //      context" carve-out.
    //   2. `install_ttbr0(child.l0_pa)` — no TLBI, but the TLB is
    //      now empty from step 1 so the walker does fresh walks
    //      via the new TTBR0.
    //
    // SAFETY: trap context, single-CPU; no kernel code holds a
    // low-half VA reference across this point.
    // QUIRK-001 (QEMU cortex-a72 TCG only): the model wedges unless
    // enough work (specifically UART-MMIO-bearing prints) sits between
    // the TLBI in `restore_boot_ttbr0` and the `eret` to EL0 below. The
    // two prints below are kept deliberately to bracket the TTBR0 switch
    // and avoid the wedge; real hardware is unaffected.
    unsafe {
        crate::arch::aarch64::user_entry::restore_boot_ttbr0();
    }
    crate::kprintln!("[exec] preparing program (slot {})", child.slot);
    unsafe {
        tos_api::bdos::process::mmu::install_ttbr0(child.l0_pa);
    }
    crate::kprintln!("[exec] launching");

    // Rewrite the current frame so EXC_BODY's `eret` lands at the
    // child entry in EL0, on the child's user stack. The child
    // receives its basepage VA in `x0` per NewTOS's AArch64
    // process ABI.
    for i in 0..30 {
        frame.gpr[i] = 0;
    }
    frame.gpr[0] = child.basepage_va;
    frame.lr = 0;
    frame.elr = child.entry_va;
    frame.spsr = 0; // SPSR.M = EL0t, DAIF = 0
    // Initial user stack = the mapped TPA top (multi-page TPA), 4-KiB /
    // 16-byte aligned. The SDK crt0 re-derives the same value from the
    // basepage's p_hitpa, so a compiled program lands on a mapped stack.
    frame.sp_el0 = child.sp_el0;
}

/// Catch-all for IRQ / FIQ / SError and unimplemented synchronous
/// vectors. PHASE-02 has no interrupt support; reaching here is a
/// configuration bug and we halt with a diagnostic.
#[unsafe(no_mangle)]
pub extern "C" fn rust_unhandled_handler(frame: &mut ExceptionFrame) {
    unhandled(frame, "unhandled");
}

fn unhandled(frame: &ExceptionFrame, kind: &str) -> ! {
    let ec = esr_ec(frame.esr);
    let far = if is_abort(ec) { Some(read_far_el1()) } else { None };

    // Stack-overflow recognition (Phase 03 D6): a Data Abort whose
    // FAR_EL1 falls inside the stack guard region means the running
    // kernel code stored below the bottom of its main stack. Print a
    // dedicated banner before the generic dump so the cause is
    // unmistakable in the boot log.
    if let Some(addr) = far {
        let guard_start = crate::bios::machine::layout::stack_guard_start();
        let guard_end = crate::bios::machine::layout::stack_guard_end();
        if addr >= guard_start && addr < guard_end {
            crate::kprintln!(
                "\n[EXC] *** STACK OVERFLOW *** FAR=0x{:016x} \
                 (guard 0x{:016x}..0x{:016x})",
                addr, guard_start, guard_end,
            );
            crate::kprintln!(
                "[EXC] ESR=0x{:016x} ELR=0x{:016x} SPSR=0x{:016x}",
                frame.esr, frame.elr, frame.spsr,
            );
            crate::arch::aarch64::halt();
        }
    }

    crate::kprintln!(
        "\n[EXC] {} exception ({}): ESR=0x{:016x} ELR=0x{:016x} SPSR=0x{:016x}",
        kind, ec_name(ec), frame.esr, frame.elr, frame.spsr,
    );
    if let Some(addr) = far {
        crate::kprintln!("[EXC] FAR=0x{:016x}", addr);
    }
    crate::arch::aarch64::halt();
}
