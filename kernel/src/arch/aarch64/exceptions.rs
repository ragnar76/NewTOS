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
use core::sync::atomic::Ordering;

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
// Rust-level handlers (called from exception_vectors.s)
// ---------------------------------------------------------------------------

/// Synchronous-exception handler. Decodes ESR_EL1; if it is an SVC
/// from a recognized class, dispatches into `tos-api`. Updates
/// `frame.gpr[0]` so the assembly stub returns the value in `x0`.
#[unsafe(no_mangle)]
pub extern "C" fn rust_sync_handler(frame: &mut ExceptionFrame) {
    let ec = esr_ec(frame.esr);
    if ec != EC_SVC_AARCH64 && ec != EC_SVC_FROM_EL1 {
        unhandled(frame, "synchronous (non-SVC)");
    }

    let svc_imm = (esr_iss(frame.esr) & 0xFFFF) as u16;
    if frame.spsr & 0xF == 0 && svc_imm == 1 {
        crate::kprintln!(
            "[svc] func=0x{:x} elr=0x{:x}",
            frame.gpr[8] as u16, frame.elr,
        );
    }

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

    // GEMDOS Pterm (0x4C) / Pterm0 (0x00) and Pexec (0x4B) need the
    // trap frame mutated rather than just an `x0` return — the
    // dispatcher table can't express that, so we special-case them
    // here.
    if matches!(class, TrapClass::Gemdos) {
        let func = frame.gpr[8] as u16;
        if func == 0x4C || func == 0x00 {
            handle_pterm(frame, func);
            return;
        }
        if func == 0x4B {
            handle_pexec(frame);
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

    let result = match class {
        TrapClass::Gemdos => tos_api::gemdos::dispatch(&args),
        // PHASE-07 will land BIOS/XBIOS dispatchers; until then any
        // SVC #13/#14 returns EINVFN so the trap path is observable.
        TrapClass::Bios | TrapClass::Xbios => tos_types::error::EINVFN,
    };

    // Sign-extend `i32` into `x0` so callers reading `i64` see the
    // correct negative value for error codes.
    frame.gpr[0] = result as i64 as u64;
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
    use core::sync::atomic::Ordering;

    // Capture the user's exit code BEFORE `terminate` runs — for
    // `Pterm` (0x4C) it lives in the user's `x0` (= `frame.gpr[0]`);
    // for `Pterm0` (0x00) it is fixed at `0`.
    let rc: i32 = if func == 0x00 {
        0
    } else {
        frame.gpr[0] as i32
    };

    // SAFETY: trap-context single-CPU contract; the BDOS giant lock
    // is held implicitly while we execute the handler.
    let result = unsafe {
        if func == 0x00 {
            tos_api::bdos::process::pterm::terminate_zero()
        } else {
            tos_api::bdos::process::pterm::terminate(rc)
        }
    };

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
            crate::kprintln!("[pterm] resume parent (rc={})", rc);
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

/// Pexec (GEMDOS 0x4B) handler. Sets up a child process and
/// rewrites the trap frame so the post-handler `eret` lands in the
/// child at EL0 instead of returning to the parent.
///
/// On error the frame is left untouched except for `gpr[0]` which
/// receives the GEMDOS error code; the parent's SVC observes a
/// negative `Long`, just like every other GEMDOS error path.
fn handle_pexec(frame: &mut ExceptionFrame) {
    use core::sync::atomic::Ordering;

    let mode = frame.gpr[0] as u16;
    let path_user_va = frame.gpr[1];
    let tail = frame.gpr[2];
    let env = frame.gpr[3];
    crate::kprintln!(
        "[pexec] entered: mode={} path_va=0x{:x}",
        mode, path_user_va,
    );

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
    let path_len = read_user_cstr(path_user_va, &mut path_buf);
    let path_bytes = &path_buf[..path_len];
    crate::kprintln!(
        "[pexec] path read from user VA 0x{:x}: {} byte(s)",
        path_user_va, path_len,
    );

    // SAFETY: trap context, BDOS giant lock implicit, single-CPU.
    let result = unsafe {
        tos_api::bdos::process::pexec::dispatch(
            mode, path_bytes, tail, env,
        )
    };
    crate::kprintln!("[pexec] dispatch returned");

    let child = match result {
        Ok(c) => {
            crate::kprintln!(
                "[pexec] child loaded: slot={} entry=0x{:x} l0_pa=0x{:x}",
                c.slot, c.entry_va, c.l0_pa,
            );
            c
        }
        Err(e) => {
            use tos_api::bdos::process::pexec::PexecError;
            let err = match e {
                PexecError::UnknownMode | PexecError::UnsupportedMode => {
                    tos_types::error::EINVFN
                }
                PexecError::NotFound => tos_types::error::EFILNF,
                PexecError::NoSlot => tos_types::error::ENSMEM,
                PexecError::LoadFailed(_) => tos_types::error::EPLFMT,
            };
            frame.gpr[0] = err as i64 as u64;
            return;
        }
    };

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
    crate::kprintln!("[pexec] snapshotted parent frame");

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
    crate::kprintln!("[pexec] parent_l0_pa stored: 0x{:x}", parent_ttbr0);

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
            crate::kprintln!(
                "[pexec] parent_slot wired: child={} parent={}",
                child.slot, parent_slot_idx,
            );
        }
        None => {
            // No parent slot (boot-launched parent). Leave the
            // child's `parent_slot` at `NO_PARENT`; Pterm then clears
            // `RUN` to null on the child's exit.
            crate::kprintln!("[pexec] parent_slot: none (boot parent)");
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
    crate::kprintln!("[pexec] flushing TLB via boot restore");
    unsafe {
        crate::arch::aarch64::user_entry::restore_boot_ttbr0();
    }
    crate::kprintln!("[pexec] installing child ttbr0 0x{:x}", child.l0_pa);
    unsafe {
        tos_api::bdos::process::mmu::install_ttbr0(child.l0_pa);
    }
    crate::kprintln!("[pexec] ttbr0 switched");

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
    frame.sp_el0 = crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
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
