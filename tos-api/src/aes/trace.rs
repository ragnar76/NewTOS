//! AES trap-path tracer — a non-perturbing instrument for the AES
//! conversation between a GEM application and NewTOS.
//!
//! `docs/TOS-REFERENCE-DUMPER.md` §10 assumes a dispatcher log exists ("obtain
//! the list by logging the trap dispatcher under NewTOS, since the dispatcher
//! is yours there"). This is that log. It records every AES call that reaches
//! [`crate::aes::dispatch`] — opcode, the `control` counts, the `int_in` words
//! the caller passed, the full dispatch-local `int_out` (including `RET_CODE`,
//! `include/aesdefs.h:140`) and the `addr_in[0]`/`addr_out[0]` pointers — plus
//! the GEM messages posted into and delivered out of the per-application FIFO.
//!
//! ## Off by default, zero cost when off
//!
//! The whole instrument is behind the **`aes-trace` cargo feature**. With the
//! feature off, [`ENABLED`] is `false`, every entry point below has an empty
//! body, and the ring buffer static does not exist — no code, no BSS. Build it
//! in with `--features aes-trace` (kernel: `cargo xtask build --board rpi4
//! --release --features aes-trace`).
//!
//! ## Why it does not change what it measures
//!
//! Both TeraDesk symptoms this was built for are timing-sensitive (a
//! double-click window; pointer motion), so the capture path must not do I/O
//! and must not format text:
//!
//! * **Capture is a struct store.** One `CNTPCT_EL0` read at call entry, one at
//!   call exit, a bounded copy of at most 16 `int_in` + 7 `int_out` WORDs into a
//!   fixed 112-byte record in a static ring, and an index bump. No allocation,
//!   no `core::fmt`, no device access, no loop over unbounded data.
//! * **Formatting happens on the host.** The file written to `C:` is the raw
//!   ring plus a 256-byte ASCII header; `tools/aestrace.py` renders it. This is
//!   the `docs/TOS-REFERENCE-DUMPER.md` §3 rule ("no formatting happens on the
//!   target") applied to a live trace.
//! * **The write happens when the user is idle, never during input.** A flush is
//!   *requested* only after an `evnt_*` wait that blocked for at least
//!   `IDLE_FLUSH_US` (3 s) — i.e. the user's hands were off the mouse — and
//!   at most once per `MIN_FLUSH_GAP_US` (20 s). The earlier per-`evnt_multi`
//!   flush of `dbg_qedtrace` froze QED on real hardware; this one cannot fire
//!   inside a click window or a hover burst.
//! * **The flush runs on the kernel exception stack, not the AES fiber stack.**
//!   The AES dispatch only sets a flag ([`request_flush`]); the GEM-trap handler
//!   calls [`flush_if_requested`] after the dispatch has returned. The FAT write
//!   path is far deeper than the 16 KiB AES fiber stack should be asked to hold.
//! * **It never waits on console input.** QUIRK-002 (the PL011 RX interrupt is
//!   not delivered on a real Pi 4) killed an earlier reference dumper that did;
//!   nothing here reads the console.
//!
//! The instrument measures its own cost: every hook accumulates the ticks it
//! spent, and the header reports the total, so "cheap enough" is a number in
//! the file rather than a claim in a comment.
//!
//! ## Getting the trace off the machine
//!
//! [`flush_on_exit`] rewrites `C:\AESTRACE.BIN` in full (kernel-internal
//! [`fcreate`](crate::bdos::fs::file::fcreate)/[`fwrite`](crate::bdos::fs::file::fwrite),
//! not a nested `svc`), so the file always holds the newest complete trace and
//! survives the application exiting — it runs from `Pterm`
//! before the process is torn down. The ASCII header states the ring capacity,
//! how many records the file holds and how many were **ever** produced, so a
//! wrapped (truncated) trace is visible in the first 256 bytes of the file
//! without any tool.

// Without the `aes-trace` feature only the empty hook wrappers survive. The
// record layout, the tick source and the size constants stay compiled in that
// configuration on purpose — so the format cannot rot silently and the layout
// assertion still runs — but nothing uses them, so dead-code is allowed for
// that configuration only.
#![cfg_attr(not(feature = "aes-trace"), allow(dead_code))]

use super::Aespb;

/// Whether the tracer is compiled in. `false` folds every hook — and every
/// argument expression at a call site guarded by this — out of the build.
pub const ENABLED: bool = cfg!(feature = "aes-trace");

// ─── Record format (little-endian, native AArch64 order) ─────────────

/// Record kind — the `kind` byte at offset 52.
#[allow(dead_code)]
mod kind {
    /// An AES dispatch that ran to completion (or aborted into a peer
    /// yield — see [`super::flags::YIELDED`]).
    pub const CALL: u8 = 1;
    /// A GEM message enqueued into an application's FIFO
    /// (`pd::post_msg` / `pd::post_words`).
    pub const MSG_POST: u8 = 2;
    /// A GEM message actually delivered to the application by
    /// `evnt_mesag` / `evnt_multi` / `appl_read`.
    pub const MSG_RECV: u8 = 3;
    /// A tracer marker (trace start, flush point).
    pub const MARK: u8 = 4;
}

/// `flags` bits — the byte at offset 53.
#[allow(dead_code)]
mod flags {
    /// The call aborted into a cooperative peer yield (S2 restartable
    /// block): no result was produced and the `svc` will re-execute.
    pub const YIELDED: u8 = 0x01;
    /// The caller's `int_in` pointer was null — no input words captured.
    pub const NO_INTIN: u8 = 0x02;
    /// A `MSG_POST` that the FIFO rejected (queue full).
    pub const POST_FAILED: u8 = 0x04;
    /// The record was produced by an in-kernel (EL1) caller, i.e. there is
    /// no user process — informational duplicate of `slot == SLOT_KERNEL`.
    pub const KERNEL_CALLER: u8 = 0x08;
}

/// `slot` value meaning "no user process — an in-kernel (EL1) GEM caller".
const SLOT_KERNEL: u8 = 0xFF;

/// Words of `int_in`/`int_out`/message payload a record can hold. The AES
/// maximum is `evnt_multi`: 16 `int_in` + 7 `int_out` = 23.
const W_LEN: usize = 24;

/// Largest `int_in` prefix captured (the AES maximum, `evnt_multi`).
const MAX_IN: usize = 16;

/// `O_SIZE` — the AES `int_out` array, 7 WORDs (`include/aesdefs.h:123`).
/// The whole local array is captured, not just the `out_len` prefix the
/// caller declared, so "the manager wrote a slot the caller never asked
/// for" is visible.
const O_SIZE: usize = 7;

/// One trace record. **Fixed 112-byte little-endian layout**, mirrored by
/// `tools/aestrace.py`; do not reorder fields without updating the decoder.
///
/// | off | size | field |
/// |----:|-----:|-------|
/// | 0 | 4 | `seq` — record number, gapless across a wrap |
/// | 4 | 4 | `passes` — predicate evaluations inside a blocking `evnt_*` |
/// | 8 | 8 | `t_ticks` — `CNTPCT_EL0` at hook entry |
/// | 16 | 8 | `dur_ticks` — call entry→exit |
/// | 24 | 8 | `addr_in0` |
/// | 32 | 8 | `addr_out0` |
/// | 40 | 2 | `opcode` (`MSG_*`: the message type `msg[0]`) |
/// | 42 | 2 | `pid` — AES process id |
/// | 44 | 2 | `in_len` = `control[1]` |
/// | 46 | 2 | `out_len` = `control[2]` |
/// | 48 | 2 | `ain_len` = `control[3]` |
/// | 50 | 2 | reserved |
/// | 52 | 1 | `kind` |
/// | 53 | 1 | `flags` |
/// | 54 | 1 | `slot` — GEMDOS process slot, `0xFF` = in-kernel caller |
/// | 55 | 1 | `nin` — `int_in` words in `w` |
/// | 56 | 1 | `nout` — `int_out` words in `w`, after the `nin` |
/// | 50 | 2 | `start_x` — pointer x at call entry |
/// | 52 | 1 | `kind` |
/// | 53 | 1 | `flags` |
/// | 54 | 1 | `slot` — GEMDOS process slot, `0xFF` = in-kernel caller |
/// | 55 | 1 | `nin` — `int_in` words in `w` |
/// | 56 | 1 | `nout` — `int_out` words in `w`, after the `nin` |
/// | 57 | 1 | `mb_or` — OR of the button byte over all samples |
/// | 58 | 1 | `mb_and` — AND of the button byte over all samples |
/// | 59 | 1 | pad |
/// | 60 | 48 | `w[24]` |
/// | 108 | 4 | `travel` — sum of `|dx|+|dy|` over the whole call |
/// | 112 | 4 | `moves` — samples in which the pointer had moved |
/// | 116 | 4 | `win_travel` — same, but only inside the double-click window |
/// | 120 | 4 | `win_passes` — samples taken inside the double-click window |
/// | 124 | 2 | `start_y` — pointer y at call entry |
/// | 126 | 1 | `max_step` — largest single-sample `|dx|` or `|dy|`, capped 255 |
/// | 127 | 1 | `win_max_step` — same, inside the double-click window |
///
/// The four `win_*` fields and `max_step` exist to answer one specific
/// question against EmuTOS `mchange` (`aes/geminput.c:360-370`), which zeroes
/// a pending button wait when the pointer moves more than ±2: **did the
/// pointer move during the double-click window, and by how much per step?**
/// A `win_max_step` above 2 on a window that stayed open is that comparison,
/// measured rather than argued.
#[repr(C)]
#[derive(Clone, Copy)]
struct Rec {
    seq: u32,
    passes: u32,
    t_ticks: u64,
    dur_ticks: u64,
    addr_in0: u64,
    addr_out0: u64,
    opcode: i16,
    pid: i16,
    in_len: i16,
    out_len: i16,
    ain_len: i16,
    start_x: i16,
    kind: u8,
    flags: u8,
    slot: u8,
    nin: u8,
    nout: u8,
    mb_or: u8,
    mb_and: u8,
    _pad0: u8,
    w: [i16; W_LEN],
    travel: u32,
    moves: u32,
    win_travel: u32,
    win_passes: u32,
    start_y: i16,
    max_step: u8,
    win_max_step: u8,
}

impl Rec {
    const ZERO: Rec = Rec {
        seq: 0,
        passes: 0,
        t_ticks: 0,
        dur_ticks: 0,
        addr_in0: 0,
        addr_out0: 0,
        opcode: 0,
        pid: 0,
        in_len: 0,
        out_len: 0,
        ain_len: 0,
        start_x: 0,
        kind: 0,
        flags: 0,
        slot: 0,
        nin: 0,
        nout: 0,
        mb_or: 0,
        mb_and: 0,
        _pad0: 0,
        w: [0; W_LEN],
        travel: 0,
        moves: 0,
        win_travel: 0,
        win_passes: 0,
        start_y: 0,
        max_step: 0,
        win_max_step: 0,
    };
}

/// On-disk record size. Pinned so the host decoder and the target agree.
pub const RECORD_SIZE: usize = 128;
const _: () = assert!(core::mem::size_of::<Rec>() == RECORD_SIZE);

/// Total records held (`CAPACITY * RECORD_SIZE` = 896 KiB of BSS, present
/// only with the `aes-trace` feature).
///
/// The buffer is **split in two** (see [`PROLOGUE`]): a one-shot prologue
/// that keeps the *first* records of the run, and a ring that keeps the
/// *newest*. A trace that overruns therefore loses only the middle, and the
/// header says exactly how much.
pub const CAPACITY: usize = 8192;

/// Records reserved for the **prologue**: filled once, from the first AES
/// call of the run, and never overwritten. The remaining
/// `CAPACITY - PROLOGUE` records are the ring.
///
/// This exists because the obvious answer to "the ring wrapped and we lost
/// the beginning" — flush on the first wrap — cannot be used here: under the
/// failure mode being investigated the wrap arrives in the middle of a click
/// or a hover, which is precisely where a write must never land. Keeping the
/// opening records instead delivers the same data (how the conversation
/// started) and defers the write to the next safe moment.
pub const PROLOGUE: usize = 2048;
const _: () = assert!(PROLOGUE < CAPACITY);

/// Opaque handle returned by [`call_begin`] and consumed by [`call_end`] —
/// the `CNTPCT_EL0` value at dispatch entry. Zero-sized information when the
/// tracer is compiled out.
#[derive(Clone, Copy)]
pub struct CallStart(u64);

// ─── Time source ─────────────────────────────────────────────────────

/// Read the ARM generic timer's physical count (`CNTPCT_EL0`, ARM ARM D17.6).
/// This is the same counter [`crate::host::Host::monotonic_millis`] derives
/// from, but raw: no 64-bit divide on the capture path. Conversion to
/// microseconds happens in the host decoder using the `CNTFRQ_EL0` value
/// recorded in the file header.
#[inline(always)]
fn ticks() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let c: u64;
        // SAFETY: read-only system-register read, always permitted at EL1.
        unsafe {
            core::arch::asm!("mrs {}, cntpct_el0", out(reg) c,
                options(nomem, nostack, preserves_flags));
        }
        c
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        0
    }
}

/// Generic-timer frequency in Hz (`CNTFRQ_EL0`) — 54 MHz on a Pi 4,
/// 62.5 MHz under QEMU. Recorded in the file header so the decoder can
/// turn ticks into microseconds.
#[inline]
fn tick_hz() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let f: u64;
        // SAFETY: read-only system-register read, always permitted at EL1.
        unsafe {
            core::arch::asm!("mrs {}, cntfrq_el0", out(reg) f,
                options(nomem, nostack, preserves_flags));
        }
        f
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        0
    }
}

// ─── Disabled build: every hook is empty ─────────────────────────────

#[cfg(not(feature = "aes-trace"))]
mod imp {
    use super::{Aespb, CallStart};

    #[inline(always)]
    pub(super) fn call_begin() -> CallStart {
        CallStart(0)
    }
    #[inline(always)]
    pub(super) fn call_end(_pb: &Aespb, _t0: CallStart, _yielded: bool) {}
    #[inline(always)]
    pub(super) fn note_eval() {}
    #[inline(always)]
    pub(super) fn note_click_window() {}
    #[inline(always)]
    pub(super) fn msg_post(_pid: i16, _src: &[i16], _words: usize, _ok: bool) {}
    #[inline(always)]
    pub(super) fn request_flush() {}
    #[inline(always)]
    pub(super) fn flush_if_requested() {}
    #[inline(always)]
    pub(super) fn flush() {}
}

// ─── Enabled build ───────────────────────────────────────────────────

#[cfg(feature = "aes-trace")]
mod imp {
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

    use super::{
        flags, kind, ticks, tick_hz, Aespb, CallStart, Rec, CAPACITY, MAX_IN, O_SIZE,
        PROLOGUE, RECORD_SIZE, SLOT_KERNEL, W_LEN,
    };

    /// The longest AES double-click window, `gl_dcrates[0]` = 450 ms
    /// (`aes/gemevlib.c`, mirrored in `event.rs`'s `DCRATES`). **Every**
    /// flush-trigger quiet threshold below is a multiple of this, because
    /// "the write never lands inside a gesture" is exactly the statement
    /// "more time has passed with the mouse untouched than the longest
    /// window in which two separate presses still count as one gesture".
    const MAX_DCLICK_US: u64 = 450_000;

    /// No mouse **button edge** for this long before the deliberate
    /// both-Shift request may fire — 600 ms, **derived** from
    /// [`MAX_DCLICK_US`] rather than written out, so the "a write can never
    /// land inside a click, nor between the two presses of a double click"
    /// property is enforced by construction and cannot drift away from
    /// `gl_dcrates[0]` in a later edit.
    const MANUAL_BTN_QUIET_US: u64 = MAX_DCLICK_US * 4 / 3;

    /// No mouse **button edge** for this long before the automatic safety-net
    /// flush may fire — 11x [`MAX_DCLICK_US`], just under 5 s.
    const AUTO_BTN_QUIET_US: u64 = MAX_DCLICK_US * 11;

    // Both thresholds must outlast the longest window in which two separate
    // presses still count as one gesture; that is the whole no-mid-gesture
    // argument, so it is asserted rather than trusted.
    const _: () = assert!(MANUAL_BTN_QUIET_US > MAX_DCLICK_US);
    const _: () = assert!(AUTO_BTN_QUIET_US > MANUAL_BTN_QUIET_US);

    /// Pointer-travel budget, in `|dx|+|dy|` pixels accumulated over one
    /// [`TRAVEL_BUCKET_US`] window, below which the pointer counts as *not
    /// being moved*.
    ///
    /// This is not a quiet test. The machine under investigation has a mouse
    /// sensitive enough to register air movement, so the pointer literally
    /// never stops and "wait for stillness" can never fire. What separates
    /// that noise from a deliberate hover is **amplitude**: a few pixels of
    /// jitter per half second versus tens or hundreds when a hand moves the
    /// mouse. The automatic trigger uses the amplitude; it does not wait for
    /// zero.
    const TRAVEL_QUIET_PX: u32 = 12;
    /// Length of the travel-accumulation window.
    const TRAVEL_BUCKET_US: u64 = 500_000;

    /// How long both Shift keys must be held down before the manual request
    /// arms, so a Shift-click or a shifted keystroke cannot trip it.
    const SHIFT_HOLD_US: u64 = 700_000;

    /// Minimum wall-clock gap between two automatic (safety-net) flushes.
    const MIN_FLUSH_GAP_US: u64 = 10_000_000;

    /// Minimum gap between two manual (both-Shift) flushes, so holding the
    /// keys down does not rewrite the file continuously.
    const MANUAL_FLUSH_GAP_US: u64 = 3_000_000;

    /// `Kbshift` bits for right + left shift (`bios/ikbd.c`, `MODE_RSHIFT` /
    /// `MODE_LSHIFT`); both together is the manual flush request.
    const BOTH_SHIFT: u8 = 0x03;

    /// The ring buffer plus its cursors. Touched only from the single-CPU,
    /// IRQ-off GEM-trap / `Pterm` path — the same discipline the AES
    /// `pd`/`window` statics use — and additionally serialised against
    /// itself by [`IN_HOOK`].
    struct Ring {
        recs: UnsafeCell<[Rec; CAPACITY]>,
        /// Total records ever produced. `next % CAPACITY` is the write slot;
        /// `next > CAPACITY` means the ring has wrapped (records dropped).
        next: AtomicU64,
    }
    // SAFETY: see the type comment — single-CPU trap path, re-entrancy-guarded.
    unsafe impl Sync for Ring {}

    static RING: Ring = Ring {
        recs: UnsafeCell::new([Rec::ZERO; CAPACITY]),
        next: AtomicU64::new(0),
    };

    /// Re-entrancy guard. The flush calls the FS layer (plain kernel
    /// functions, not `svc`), so it cannot re-enter the AES dispatch; the
    /// guard is defensive and makes any nested hook a harmless no-op.
    static IN_HOOK: AtomicBool = AtomicBool::new(false);

    /// `CNTPCT_EL0` at the first record — the trace epoch.
    static T0: AtomicU64 = AtomicU64::new(0);
    /// Ticks spent inside the **capture** hooks, so the file can state the
    /// instrument's own cost on the timing-critical path instead of asserting
    /// it is small. Excludes the flush, which is accounted separately in
    /// [`FLUSH_TICKS`] — mixing the two would hide the number that matters.
    static OVERHEAD: AtomicU64 = AtomicU64::new(0);
    /// Number of capture-hook invocations [`OVERHEAD`] is spread over.
    static HOOKS: AtomicU64 = AtomicU64::new(0);
    /// Worst single capture hook, in ticks. A mean alone cannot distinguish
    /// "systematically expensive" from "one host-scheduling outlier under
    /// QEMU", and the whole no-perturbation claim rests on that difference.
    static OVERHEAD_MAX: AtomicU64 = AtomicU64::new(0);
    /// Ticks spent inside the flush (file write), counted apart from the
    /// capture path.
    static FLUSH_TICKS: AtomicU64 = AtomicU64::new(0);

    /// A flush has been requested and will run at the next
    /// [`flush_if_requested`] on the kernel exception stack.
    static FLUSH_REQ: AtomicBool = AtomicBool::new(false);
    /// `CNTPCT_EL0` of the last completed flush (0 = never).
    static LAST_FLUSH: AtomicU64 = AtomicU64::new(0);
    /// Flushes completed so far.
    static FLUSHES: AtomicU32 = AtomicU32::new(0);
    /// `RING.next` at the last completed flush — "is there anything new to
    /// write" without re-reading the file.
    static FLUSHED_AT: AtomicU64 = AtomicU64::new(0);

    /// Last observed mouse **button** byte, and the `CNTPCT_EL0` at which it
    /// last changed. This carries the whole no-mid-gesture guarantee: a press
    /// and its release are both edges, and the two presses of a double click
    /// are at most [`MAX_DCLICK_US`] apart, so requiring "no button edge for
    /// longer than [`MAX_DCLICK_US`]" is exactly the statement "we are not
    /// inside a click, and not inside the window in which a further press
    /// would still be counted as part of one".
    static LAST_MB: AtomicU32 = AtomicU32::new(u32::MAX);
    static LAST_BUTTON_CHANGE: AtomicU64 = AtomicU64::new(0);
    /// Pointer travel accumulated in the current [`TRAVEL_BUCKET_US`] window,
    /// the previous window's total, and when the current one started.
    static TRAVEL_ACC: AtomicU32 = AtomicU32::new(0);
    static TRAVEL_PREV: AtomicU32 = AtomicU32::new(u32::MAX);
    static TRAVEL_BUCKET_START: AtomicU64 = AtomicU64::new(0);
    /// `CNTPCT_EL0` at which both Shift keys most recently became held
    /// (0 = not currently held).
    static SHIFT_SINCE: AtomicU64 = AtomicU64::new(0);

    /// Per-call sampling accumulators, reset by [`call_begin`] and folded into
    /// the record by [`call_end`]. They answer, without emitting a record per
    /// spin, two questions the call boundary cannot:
    ///
    /// * what button byte did the wait actually read, on **every** one of its
    ///   evaluations (`mb_or` / `mb_and` / `passes`), and
    /// * did the pointer move while the wait ran, and by how much per step
    ///   (`travel` / `moves` / `max_step`) — separately for the whole call and
    ///   for the **double-click window** inside `count_clicks` (`win_*`),
    ///   which is the interval EmuTOS `mchange` (`aes/geminput.c:360-370`)
    ///   cancels on a >±2 move.
    ///
    /// **This is the only tracer state on a hot loop** — a blocking
    /// `evnt_multi` re-evaluates its predicate millions of times per second in
    /// the single-process case — so it is deliberately *not* atomic: plain
    /// loads/stores under the same single-CPU, IRQ-off GEM-trap discipline as
    /// [`Ring`] and the AES `pd`/`window` statics. Atomic read-modify-writes
    /// per pass would be the one place this instrument could plausibly move
    /// the thing it measures; plain stores through one address are not.
    #[derive(Clone, Copy)]
    struct EvalData {
        passes: u32,
        win_passes: u32,
        travel: u32,
        win_travel: u32,
        moves: u32,
        samp_x: i16,
        samp_y: i16,
        start_x: i16,
        start_y: i16,
        mb_or: u8,
        mb_and: u8,
        max_step: u8,
        win_max_step: u8,
    }
    impl EvalData {
        const RESET: EvalData = EvalData {
            passes: 0,
            win_passes: 0,
            travel: 0,
            win_travel: 0,
            moves: 0,
            samp_x: 0,
            samp_y: 0,
            start_x: 0,
            start_y: 0,
            mb_or: 0,
            mb_and: 0xFF,
            max_step: 0,
            win_max_step: 0,
        };
    }
    struct Eval(UnsafeCell<EvalData>);
    // SAFETY: see the type comment — single-CPU GEM-trap path only.
    unsafe impl Sync for Eval {}
    static EVAL: Eval = Eval(UnsafeCell::new(EvalData::RESET));

    /// `MU_MESAG` — the `evnt_multi` bit that says a message was delivered
    /// (`include/aesdefs.h`).
    const MU_MESAG: u16 = 0x0010;

    /// Convert a microsecond budget into generic-timer ticks.
    #[inline]
    fn us_to_ticks(us: u64) -> u64 {
        let hz = tick_hz();
        if hz == 0 {
            return u64::MAX; // no timer: never trip a time-based threshold
        }
        ((us as u128 * hz as u128) / 1_000_000) as u64
    }

    /// Claim the hook. Returns `false` if a hook is already running (nested
    /// call — dropped rather than corrupting the ring).
    #[inline]
    fn enter() -> bool {
        !IN_HOOK.swap(true, Ordering::AcqRel)
    }

    /// Release the hook, charging the elapsed ticks to the capture budget.
    #[inline]
    fn leave(t_in: u64) {
        let d = ticks().wrapping_sub(t_in);
        OVERHEAD.fetch_add(d, Ordering::Relaxed);
        HOOKS.fetch_add(1, Ordering::Relaxed);
        if d > OVERHEAD_MAX.load(Ordering::Relaxed) {
            OVERHEAD_MAX.store(d, Ordering::Relaxed);
        }
        IN_HOOK.store(false, Ordering::Release);
    }

    /// Release the hook, charging the elapsed ticks to the **flush** budget.
    #[inline]
    fn leave_flush(t_in: u64) {
        FLUSH_TICKS.fetch_add(ticks().wrapping_sub(t_in), Ordering::Relaxed);
        IN_HOOK.store(false, Ordering::Release);
    }

    /// Append `r`, stamping its sequence number.
    ///
    /// The first [`PROLOGUE`] records land in the prologue slots and stay
    /// there for the whole run; everything after cycles through the ring,
    /// overwriting the oldest. Nothing is ever lost silently: each record
    /// carries its `seq`, so the decoder sees the discontinuity, and the
    /// header reports produced-vs-held.
    fn push(mut r: Rec) {
        let seq = RING.next.fetch_add(1, Ordering::Relaxed);
        if seq == 0 {
            T0.store(r.t_ticks, Ordering::Relaxed);
        }
        r.seq = seq as u32;
        let idx = if (seq as usize) < PROLOGUE {
            seq as usize
        } else {
            PROLOGUE + ((seq - PROLOGUE as u64) % (CAPACITY - PROLOGUE) as u64) as usize
        };
        // SAFETY: single-CPU trap path serialised by `IN_HOOK`; `idx` is
        // either below `PROLOGUE` or reduced modulo the ring length, so it is
        // inside the record array either way.
        unsafe {
            (*RING.recs.get())[idx] = r;
        }
    }

    /// Read WORD `i` of a caller WORD array, tolerating a null base.
    #[inline]
    fn wrd(base: *const i16, i: usize) -> i16 {
        if base.is_null() {
            return 0;
        }
        // SAFETY: `i` is bounded by the opcode's declared array length (the
        // dispatcher reads the same array at the same indices); the array is
        // a mapped parameter-block VA.
        unsafe { base.add(i).read() }
    }

    /// The GEMDOS process slot of the current caller, or [`SLOT_KERNEL`] for
    /// an in-kernel (EL1) GEM caller — the desktop and the boot selftests
    /// run with a null `RUN` basepage.
    #[inline]
    fn caller_slot() -> u8 {
        match crate::bdos::process::slot_for(crate::bdos::process::current()) {
            Some(i) if i < 0xFF => i as u8,
            _ => SLOT_KERNEL,
        }
    }

    pub(super) fn call_begin() -> CallStart {
        let t = ticks();
        let (x, y) = crate::vdi::mouse_position();
        // SAFETY: single-CPU GEM-trap path (see `Eval`).
        unsafe {
            let e = &mut *EVAL.0.get();
            *e = EvalData::RESET;
            e.samp_x = x;
            e.samp_y = y;
            e.start_x = x;
            e.start_y = y;
        }
        CallStart(t)
    }

    /// One sample of the live input state: the mouse button byte and the
    /// pointer position, folded into the per-call accumulators. `in_window`
    /// selects the double-click-window set (`count_clicks`) over the general
    /// wait set. See [`Eval`] for why this is not atomic.
    #[inline(always)]
    fn sample(in_window: bool) {
        let mb = crate::vdi::mouse_buttons();
        let (x, y) = crate::vdi::mouse_position();
        // SAFETY: single-CPU GEM-trap path (see `Eval`); every counter
        // saturates so a very long wait cannot wrap one into a misleadingly
        // small number.
        unsafe {
            let e = &mut *EVAL.0.get();
            e.mb_or |= mb;
            e.mb_and &= mb;
            let dx = (x as i32 - e.samp_x as i32).unsigned_abs();
            let dy = (y as i32 - e.samp_y as i32).unsigned_abs();
            e.samp_x = x;
            e.samp_y = y;
            let step = dx.max(dy).min(255) as u8;
            let d = dx.saturating_add(dy);
            if d != 0 {
                e.moves = e.moves.saturating_add(1);
                e.travel = e.travel.saturating_add(d);
                if step > e.max_step {
                    e.max_step = step;
                }
            }
            if in_window {
                e.win_passes = e.win_passes.saturating_add(1);
                if d != 0 {
                    e.win_travel = e.win_travel.saturating_add(d);
                    if step > e.win_max_step {
                        e.win_max_step = step;
                    }
                }
            } else {
                e.passes = e.passes.saturating_add(1);
            }
        }
    }

    /// One predicate evaluation of a blocking `evnt_*` wait.
    #[inline(always)]
    pub(super) fn note_eval() {
        sample(false);
    }

    /// One poll inside the **double-click window** (`count_clicks` →
    /// `wait_until`) — the interval in which a second press still counts as
    /// part of the first gesture, and the interval EmuTOS `mchange`
    /// (`aes/geminput.c:360-370`) cancels when the pointer moves more than
    /// ±2. Recorded separately so the window can be seen opening and closing
    /// against the movement that should or should not have closed it.
    #[inline(always)]
    pub(super) fn note_click_window() {
        sample(true);
    }

    pub(super) fn call_end(pb: &Aespb, t0: CallStart, yielded: bool) {
        if !enter() {
            return;
        }
        let t_in = ticks();

        let control = pb.control;
        let opcode = wrd(control, 0);
        let in_len = wrd(control, 1);
        let out_len = wrd(control, 2);
        let ain_len = wrd(control, 3);

        let mut r = Rec::ZERO;
        r.kind = kind::CALL;
        r.t_ticks = t0.0;
        r.dur_ticks = t_in.wrapping_sub(t0.0);
        r.opcode = opcode;
        r.in_len = in_len;
        r.out_len = out_len;
        r.ain_len = ain_len;
        r.pid = super::super::appl::current_pid();
        r.slot = caller_slot();
        if r.slot == SLOT_KERNEL {
            r.flags |= flags::KERNEL_CALLER;
        }
        // SAFETY: single-CPU GEM-trap path (see `Eval`).
        unsafe {
            let e = *EVAL.0.get();
            r.passes = e.passes;
            r.win_passes = e.win_passes;
            r.mb_or = e.mb_or;
            r.mb_and = e.mb_and;
            r.travel = e.travel;
            r.win_travel = e.win_travel;
            r.moves = e.moves;
            r.max_step = e.max_step;
            r.win_max_step = e.win_max_step;
            r.start_x = e.start_x;
            r.start_y = e.start_y;
        }

        // `int_in[0 .. min(in_len, 16)]` — exactly what the caller declared,
        // capped at the AES maximum (`evnt_multi`'s 16 WORDs). Read out of the
        // **dispatch-local** buffer, i.e. the words the manager actually saw
        // after `Aespb::from_raw`'s `min(IN_LEN, I_SIZE)` copy
        // (`aes/gemsuper.c:436-437`), not out of the caller's array.
        let nin = if in_len <= 0 { 0 } else { (in_len as usize).min(MAX_IN) };
        if pb.caller_intin.is_null() {
            r.flags |= flags::NO_INTIN;
        } else {
            for i in 0..nin {
                r.w[i] = wrd(pb.intin, i);
            }
            r.nin = nin as u8;
        }

        // The **whole** dispatch-local `int_out[O_SIZE]`, not just the
        // `out_len` prefix: what the manager wrote is the interesting fact,
        // what the caller receives follows from `out_len`. A yielded call
        // produced no result at all (`dispatch` skips the copy-back).
        if yielded {
            r.flags |= flags::YIELDED;
        } else {
            let base = r.nin as usize;
            for i in 0..O_SIZE {
                if base + i < W_LEN {
                    r.w[base + i] = wrd(pb.intout, i);
                }
            }
            r.nout = O_SIZE as u8;
        }

        // `addr_in[0]` (OBJECT tree / message buffer / string) and
        // `addr_out[0]` (rsrc_gaddr & co.), at the caller's pointer width.
        // Like `int_in`, the address slot comes from the dispatch-local
        // `addr_in[AI_SIZE]` — the pointer the manager was handed after the
        // `min(AIN_LEN, AI_SIZE)` copy (`aes/gemsuper.c:438-439`), already
        // widened from the caller's ABI.
        if ain_len > 0 {
            r.addr_in0 = pb.addrin(0);
        }
        // `addr_out[0]` is read only for the one opcode our AES actually
        // writes it for — `rsrc_gaddr` (112), the sole `set_addrout` caller
        // (`aes/resource.rs:1286`). Reading it unconditionally would
        // dereference a pointer the dispatcher itself never touches for the
        // other ~50 opcodes, i.e. the instrument would add a fault surface to
        // the very run it is measuring. Everything else records 0.
        if pb.addrout != 0 && opcode == 112 {
            // The caller's `addr_out` array, which `rsrc_gaddr` writes through
            // `set_addrout` on this same call — read the way it was written,
            // in the caller's space (ARCH03 3a); nothing on a fault.
            r.addr_out0 = pb.abi.read_ptr_in(pb.space, pb.addrout, 0).unwrap_or(0);
        }

        let ret = wrd(pb.intout, 0);
        push(r);

        // A message actually delivered to the application: `evnt_mesag` (23)
        // and `appl_read` (11) on a return of 1, `evnt_multi` (25) when
        // `MU_MESAG` is among the fired bits. The buffer is `addr_in[0]` in
        // all three (`ME_PBUFF` / `AP_PBUFF` / `MME_PBUFF`).
        if !yielded && r.addr_in0 != 0 {
            let delivered = match opcode {
                23 | 11 => ret == 1,
                25 => (ret as u16) & MU_MESAG != 0,
                _ => false,
            };
            if delivered {
                let mut m = Rec::ZERO;
                m.kind = kind::MSG_RECV;
                m.t_ticks = t_in;
                m.pid = r.pid;
                m.slot = r.slot;
                m.addr_in0 = r.addr_in0;
                // The 16-byte message buffer the manager just wrote, read in
                // the caller's space (ARCH03 3a); a word that cannot be read
                // (none: the manager wrote it) is recorded as 0.
                for i in 0..8 {
                    m.w[i] = pb.space.get_u16(r.addr_in0 + 2 * i as u64).unwrap_or(0) as i16;
                }
                m.nin = 8;
                m.opcode = m.w[0]; // the message type, for a quick scan
                push(m);
            }
        }

        maybe_request_flush(t_in, r.travel);
        leave(t_in);
    }

    /// A GEM message enqueued into `pid`'s FIFO. `ok` is the enqueue result
    /// (`false` = queue full, the message was rejected).
    pub(super) fn msg_post(pid: i16, src: &[i16], words: usize, ok: bool) {
        if !enter() {
            return;
        }
        let t_in = ticks();
        let mut r = Rec::ZERO;
        r.kind = kind::MSG_POST;
        r.t_ticks = t_in;
        r.pid = pid;
        r.slot = caller_slot();
        r.in_len = words as i16;
        if !ok {
            r.flags |= flags::POST_FAILED;
        }
        let n = words.min(8).min(src.len());
        r.w[..n].copy_from_slice(&src[..n]);
        r.nin = n as u8;
        r.opcode = if n > 0 { src[0] } else { 0 };
        push(r);
        leave(t_in);
    }

    // NOTE: the real `maybe_request_flush` follows further down in this
    // module; this marker line is removed by the next edit.
    /// Decide, at the end of every AES call, whether this is a safe moment to
    /// write the trace. `travel` is the pointer movement accumulated during
    /// the call that just ended.
    ///
    /// **Two earlier designs failed here, and both failures shape this one.**
    ///
    /// 1. "Did an `evnt_*` wait just block for 3 s?" inferred idleness from
    ///    the AES having *stopped*. Unusable for the very defect it was built
    ///    to measure: if `evnt_multi` returns an event immediately on every
    ///    call, no wait ever blocks, so the instrument never armed — the bug
    ///    switched off its own measurement.
    /// 2. "Has the mouse been completely unchanged for 2 s?" assumed quiet
    ///    exists. On the machine under investigation the mouse is sensitive
    ///    enough to register air movement, so the pointer never stops and that
    ///    condition can never be true either.
    ///
    /// What is left, and what this uses: the **mouse buttons**, which really
    /// are quiet between gestures, plus pointer-movement **amplitude** to tell
    /// jitter from a hand on the mouse.
    ///
    /// **Why a write can never land inside a click gesture.**
    /// [`LAST_BUTTON_CHANGE`] is reset by every button edge — press and
    /// release alike. Both triggers require `buttons == 0` *now* and no edge
    /// for longer than [`MAX_DCLICK_US`] (450 ms, `gl_dcrates[0]`, the longest
    /// interval in which a second press still joins the first gesture). So the
    /// write cannot occur while a button is down, and cannot occur between the
    /// two presses of a double click.
    ///
    /// **What is *not* mechanically guaranteed, and is procedural instead.**
    /// Because pointer motion is permanent noise on this hardware, "never
    /// during pointer movement" is not achievable by waiting. The automatic
    /// trigger therefore also demands that recent travel stay under
    /// [`TRAVEL_QUIET_PX`], which excludes a hand moving the mouse but not a
    /// slow drift; and the primary path is the **manual** both-Shift request,
    /// which puts the moment entirely under the user's control — they simply
    /// do not press Shift in the middle of a hover test.
    fn maybe_request_flush(now: u64, travel: u32) {
        let mb = crate::vdi::mouse_buttons();
        if LAST_MB.swap(mb as u32, Ordering::Relaxed) != mb as u32 {
            LAST_BUTTON_CHANGE.store(now, Ordering::Relaxed);
        }
        let btn_quiet = now.wrapping_sub(LAST_BUTTON_CHANGE.load(Ordering::Relaxed));

        // Roll the pointer-travel bucket. `TRAVEL_PREV` starts at `u32::MAX`
        // so no automatic flush can fire before one full window has been
        // measured.
        let acc = TRAVEL_ACC.load(Ordering::Relaxed).saturating_add(travel);
        let bstart = TRAVEL_BUCKET_START.load(Ordering::Relaxed);
        if bstart == 0 || now.wrapping_sub(bstart) >= us_to_ticks(TRAVEL_BUCKET_US) {
            TRAVEL_PREV.store(acc, Ordering::Relaxed);
            TRAVEL_ACC.store(0, Ordering::Relaxed);
            TRAVEL_BUCKET_START.store(now, Ordering::Relaxed);
        } else {
            TRAVEL_ACC.store(acc, Ordering::Relaxed);
        }

        // Nothing new since the last write? Then there is nothing to gain and
        // a needless SD write to lose.
        let produced = RING.next.load(Ordering::Relaxed);
        if produced == 0 || produced <= FLUSHED_AT.load(Ordering::Relaxed) {
            return;
        }
        let last = LAST_FLUSH.load(Ordering::Relaxed);
        let since_flush = now.wrapping_sub(last);

        // ── Primary: the user asks for it, by holding both Shift keys ──
        // Deterministic, and the only trigger guaranteed to be reachable on
        // hardware where the pointer never rests.
        let shift = crate::host::host()
            .map(|h| h.keyboard_shift_state())
            .unwrap_or(0);
        if shift & BOTH_SHIFT == BOTH_SHIFT {
            let since = SHIFT_SINCE.load(Ordering::Relaxed);
            if since == 0 {
                SHIFT_SINCE.store(now, Ordering::Relaxed);
            } else if now.wrapping_sub(since) >= us_to_ticks(SHIFT_HOLD_US)
                && mb == 0
                && btn_quiet >= us_to_ticks(MANUAL_BTN_QUIET_US)
                && (last == 0 || since_flush >= us_to_ticks(MANUAL_FLUSH_GAP_US))
            {
                FLUSH_REQ.store(true, Ordering::Release);
                return;
            }
        } else {
            SHIFT_SINCE.store(0, Ordering::Relaxed);
        }

        // ── Safety net: no button touched for 5 s and the pointer is only
        // drifting, not being moved. Gives the user a file even if the
        // keyboard is unusable or they forget the Shift request.
        if mb == 0
            && btn_quiet >= us_to_ticks(AUTO_BTN_QUIET_US)
            && TRAVEL_PREV.load(Ordering::Relaxed) <= TRAVEL_QUIET_PX
            && (last == 0 || since_flush >= us_to_ticks(MIN_FLUSH_GAP_US))
        {
            FLUSH_REQ.store(true, Ordering::Release);
        }
    }

    pub(super) fn request_flush() {
        FLUSH_REQ.store(true, Ordering::Release);
    }

    pub(super) fn flush_if_requested() {
        if FLUSH_REQ.swap(false, Ordering::AcqRel) {
            flush();
        }
    }

    // ─── The file ────────────────────────────────────────────────────

    /// Sink for the trace. Absolute so it does not depend on the running
    /// process's current drive/directory.
    const PATH: &[u8] = b"C:\\AESTRACE.BIN\0";

    /// Companion status file, written **after** `AESTRACE.BIN` is closed.
    /// Its whole purpose is to make the card unambiguous without a tool and
    /// without a console:
    ///
    /// * neither file present → the trigger never fired (or `C:` is not
    ///   writable);
    /// * `AESTRACE.BIN` but no `AESTRACE.TXT` → the write was interrupted;
    /// * both present → the trace was written completely, and the `.TXT` says
    ///   in plain words how many records it holds and whether it wrapped.
    ///
    /// It is also readable **at the machine**: it is a short text file in the
    /// root of `C:`, so re-reading that directory in the application under
    /// test shows it appear, and its size grows with every flush.
    const STATUS_PATH: &[u8] = b"C:\\AESTRACE.TXT\0";

    /// Header length in bytes — one ASCII line, space-padded, `\n`-terminated.
    /// 256 so every field still fits at its widest (a `produced`/`t0` after a
    /// long session is 12-13 digits); the builder truncates rather than
    /// overflowing, and a truncated header would silently lose `hz`.
    const HDR_LEN: usize = 256;

    /// Bytes reserved for the human-readable status file.
    const STATUS_LEN: usize = 640;

    /// A tiny fixed-capacity ASCII builder, used for both the binary file's
    /// header line and the status file. No `core::fmt`, so this stays as dumb
    /// (and as auditable) as the rest of the path. Appends are silently
    /// dropped once full — it can never overflow.
    struct Asc<const N: usize> {
        buf: [u8; N],
        len: usize,
    }
    impl<const N: usize> Asc<N> {
        fn new(fill: u8) -> Self {
            Asc { buf: [fill; N], len: 0 }
        }
        fn s(&mut self, s: &[u8]) {
            for &b in s {
                if self.len < N {
                    self.buf[self.len] = b;
                    self.len += 1;
                }
            }
        }
        fn u(&mut self, mut v: u64) {
            let mut d = [0u8; 20];
            let mut n = 0;
            if v == 0 {
                d[0] = b'0';
                n = 1;
            }
            while v > 0 {
                d[n] = b'0' + (v % 10) as u8;
                v /= 10;
                n += 1;
            }
            while n > 0 {
                n -= 1;
                if self.len < N {
                    self.buf[self.len] = d[n];
                    self.len += 1;
                }
            }
        }
        /// The full fixed-size buffer, with the last byte forced to `\n` —
        /// the binary file's header is a fixed-width field, not a string.
        fn fixed(&mut self) -> &[u8] {
            self.buf[N - 1] = b'\n';
            &self.buf
        }
        /// Only the bytes written so far — for the free-form status file.
        fn written(&self) -> &[u8] {
            &self.buf[..self.len]
        }
    }

    /// Rewrite `C:\AESTRACE.BIN` with the header and the records in
    /// chronological order, then write the human-readable `C:\AESTRACE.TXT`.
    /// Runs on the kernel exception stack (never the 16 KiB AES fiber stack).
    /// Every failure is swallowed — the instrument must never hang or fault
    /// the machine it is measuring.
    pub(super) fn flush() {
        if !enter() {
            return;
        }
        let t_in = ticks();

        let produced = RING.next.load(Ordering::Relaxed);
        let held = produced.min(CAPACITY as u64);
        let dropped = produced.saturating_sub(held);
        // Prologue records actually present, and the ring's share of `held`.
        let pro = produced.min(PROLOGUE as u64);
        let ring_held = held - pro;

        let mut h = Asc::<HDR_LEN>::new(b' ');
        h.s(b"NEWTOS-AESTRACE v3 le rec=");
        h.u(RECORD_SIZE as u64);
        h.s(b" cap=");
        h.u(CAPACITY as u64);
        h.s(b" pro=");
        h.u(PROLOGUE as u64);
        h.s(b" n=");
        h.u(held);
        h.s(b" produced=");
        h.u(produced);
        h.s(if dropped > 0 { b" WRAPPED drop=" } else { b" COMPLETE drop=" });
        h.u(dropped);
        h.s(b" hz=");
        h.u(tick_hz());
        h.s(b" t0=");
        h.u(T0.load(Ordering::Relaxed));
        h.s(b" ovh=");
        h.u(OVERHEAD.load(Ordering::Relaxed));
        h.s(b"/");
        h.u(HOOKS.load(Ordering::Relaxed));
        h.s(b" ovhmax=");
        h.u(OVERHEAD_MAX.load(Ordering::Relaxed));
        h.s(b" flovh=");
        h.u(FLUSH_TICKS.load(Ordering::Relaxed));
        h.s(b"/");
        h.u(FLUSHES.load(Ordering::Relaxed) as u64);

        let handle = match create(PATH) {
            Some(x) => x,
            None => {
                leave_flush(t_in);
                return;
            }
        };
        let base = RING.recs.get() as *const Rec;
        let hdr = h.fixed();
        let mut ok = write(handle, hdr.as_ptr() as u64, hdr.len() as i64);

        // Prologue first (records 0..pro, already in order), then the ring
        // oldest-first. Each record carries its own `seq`, so the decoder sees
        // the gap between the two spans without needing to be told where it is.
        if ok && pro > 0 {
            // SAFETY: `pro <= PROLOGUE <= CAPACITY`, so the span lies inside
            // the record array; `Rec` is `repr(C)` plain data whose every
            // field (padding included) is written by `Rec::ZERO`, so its bytes
            // are valid to write out.
            ok = write(handle, base as u64, (pro as usize * RECORD_SIZE) as i64);
        }
        if ok && ring_held > 0 {
            let ring_len = CAPACITY - PROLOGUE;
            // Oldest ring record: once wrapped it sits at
            // `(produced - PROLOGUE) % ring_len`, otherwise at the region start.
            let start = if produced > CAPACITY as u64 {
                ((produced - PROLOGUE as u64) % ring_len as u64) as usize
            } else {
                0
            };
            let mut done = 0u64;
            while done < ring_held {
                let idx = (start + done as usize) % ring_len;
                // Longest contiguous run to the end of the ring region.
                let run = (ring_len - idx).min((ring_held - done) as usize);
                // SAFETY: `PROLOGUE + idx + run <= CAPACITY`, so the span lies
                // inside the record array (see the prologue write above for
                // why `Rec`'s bytes are valid to write out).
                let ptr = unsafe { base.add(PROLOGUE + idx) } as u64;
                if !write(handle, ptr, (run * RECORD_SIZE) as i64) {
                    ok = false;
                    break;
                }
                done += run as u64;
            }
        }
        close(handle);

        // A zero-record flush (the tracer proving at boot that it is armed and
        // that `C:` is writable) must not start the inter-flush timer, or the
        // first real trace would be needlessly delayed behind it.
        if produced > 0 {
            LAST_FLUSH.store(ticks(), Ordering::Relaxed);
        }
        FLUSHED_AT.store(produced, Ordering::Relaxed);
        let nflush = FLUSHES.fetch_add(1, Ordering::Relaxed) + 1;
        if ok {
            write_status(nflush, produced, held, dropped, pro);
        }
        leave_flush(t_in);
    }

    /// Write `C:\AESTRACE.TXT`: the plain-language confirmation that the
    /// binary file above was written completely. Deliberately the **last**
    /// thing a flush does, so its presence means the `.BIN` is finished — that
    /// is what makes an empty card unambiguous instead of a guess between
    /// "nothing captured", "trigger never fired" and "the write failed".
    fn write_status(nflush: u32, produced: u64, held: u64, dropped: u64, pro: u64) {
        let mut a = Asc::<STATUS_LEN>::new(0);
        a.s(b"NewTOS AES trap trace -- written successfully.\r\n\r\n");
        a.s(b"flush number : ");
        a.u(nflush as u64);
        a.s(b"\r\nrecords held : ");
        a.u(held);
        a.s(b" (");
        a.u(pro);
        a.s(b" from the start of the run, ");
        a.u(held - pro);
        a.s(b" most recent)\r\nrecords made : ");
        a.u(produced);
        a.s(b"\r\ndropped      : ");
        a.u(dropped);
        if dropped > 0 {
            a.s(b"  <-- the middle of the run was overwritten");
        }
        a.s(b"\r\ntimer clock  : ");
        a.u(tick_hz());
        a.s(b" Hz\r\ncapture cost : ");
        a.u(OVERHEAD.load(Ordering::Relaxed));
        a.s(b" ticks over ");
        a.u(HOOKS.load(Ordering::Relaxed));
        a.s(b" hooks, worst ");
        a.u(OVERHEAD_MAX.load(Ordering::Relaxed));
        a.s(b"\r\nwrite cost   : ");
        a.u(FLUSH_TICKS.load(Ordering::Relaxed));
        a.s(b" ticks total\r\ndata file    : C:\\AESTRACE.BIN, ");
        a.u(HDR_LEN as u64 + held * RECORD_SIZE as u64);
        a.s(b" bytes\r\n\r\nDecode on the development machine with:\r\n");
        a.s(b"    python3 tools/aestrace.py AESTRACE.BIN --button\r\n");
        let handle = match create(STATUS_PATH) {
            Some(x) => x,
            None => return,
        };
        let body = a.written();
        write(handle, body.as_ptr() as u64, body.len() as i64);
        close(handle);
    }

    /// `Fcreate(path, 0)` through the kernel-internal FS entry point (not a
    /// nested `svc`). `None` if C: is not writable.
    fn create(path: &[u8]) -> Option<u64> {
        let args = crate::trap::TrapArgs {
            regs: [path.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0],
            func: 0x3C,
            svc_imm: 1,
            space: crate::uaccess::Space::Kernel,
        };
        let h = crate::bdos::fs::file::fcreate(&args);
        if h < 0 {
            None
        } else {
            Some(h as u64)
        }
    }

    /// `Fwrite(handle, len, buf)`; `true` when the whole span was written.
    fn write(handle: u64, buf: u64, len: i64) -> bool {
        let args = crate::trap::TrapArgs {
            regs: [handle, len as u64, buf, 0, 0, 0, 0, 0],
            func: 0x40,
            svc_imm: 1,
            space: crate::uaccess::Space::Kernel,
        };
        crate::bdos::fs::file::fwrite(&args) == len as i32
    }

    fn close(handle: u64) {
        let args = crate::trap::TrapArgs {
            regs: [handle, 0, 0, 0, 0, 0, 0, 0],
            func: 0x3E,
            svc_imm: 1,
            space: crate::uaccess::Space::Kernel,
        };
        let _ = crate::bdos::fs::file::fclose(&args);
    }
}

// ─── Public surface (identical in both configurations) ───────────────

/// Called by [`crate::aes::dispatch`] immediately before the opcode switch.
/// Returns the entry timestamp to hand back to [`call_end`].
#[inline(always)]
pub(super) fn call_begin() -> CallStart {
    imp::call_begin()
}

/// Called by [`crate::aes::dispatch`] after the opcode switch (and after the
/// `int_out` copy-back, when there was one). `yielded` is `true` when the
/// call aborted into a cooperative peer yield and produced no result.
#[inline(always)]
pub(super) fn call_end(pb: &Aespb, t0: CallStart, yielded: bool) {
    imp::call_end(pb, t0, yielded)
}

/// Record one predicate evaluation of a blocking `evnt_*` wait: the mouse
/// button byte it read and how far the pointer moved since the previous
/// sample. Call sites guard with [`ENABLED`] so the whole statement folds away
/// in a build without the tracer.
#[inline(always)]
pub(super) fn note_eval() {
    imp::note_eval()
}

/// Record one poll inside the **double-click window** (`count_clicks` →
/// `wait_until`). Kept apart from [`note_eval`] so the trace can show whether
/// the pointer moved while that window stood open, and by how much per step —
/// the measurement against EmuTOS `mchange` (`aes/geminput.c:360-370`), which
/// cancels a pending button wait on a move of more than ±2.
#[inline(always)]
pub(super) fn note_click_window() {
    imp::note_click_window()
}

/// Record a GEM message enqueued into `pid`'s FIFO — the `aqueue`/`doq`
/// enqueue point (`aes/gemqueue.c:85-125`). `ok` is the enqueue result.
#[inline(always)]
pub(crate) fn msg_post(pid: i16, src: &[i16], words: usize, ok: bool) {
    imp::msg_post(pid, src, words, ok)
}

/// Ask for the trace to be written at the next [`flush_if_requested`].
/// Safe to call from anywhere; the write itself is deferred to the kernel
/// exception stack.
#[inline(always)]
pub fn request_flush() {
    imp::request_flush()
}

/// Perform a pending flush. Called by the kernel's GEM-trap handler after the
/// AES dispatch has returned, so the FAT write path runs on the kernel
/// exception stack rather than the 16 KiB AES fiber stack. A no-op when no
/// flush is pending (one atomic swap).
#[inline(always)]
pub fn flush_if_requested() {
    imp::flush_if_requested()
}

/// Write the trace now — used by `Pterm` so the file survives the application
/// exiting, and available to any kernel-side caller that wants a snapshot.
#[inline(always)]
pub fn flush_on_exit() {
    imp::flush()
}
