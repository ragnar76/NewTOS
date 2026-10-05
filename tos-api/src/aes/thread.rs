//! AES-on-a-fiber — step 1 of the "AES as kernel thread" design.
//!
//! Spec: `docs/analysis/coop-kernel/30-aes-thread-design.md`, §3.2 (fiber
//! switch over CPS), §3.3 (building blocks), §3.4(a) (the no-stall flow),
//! and §5 "Schritt 1".
//!
//! ## Why this module lives in `tos-api`, not `kernel`
//!
//! `kernel` depends on `tos-api` (the GEM-trap handler in
//! `kernel/src/arch/aarch64/exceptions.rs` calls `tos_api::aes::dispatch`),
//! and the eventual stall caller `ob_user` already lives in
//! `tos_api::aes::object`. Placing the fiber primitive here keeps the
//! dependency direction intact and co-locates the AArch64 asm with the
//! business logic that needs it — exactly the precedent set by
//! [`crate::bdos::process::mmu::install_ttbr0`] (TTBR0/TLBI asm in
//! `tos-api`). See design §3.3.
//!
//! ## What this module does
//!
//! Step 1 routes the AES trap dispatch through a dedicated, trap-persistent
//! EL1 fiber stack ([`AES_THREAD_STACK`]) instead of the shared exception
//! stack: [`call`] enters the fiber stack, runs [`crate::aes::dispatch`]
//! there as an ordinary call, and — when nothing stalls — returns its result
//! byte-identically to the pre-fiber path (design §3.4(a),
//! "Nullrisiko-Refactor für alle ~44 AES-Funktionen").
//!
//! Step 2 adds the synchronous `G_USERDEF` callout. When `ob_user` hits an
//! app-provided userdef, [`block_for_userdef`] fiber-switches out of the frozen
//! draw back into [`call`], which returns [`AesCallOutcome::Stalled`]; the
//! kernel GEM-trap handler runs the app's `ub_code` at EL0 (via the callout
//! stub in [`crate::bdos::process::loader`]) and calls [`resume`] with the WORD
//! result, which fiber-switches back in so the draw continues where it paused
//! (design §3.4(b)–(e)). [`fiber_switch`], [`AES_THREAD_CTX`],
//! [`AES_THREAD_STATE`] and the `AES_CALL_ORIGIN`/`PENDING_*` statics are the
//! primitives this flow is built on.

use crate::bdos::process::sched::Switch;
use crate::bdos::process::{Abi, ParentFrame};
use crate::uaccess::Space;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};

// ─── Fiber context ──────────────────────────────────────────────────

/// A saved fiber execution context: the AArch64 stack pointer plus the
/// AAPCS64 callee-saved registers `x19-x28`, the frame pointer `x29` and
/// the link register `x30` (ARM ARM / AAPCS64 §5.1.1). Thirteen `u64`s.
///
/// These are the *only* registers a cooperative EL1↔EL1 fiber switch must
/// preserve: caller-saved registers (`x0-x18`) are, by the calling
/// convention, already dead across the call that performs the switch, and
/// no privilege/address-space state is involved (no `TTBR0`, no
/// `ExceptionFrame`/`SPSR`/`ELR`) — this is pure EL1-internal bookkeeping,
/// the AArch64 analogue of EmuTOS' `switchto()` (design §3.2).
///
/// The fields are read and written **by byte offset from asm** (see
/// [`fiber_switch`] / `fiber_enter`), never by name from Rust; the
/// `#[repr(C)]` fixes the layout the asm offsets assume, and the
/// compile-time assertions below pin those offsets.
#[repr(C)]
#[allow(dead_code)] // fields are accessed by byte offset from the fiber asm
pub(crate) struct FiberCtx {
    /// Saved stack pointer (offset 0).
    sp: u64,
    /// Saved `x19-x28` (offset 8..88).
    x: [u64; 10],
    /// Saved `x29` / frame pointer (offset 88).
    x29: u64,
    /// Saved `x30` / link register (offset 96).
    x30: u64,
}

impl FiberCtx {
    /// A zeroed context (all registers 0). Valid as a *destination* for a
    /// save; a fresh context is never *resumed* until it has been saved
    /// into at least once.
    const fn zeroed() -> Self {
        Self { sp: 0, x: [0; 10], x29: 0, x30: 0 }
    }
}

// The `fiber_switch` / `fiber_enter` asm hard-codes these byte offsets;
// keep them in lockstep with `FiberCtx`'s `#[repr(C)]` layout.
const _: () = assert!(core::mem::offset_of!(FiberCtx, sp) == 0);
const _: () = assert!(core::mem::offset_of!(FiberCtx, x) == 8);
const _: () = assert!(core::mem::offset_of!(FiberCtx, x29) == 88);
const _: () = assert!(core::mem::offset_of!(FiberCtx, x30) == 96);
const _: () = assert!(core::mem::size_of::<FiberCtx>() == 13 * 8);

// ─── Fiber statics ──────────────────────────────────────────────────

/// Size of the dedicated AES fiber stack.
///
/// **OFFEN (design §3.3 / §6.1):** proposed 16 KiB, *not* a measured
/// value. Rationale: the main kernel stack is 64 KiB with a 16 KiB
/// emergency stack (`kernel/src/arch/aarch64/link-rpi4.ld`); the AES tree
/// traversal is shallower/leaner than the full kernel call stack, so 16
/// KiB (the emergency-stack size) is a plausible starting point. Whether a
/// guard page (as the main stack has) is needed is likewise OFFEN — step 1
/// exercises no deeper path than the pre-fiber dispatch already used on the
/// exception stack.
const AES_THREAD_STACK_SIZE: usize = 16 * 1024;

/// 16-byte-aligned backing storage for the AES fiber stack. AAPCS64
/// requires SP to be 16-byte aligned at a public interface; the `align(16)`
/// base plus the 16-KiB (multiple-of-16) size makes the computed top
/// 16-byte aligned.
#[repr(C, align(16))]
struct FiberStack([u8; AES_THREAD_STACK_SIZE]);

/// The AES fiber stack. Mutated only through `SP` while a `call` runs on
/// it; there is exactly **one** physical AES processing instance at a time
/// (design §3.3, EmuTOS' single-`rlr` invariant), so a single global stack
/// suffices — no per-process array.
static mut AES_THREAD_STACK: FiberStack = FiberStack([0; AES_THREAD_STACK_SIZE]);

/// Saved caller context at the point [`call`] switches onto the fiber
/// stack: "how to get back to the GEM-trap handler". In step 1 this is
/// captured and restored on the ordinary return; in step 2 it is the
/// resume target `block_for_userdef` switches to when it stalls.
static mut AES_ENTRY_CTX: FiberCtx = FiberCtx::zeroed();

/// Saved fiber context at a userdef stall: "how to get back into
/// `ob_user`'s suspended draw". Written by [`block_for_userdef`] (save side)
/// and read by [`resume`] (restore side).
static mut AES_THREAD_CTX: FiberCtx = FiberCtx::zeroed();

/// The fiber is idle — no AES call is currently suspended at a userdef.
const AES_STATE_IDLE: u8 = 0;
/// The fiber is suspended inside `ob_user` awaiting a userdef return: set on
/// the first stall of a call and cleared when the call finally completes. The
/// step-3 reentrancy guard reads this to reject a nested opcode-200 call.
const AES_STATE_SUSPENDED: u8 = 1;

/// AES fiber state: [`AES_STATE_IDLE`] or [`AES_STATE_SUSPENDED`]. Set to
/// SUSPENDED by [`block_for_userdef`], back to IDLE by [`outcome_after_switch`]
/// on completion.
static AES_THREAD_STATE: AtomicU8 = AtomicU8::new(AES_STATE_IDLE);

// ─── Stall bookkeeping (step 2) ─────────────────────────────────────

/// Sentinel for [`AES_CALL_ORIGIN_SLOT`]: no AES call currently has a saved
/// origin frame (no stall in flight).
pub const AES_ORIGIN_NONE: usize = usize::MAX;

/// Snapshot of the client's unchanged `wind_*`/`objc_*` trap frame, taken by
/// the GEM-trap handler on the **first** userdef stall of a call (design
/// §3.4(c)). When the AES fiber finally completes, the handler restores this
/// over the trap frame so the client resumes at exactly its pre-call state —
/// **not** the interrupted stub context. Global, not per-process: there is
/// exactly one AES processing instance at a time (design §3.3). Written/read
/// from the kernel GEM-trap handler via the [`ParentFrame`] accessors.
pub static AES_CALL_ORIGIN: ParentFrame = ParentFrame::zeroed();

/// Slot index whose frame is saved in [`AES_CALL_ORIGIN`], or
/// [`AES_ORIGIN_NONE`] when no call has an origin captured. Doubles as the
/// "first stall of this call" discriminator (design §3.4(c)): the handler
/// captures the origin only while this is [`AES_ORIGIN_NONE`], and clears it
/// back to [`AES_ORIGIN_NONE`] on completion.
pub static AES_CALL_ORIGIN_SLOT: AtomicUsize = AtomicUsize::new(AES_ORIGIN_NONE);

/// `true` between [`block_for_userdef`] switching out and the matching
/// [`outcome_after_switch`] observing it: the discriminator that tells `call`/
/// `resume` a *stall* (vs. a normal dispatch completion) brought control back.
static STALL_PENDING: AtomicBool = AtomicBool::new(false);

/// The current stall's `ub_code` EL0 VA — read into the [`UserdefStall`]
/// descriptor `call`/`resume` hand to the GEM-trap handler.
static PENDING_UB_CODE: AtomicU64 = AtomicU64::new(0);
/// The current stall's `PARMBLK` EL0 VA (the [`crate::bdos::process::loader`]
/// scratch VA passed to `ub_code`).
static PENDING_PARMBLK: AtomicU64 = AtomicU64::new(0);
/// The current stall's target slot (the tree owner). Single-client today it
/// equals `slot_for(current())`; recorded for the multi-client `NEWDESK_SLOT`
/// future (design §6.1.3).
static PENDING_TARGET_SLOT: AtomicUsize = AtomicUsize::new(AES_ORIGIN_NONE);

/// Result slot for [`fiber_dispatch_trampoline`]. A **static**, not a `call`
/// stack local: a userdef stall unwinds `call` early and the dispatch only
/// finishes later from [`resume`], by which time a stack local would be
/// dangling (its exception-stack frame reused). Written by the trampoline via
/// the pointer `call` hands `fiber_enter`; drained by [`outcome_after_switch`].
static mut AES_DISPATCH_RESULT: Option<Switch> = None;

// ─── Call outcome ───────────────────────────────────────────────────

/// The result of routing an AES trap through the fiber.
pub enum AesCallOutcome {
    /// The AES call finished; carry its `Option<Switch>` result to the
    /// GEM-trap handler unchanged (a blocking `evnt_*` wait may hand back the
    /// peer [`Switch`] to yield to; every other opcode returns `None`).
    Complete(Option<Switch>),
    /// The AES draw stalled at an app-provided `G_USERDEF`: the fiber is
    /// frozen mid-draw and the GEM-trap handler must run the app's `ub_code`
    /// at EL0 (via the callout stub) and feed the WORD result back through
    /// [`resume`] (design §3.4(c)).
    Stalled(UserdefStall),
}

/// Describes a single userdef stall for the GEM-trap handler to deliver
/// (design §3.4(c)): the app's `ub_code` EL0 VA and the `PARMBLK` EL0 VA the
/// kernel already populated in the scratch region. Both travel to the callout
/// stub as upcall arguments (`x0`/`x1`).
#[derive(Copy, Clone)]
pub struct UserdefStall {
    /// EL0 VA of the app's `WORD (*ub_code)(PARMBLK*)` callback.
    pub ub_code_va: u64,
    /// EL0 VA of the populated `PARMBLK` scratch (the argument to `ub_code`).
    pub parmblk_ptr: u64,
}

// ─── Entry point ────────────────────────────────────────────────────

/// Route one AES (GEM-trap selector 200) call through the dedicated fiber
/// stack (design §3.4(a)).
///
/// Saves the current EL1 context into [`AES_ENTRY_CTX`], switches `SP` to
/// the top of [`AES_THREAD_STACK`], runs [`crate::aes::dispatch`] there as
/// an ordinary call, then restores `SP` and returns
/// [`AesCallOutcome::Complete`] with dispatch's `Option<Switch>`. No fiber
/// *switch* is needed for the fresh entry — the entry is a plain call on a
/// relocated stack (the analogue of EmuTOS' `psetup` / our `Switch::fresh`,
/// design §3.4(a)) — and because there is no stall in step 1, this is
/// byte-identical to calling `dispatch` directly on the exception stack.
///
/// # Safety
///
/// `aespb_ptr` must be a valid AES parameter block of `abi`'s pointer width
/// — exactly [`crate::aes::dispatch`]'s contract, forwarded here.
pub unsafe fn call(aespb_ptr: u64, abi: Abi, space: Space) -> AesCallOutcome {
    // Reset the trampoline's result slot (a static — see `AES_DISPATCH_RESULT`).
    // SAFETY: single-CPU trap context; the fiber is not re-entrant (design §6.2).
    unsafe {
        AES_DISPATCH_RESULT = None;
    }
    // SAFETY: `fiber_enter` saves the current callee-saved context + SP into
    // `AES_ENTRY_CTX`, runs `fiber_dispatch_trampoline(aespb_ptr, abi,
    // &AES_DISPATCH_RESULT)` on the 16-byte-aligned top of `AES_THREAD_STACK`,
    // and — on either a normal completion (fiber epilogue) or a userdef stall
    // (`block_for_userdef` switching back to `AES_ENTRY_CTX`) — restores SP +
    // callee-saved + LR from `AES_ENTRY_CTX` before returning here. The pointers
    // are valid; `aespb_ptr`/`abi` are forwarded under this fn's safety contract.
    unsafe {
        fiber_enter(
            aespb_ptr,
            pack_abi_space(abi, space),
            &raw mut AES_DISPATCH_RESULT,
            fiber_stack_top(),
            &raw mut AES_ENTRY_CTX,
        );
    }
    outcome_after_switch()
}

/// The caller's [`Abi`] and [`Space`] in the one `u64` the fiber entry
/// passes to the trampoline (`x1`, `abi_raw`): the ABI in bits 0..8 (as
/// before), bit 8 set for [`Space::Kernel`] (ARCH03 stage 3a).
fn pack_abi_space(abi: Abi, space: Space) -> u64 {
    abi as u8 as u64 | if space == Space::Kernel { 1 << 8 } else { 0 }
}

/// The inverse of [`pack_abi_space`].
fn unpack_abi_space(raw: u64) -> (Abi, Space) {
    let space = if raw & (1 << 8) != 0 { Space::Kernel } else { Space::User };
    (Abi::from_u8(raw as u8), space)
}

/// Classify why control returned to `call`/`resume` from the fiber: a userdef
/// **stall** ([`STALL_PENDING`] set by [`block_for_userdef`]) yields
/// [`AesCallOutcome::Stalled`]; otherwise the dispatch ran to completion, so
/// the fiber is idle again and its `Option<Switch>` result is drained
/// (design §3.4(a)/(e)).
fn outcome_after_switch() -> AesCallOutcome {
    if STALL_PENDING.swap(false, Ordering::AcqRel) {
        AesCallOutcome::Stalled(UserdefStall {
            ub_code_va: PENDING_UB_CODE.load(Ordering::Acquire),
            parmblk_ptr: PENDING_PARMBLK.load(Ordering::Acquire),
        })
    } else {
        AES_THREAD_STATE.store(AES_STATE_IDLE, Ordering::Release);
        // Drain the result slot (read-and-clear) through the raw pointer so no
        // reference to the `static mut` is formed.
        // SAFETY: single-CPU trap context; the trampoline has finished writing.
        let sw = unsafe { core::ptr::replace(&raw mut AES_DISPATCH_RESULT, None) };
        AesCallOutcome::Complete(sw)
    }
}

/// Suspend the AES draw at an app `G_USERDEF` and hand the callout to the
/// GEM-trap handler (design §3.4(b)). Records the stall (`ub_code`/`parmblk`/
/// target slot), saves the current (fiber) context into [`AES_THREAD_CTX`], and
/// fiber-switches back to [`AES_ENTRY_CTX`] — i.e. into `call`/`resume`, which
/// then returns [`AesCallOutcome::Stalled`]. The intervening Rust frames
/// (`just_draw`/`draw_tree`/`paint_bg_clip`/…) stay frozen on the fiber stack.
/// When the kernel later calls [`resume`]`(word)`, control returns here and this
/// function returns `word` to `ob_user` — the frozen draw continues.
///
/// `target_slot` is the tree owner (single-client: `slot_for(current())`).
pub(crate) fn block_for_userdef(
    target_slot: usize,
    ub_code_va: u64,
    parmblk_ptr: u64,
) -> i16 {
    PENDING_UB_CODE.store(ub_code_va, Ordering::Release);
    PENDING_PARMBLK.store(parmblk_ptr, Ordering::Release);
    PENDING_TARGET_SLOT.store(target_slot, Ordering::Release);
    AES_THREAD_STATE.store(AES_STATE_SUSPENDED, Ordering::Release);
    // Publish the stall last (release) so `outcome_after_switch` sees the
    // descriptor fields once it observes the flag.
    STALL_PENDING.store(true, Ordering::Release);
    // SAFETY: saves this fiber context into `AES_THREAD_CTX` and resumes
    // `AES_ENTRY_CTX` (a live `call`/`resume` frame on the exception stack).
    // `resume` later restores `AES_THREAD_CTX` with the WORD in x0.
    let word = unsafe {
        fiber_switch(&raw mut AES_THREAD_CTX, &raw const AES_ENTRY_CTX, 0)
    };
    word as u16 as i16
}

/// Resume the AES fiber after the EL0 `ub_code` returned `word` (design
/// §3.4(e)). Fiber-switches back into [`AES_THREAD_CTX`] (the parked
/// [`block_for_userdef`]), delivering `word` in x0; the frozen draw continues
/// until it either stalls again (another `G_USERDEF` → [`AesCallOutcome::Stalled`])
/// or completes ([`AesCallOutcome::Complete`]).
///
/// Called by the kernel `Psigreturn` handler with the WORD read from the
/// userdef ret-slot.
pub fn resume(word: i16) -> AesCallOutcome {
    // SAFETY: saves this (`resume`) context into `AES_ENTRY_CTX` — the target
    // the fiber returns to on its next stall/completion — and resumes the parked
    // `AES_THREAD_CTX`, delivering `word` (zero-extended) as its x0.
    unsafe {
        fiber_switch(
            &raw mut AES_ENTRY_CTX,
            &raw const AES_THREAD_CTX,
            word as u16 as u64,
        );
    }
    outcome_after_switch()
}

/// Exclusive top of the AES fiber stack (base + size). 16-byte aligned by
/// construction (see [`FiberStack`]).
fn fiber_stack_top() -> u64 {
    (&raw mut AES_THREAD_STACK as *mut u8 as usize + AES_THREAD_STACK_SIZE) as u64
}

/// The trampoline `fiber_enter` calls once `SP` is on the fiber stack: run
/// `dispatch` and store its `Option<Switch>` through `out`.
///
/// A separate `extern "C"` function (rather than inlining the call into the
/// naked entry) keeps the `Option<Switch>` return-value ABI entirely in
/// compiler-managed code — the asm only ever moves `u64`s and a pointer.
///
/// # Safety
///
/// Called only from `fiber_enter` with `SP` on the fiber stack. `aespb_ptr`
/// must satisfy [`crate::aes::dispatch`]'s contract; `out` must be a valid,
/// writable `*mut Option<Switch>` — [`call`] passes `&raw mut
/// AES_DISPATCH_RESULT`, a static that stays live even when a userdef stall
/// unwinds `call` and the dispatch only completes later from [`resume`].
unsafe extern "C" fn fiber_dispatch_trampoline(
    aespb_ptr: u64,
    abi_raw: u64,
    out: *mut Option<Switch>,
) {
    let (abi, space) = unpack_abi_space(abi_raw);
    // SAFETY: `aespb_ptr`/`abi` satisfy `dispatch`'s contract (forwarded from
    // `call`); `dispatch` now runs on the fiber stack.
    let sw = unsafe { super::dispatch_in(aespb_ptr, abi, space) };
    // SAFETY: `out` is `AES_DISPATCH_RESULT` (a live static), passed by `call`.
    unsafe { out.write(sw) };
}

/// Fresh entry onto the fiber stack (design §3.4(a)): save the current
/// callee-saved context + SP + LR into `*entry_ctx`, switch `SP` to
/// `new_sp`, call `fiber_dispatch_trampoline(aespb_ptr, abi_raw, out)`, and
/// on its normal return restore SP + callee-saved + LR from `*entry_ctx`
/// and return to the caller.
///
/// The context is captured at the call boundary, so `*entry_ctx.x30` is the
/// return address into [`call`]; step 2's `block_for_userdef` resumes
/// `*entry_ctx` via [`fiber_switch`] and lands at the identical point in
/// [`call`], where it will inspect [`AES_THREAD_STATE`] to return `Stalled`.
/// The step-1 normal-return path here reaches that same point — hence
/// byte-identical for the no-stall case.
///
/// Register ABI (AAPCS64): `x0`=aespb_ptr, `x1`=abi_raw, `x2`=out,
/// `x3`=new_sp, `x4`=entry_ctx.
///
/// # Safety
///
/// `new_sp` must be a 16-byte-aligned, exclusive top of a stack with enough
/// headroom for the whole AES dispatch; `entry_ctx` must be a valid,
/// writable `*mut FiberCtx`; `out`/`aespb_ptr`/`abi_raw` are forwarded to
/// `fiber_dispatch_trampoline` under its contract.
#[unsafe(naked)]
unsafe extern "C" fn fiber_enter(
    aespb_ptr: u64,
    abi_raw: u64,
    out: *mut Option<Switch>,
    new_sp: u64,
    entry_ctx: *mut FiberCtx,
) {
    core::arch::naked_asm!(
        // Save callee-saved regs + fp + lr + current SP into *entry_ctx (x4).
        "stp x19, x20, [x4, #8]",
        "stp x21, x22, [x4, #24]",
        "stp x23, x24, [x4, #40]",
        "stp x25, x26, [x4, #56]",
        "stp x27, x28, [x4, #72]",
        "stp x29, x30, [x4, #88]",
        "mov x9, sp",
        "str x9, [x4, #0]",
        // Keep the ctx pointer across the trampoline call in a callee-saved
        // register (the trampoline preserves x19 per AAPCS64).
        "mov x19, x4",
        // Switch to the dedicated AES fiber stack.
        "mov sp, x3",
        // Run aes::dispatch on the fiber stack. x0/x1/x2 already carry
        // aespb_ptr / abi_raw / out.
        "bl {tramp}",
        // No-stall (step 1) return: restore SP + callee-saved + LR from
        // *entry_ctx, then return into `call`.
        "mov x4, x19",
        "ldr x9, [x4, #0]",
        "mov sp, x9",
        "ldp x19, x20, [x4, #8]",
        "ldp x21, x22, [x4, #24]",
        "ldp x23, x24, [x4, #40]",
        "ldp x25, x26, [x4, #56]",
        "ldp x27, x28, [x4, #72]",
        "ldp x29, x30, [x4, #88]",
        "ret",
        tramp = sym fiber_dispatch_trampoline,
    )
}

/// Cooperative EL1↔EL1 fiber switch (design §3.2/§3.3): save the current
/// callee-saved context (`x19-x28`, `x29`, `x30`) and `SP` into
/// `*save_into`, load them from `*resume_from`, and return into the resumed
/// context — the AArch64 analogue of EmuTOS' `switchto()`.
///
/// `resume_x0` is loaded into `x0` immediately before returning into the
/// resumed context, so it becomes the *return value* observed there by the
/// matching `fiber_switch`/`fiber_enter` call. This is the word-delivery
/// channel: [`resume`] passes the userdef WORD as `resume_x0`, and the parked
/// [`block_for_userdef`] receives it as its `fiber_switch` return value. A
/// caller that does not care what the resumed side observes (e.g.
/// [`block_for_userdef`] switching back into [`call`], whose `fiber_enter`
/// return is `()`) passes `0`.
///
/// # Safety
///
/// `save_into` must be a valid, writable `*mut FiberCtx`; `resume_from`
/// must point at a `FiberCtx` previously saved by a matching switch/enter
/// (its SP/LR/callee-saved describe a live, resumable EL1 context on a
/// stack that is still allocated).
#[unsafe(naked)]
pub(crate) unsafe extern "C" fn fiber_switch(
    save_into: *mut FiberCtx,
    resume_from: *const FiberCtx,
    resume_x0: u64,
) -> u64 {
    core::arch::naked_asm!(
        // Save current context into *save_into (x0).
        "stp x19, x20, [x0, #8]",
        "stp x21, x22, [x0, #24]",
        "stp x23, x24, [x0, #40]",
        "stp x25, x26, [x0, #56]",
        "stp x27, x28, [x0, #72]",
        "stp x29, x30, [x0, #88]",
        "mov x9, sp",
        "str x9, [x0, #0]",
        // Restore context from *resume_from (x1).
        "ldr x9, [x1, #0]",
        "mov sp, x9",
        "ldp x19, x20, [x1, #8]",
        "ldp x21, x22, [x1, #24]",
        "ldp x23, x24, [x1, #40]",
        "ldp x25, x26, [x1, #56]",
        "ldp x27, x28, [x1, #72]",
        "ldp x29, x30, [x1, #88]",
        // Deliver the message word (x2) as x0 in the resumed context, then
        // return into it (its saved x30).
        "mov x0, x2",
        "ret",
    )
}
