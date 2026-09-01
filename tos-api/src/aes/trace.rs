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
/// | 57 | 1 | `mb_or` — OR of the button byte over all evaluations |
/// | 58 | 1 | `mb_and` — AND of the button byte over all evaluations |
/// | 59 | 1 | pad |
/// | 60 | 48 | `w[24]` |
/// | 108 | 4 | pad |
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
    _rsv: i16,
    kind: u8,
    flags: u8,
    slot: u8,
    nin: u8,
    nout: u8,
    mb_or: u8,
    mb_and: u8,
    _pad0: u8,
    w: [i16; W_LEN],
    _pad1: [u8; 4],
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
        _rsv: 0,
        kind: 0,
        flags: 0,
        slot: 0,
        nin: 0,
        nout: 0,
        mb_or: 0,
        mb_and: 0,
        _pad0: 0,
        w: [0; W_LEN],
        _pad1: [0; 4],
    };
}

/// On-disk record size. Pinned so the host decoder and the target agree.
pub const RECORD_SIZE: usize = 112;
const _: () = assert!(core::mem::size_of::<Rec>() == RECORD_SIZE);

/// Ring capacity in records (`CAPACITY * RECORD_SIZE` = 896 KiB of BSS,
/// present only with the `aes-trace` feature). Sized so a TeraDesk startup
/// plus several seconds of interaction fit; when it wraps, the newest
/// records survive and the header says how many were dropped.
pub const CAPACITY: usize = 8192;

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
    pub(super) fn note_eval(_mb: u8) {}
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
        RECORD_SIZE, SLOT_KERNEL, W_LEN,
    };

    /// Blocking time (in ticks at 54 MHz / 62.5 MHz — see the note below)
    /// above which a completed `evnt_*` wait counts as "the user was idle",
    /// which is the only moment a flush may be requested. Stored as a
    /// *microsecond* count and scaled by [`tick_hz`] at the comparison, so
    /// it means the same on both boards. 3 s.
    const IDLE_FLUSH_US: u64 = 3_000_000;
    /// Minimum wall-clock gap between two flushes. 20 s.
    const MIN_FLUSH_GAP_US: u64 = 20_000_000;

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

    /// Per-call evaluation accumulators, reset by [`call_begin`] and folded
    /// into the record by [`call_end`]. They answer "what button byte did the
    /// wait actually read, on every one of its evaluations" without emitting a
    /// record per spin.
    ///
    /// **This is the only tracer state on a hot loop** — a blocking
    /// `evnt_multi` re-evaluates its predicate millions of times per second in
    /// the single-process case — so it is deliberately *not* atomic: plain
    /// loads/stores under the same single-CPU, IRQ-off GEM-trap discipline as
    /// [`Ring`] and the AES `pd`/`window` statics. Three atomic read-modify-
    /// writes per pass would be the one place this instrument could plausibly
    /// move the thing it measures; three plain stores are not.
    struct Eval {
        passes: UnsafeCell<u32>,
        mb_or: UnsafeCell<u8>,
        mb_and: UnsafeCell<u8>,
    }
    // SAFETY: see the type comment — single-CPU GEM-trap path only.
    unsafe impl Sync for Eval {}
    static EVAL: Eval = Eval {
        passes: UnsafeCell::new(0),
        mb_or: UnsafeCell::new(0),
        mb_and: UnsafeCell::new(0xFF),
    };

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

    /// Append `r` to the ring, stamping its sequence number. Overwrites the
    /// oldest record once the ring is full; nothing is ever lost silently,
    /// because the header reports produced-vs-held.
    fn push(mut r: Rec) {
        let seq = RING.next.fetch_add(1, Ordering::Relaxed);
        if seq == 0 {
            T0.store(r.t_ticks, Ordering::Relaxed);
        }
        r.seq = seq as u32;
        let idx = (seq % CAPACITY as u64) as usize;
        // SAFETY: single-CPU trap path serialised by `IN_HOOK`; `idx` is
        // reduced modulo the array length.
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
        // SAFETY: single-CPU GEM-trap path (see `Eval`).
        unsafe {
            *EVAL.passes.get() = 0;
            *EVAL.mb_or.get() = 0;
            *EVAL.mb_and.get() = 0xFF;
        }
        CallStart(t)
    }

    /// One predicate evaluation inside a blocking `evnt_*` wait, with the
    /// button byte the wait just read. Folded into the call record as
    /// `passes` / `mb_or` / `mb_and`. Three plain loads/stores — see [`Eval`]
    /// for why this is not atomic.
    #[inline(always)]
    pub(super) fn note_eval(mb: u8) {
        // SAFETY: single-CPU GEM-trap path (see `Eval`); saturating so a very
        // long wait cannot wrap the counter into a misleading small number.
        unsafe {
            let p = EVAL.passes.get();
            *p = (*p).saturating_add(1);
            *EVAL.mb_or.get() |= mb;
            *EVAL.mb_and.get() &= mb;
        }
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
            r.passes = *EVAL.passes.get();
            r.mb_or = *EVAL.mb_or.get();
            r.mb_and = *EVAL.mb_and.get();
        }

        // `int_in[0 .. min(in_len, 16)]` — exactly what the caller declared,
        // capped at the AES maximum (`evnt_multi`'s 16 WORDs).
        let nin = if in_len <= 0 { 0 } else { (in_len as usize).min(MAX_IN) };
        if pb.intin.is_null() {
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
        if pb.addrin != 0 && ain_len > 0 {
            // SAFETY: the caller's `addr_in` array, read at the caller's ABI
            // width exactly as `Aespb::addrin` does for the handlers.
            r.addr_in0 = unsafe { pb.abi.read_ptr(pb.addrin, 0) };
        }
        // `addr_out[0]` is read only for the one opcode our AES actually
        // writes it for — `rsrc_gaddr` (112), the sole `set_addrout` caller
        // (`aes/resource.rs:1286`). Reading it unconditionally would
        // dereference a pointer the dispatcher itself never touches for the
        // other ~50 opcodes, i.e. the instrument would add a fault surface to
        // the very run it is measuring. Everything else records 0.
        if pb.addrout != 0 && opcode == 112 {
            // SAFETY: as above, for the caller's `addr_out` array, which
            // `rsrc_gaddr` writes through `set_addrout` on this same call.
            r.addr_out0 = unsafe { pb.abi.read_ptr(pb.addrout, 0) };
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
                let p = r.addr_in0 as *const i16;
                for i in 0..8 {
                    m.w[i] = wrd(p, i);
                }
                m.nin = 8;
                m.opcode = m.w[0]; // the message type, for a quick scan
                push(m);
            }
        }

        maybe_request_flush(opcode, r.dur_ticks, t_in);
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

    /// Request a flush if the just-completed call was a blocking `evnt_*`
    /// wait that sat idle for at least [`IDLE_FLUSH_US`] — the user's hands
    /// were off the mouse — and the last flush is at least
    /// [`MIN_FLUSH_GAP_US`] ago. Never fires inside a click window or a
    /// hover burst, because those calls return in milliseconds.
    fn maybe_request_flush(opcode: i16, dur_ticks: u64, now: u64) {
        if !(20..=25).contains(&opcode) {
            return; // only the six blocking evnt_* waits
        }
        if dur_ticks < us_to_ticks(IDLE_FLUSH_US) {
            return;
        }
        let last = LAST_FLUSH.load(Ordering::Relaxed);
        if last != 0 && now.wrapping_sub(last) < us_to_ticks(MIN_FLUSH_GAP_US) {
            return;
        }
        if RING.next.load(Ordering::Relaxed) == 0 {
            return; // nothing to write
        }
        FLUSH_REQ.store(true, Ordering::Release);
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

    /// Header length in bytes — one ASCII line, space-padded, `\n`-terminated.
    /// 256 so every field still fits at its widest (a `produced`/`t0` after a
    /// long session is 12-13 digits); the builder truncates rather than
    /// overflowing, and a truncated header would silently lose `hz`.
    const HDR_LEN: usize = 256;

    /// A tiny fixed-capacity ASCII builder for the header. No `core::fmt`,
    /// so this is as dumb (and as auditable) as the rest of the path.
    struct Hdr {
        buf: [u8; HDR_LEN],
        len: usize,
    }
    impl Hdr {
        fn new() -> Self {
            Hdr { buf: [b' '; HDR_LEN], len: 0 }
        }
        fn s(&mut self, s: &[u8]) {
            for &b in s {
                if self.len < HDR_LEN - 1 {
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
                if self.len < HDR_LEN - 1 {
                    self.buf[self.len] = d[n];
                    self.len += 1;
                }
            }
        }
        fn finish(&mut self) -> &[u8] {
            self.buf[HDR_LEN - 1] = b'\n';
            &self.buf
        }
    }

    /// Rewrite `C:\AESTRACE.BIN` with the header and the ring's records in
    /// chronological order. Runs on the kernel exception stack (never the
    /// 16 KiB AES fiber stack). Every failure is swallowed — the instrument
    /// must never hang or fault the machine it is measuring.
    pub(super) fn flush() {
        if !enter() {
            return;
        }
        let t_in = ticks();

        let produced = RING.next.load(Ordering::Relaxed);
        let held = produced.min(CAPACITY as u64);
        let dropped = produced.saturating_sub(held);

        let mut h = Hdr::new();
        h.s(b"NEWTOS-AESTRACE v2 le rec=");
        h.u(RECORD_SIZE as u64);
        h.s(b" cap=");
        h.u(CAPACITY as u64);
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

        let handle = match create() {
            Some(x) => x,
            None => {
                leave_flush(t_in);
                return;
            }
        };
        let hdr = h.finish();
        if write(handle, hdr.as_ptr() as u64, hdr.len() as i64) {
            // Oldest first: with a wrapped ring the oldest record sits at
            // `produced % CAPACITY`, so the file is always chronological.
            let start = (produced.wrapping_sub(held) % CAPACITY as u64) as usize;
            let base = RING.recs.get() as *const Rec;
            let mut done = 0u64;
            while done < held {
                let idx = (start + done as usize) % CAPACITY;
                // Longest contiguous run from `idx` to the end of the array.
                let run = (CAPACITY - idx).min((held - done) as usize);
                // SAFETY: `idx + run <= CAPACITY`, so the span is inside the
                // record array; `Rec` is `repr(C)` plain data with no padding
                // holes left uninitialised (every field is written by
                // `Rec::ZERO`), so its bytes are valid to write out.
                let ptr = unsafe { base.add(idx) } as u64;
                if !write(handle, ptr, (run * RECORD_SIZE) as i64) {
                    break;
                }
                done += run as u64;
            }
        }
        close(handle);

        LAST_FLUSH.store(ticks(), Ordering::Relaxed);
        FLUSHES.fetch_add(1, Ordering::Relaxed);
        leave_flush(t_in);
    }

    /// `Fcreate(PATH, 0)` through the kernel-internal FS entry point (not a
    /// nested `svc`). `None` if C: is not writable.
    fn create() -> Option<u64> {
        let args = crate::trap::TrapArgs {
            regs: [PATH.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0],
            func: 0x3C,
            svc_imm: 1,
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
        };
        crate::bdos::fs::file::fwrite(&args) == len as i32
    }

    fn close(handle: u64) {
        let args = crate::trap::TrapArgs {
            regs: [handle, 0, 0, 0, 0, 0, 0, 0],
            func: 0x3E,
            svc_imm: 1,
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

/// Record one predicate evaluation of a blocking `evnt_*` wait together with
/// the mouse-button byte it read. Call sites must guard with [`ENABLED`] so
/// the argument expression folds away in a build without the tracer.
#[inline(always)]
pub(super) fn note_eval(mb: u8) {
    imp::note_eval(mb)
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
