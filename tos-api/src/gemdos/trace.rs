//! GEMDOS trap-path tracer — the file-system half of the dispatcher log
//! [`docs/TOS-REFERENCE-DUMPER.md`] §10 asks for.
//!
//! §10 says the case list for an area "comes from logging the trap dispatcher
//! under NewTOS, since the dispatcher is yours there". [`crate::aes::trace`] is
//! that log for the AES; this is the same instrument one layer down, recording
//! every call that reaches [`crate::gemdos::dispatch`] — opcode, the seven
//! argument registers, the `Long` result, the caller's process slot and ABI —
//! **plus the memory behind the pointer arguments**, which is the thing an AES
//! record never needed.
//!
//! ## Why the content matters (and an address does not)
//!
//! The AES passes WORD arrays at fixed positions, so its records could be dumb
//! and uniform. GEMDOS passes *pointers*: paths, buffers, DTAs, basepages. A
//! captured address differs on every run and says nothing about behaviour — a
//! trace of addresses is not an instrument. So each opcode declares, in
//! [`imp::capture`], **which argument to follow and how many bytes to copy**:
//! exactly the `capture` column of `docs/TOS-REFERENCE-DUMPER.md` §6
//! ("after the call, copy `length` bytes from the buffer pointed to by
//! argument `argindex`"), applied per opcode rather than per case-table line.
//!
//! ## Reading caller memory without adding a failure mode
//!
//! An instrument may not fault where the run it measures would not. Four rules,
//! enforced by construction in [`imp::capture`]:
//!
//! * **C1 — only memory the completed call itself already touched.** Every
//!   capture is gated on the implementation having read or written those exact
//!   bytes. `Fread`/`Fwrite` capture `min(ret, CAP_LEN)` bytes and nothing on a
//!   short or failed transfer (`bdos/fs/file.rs`: the copy loop moves exactly
//!   `done` bytes, and `done` is the return value). The `Fsfirst`/`Fsnext` DTA
//!   is captured only on `E_OK`, because that is the branch in which
//!   `put_state` + `makbuf` wrote all 44 bytes (`bdos/fs/find.rs:476-483`,
//!   `:508-511`). A path is captured because the implementation's very first
//!   act is `path_arg(args, i, …)` (`bdos/fs/mod.rs`).
//! * **C2 — every read is length-bounded**, and by the *same or a tighter*
//!   bound the implementation used: 128 bytes for a path (the `path_arg` cap,
//!   `bdos/fs/mod.rs` `PATH_ARG_MAX`), 44 for a DTA, 16 for a `Dfree` buffer, 4 for a
//!   `Fdatime` stamp, [`CAP_LEN`] for a data head.
//! * **C3 — a null pointer is skipped**, never dereferenced.
//! * **C4 — the table is closed.** There is no "if it looks like a pointer,
//!   read it" fallback; an opcode not listed captures nothing. So the tracer
//!   can never follow an argument its own implementation does not follow.
//!
//! What this does **not** cover, stated rather than glossed: a pointer that was
//! valid during the call and unmapped before the capture. Nothing runs between
//! the dispatch returning and the capture, and no traced opcode unmaps caller
//! memory (since ARCH03 stage 1 the pools stay mapped for every process; a
//! freed block remains readable memory, as on EmuTOS), so the window is empty
//! in this kernel. If that ever changes (stage 3 unmaps freed blocks), C1 has
//! to be re-argued.
//!
//! Reads use the access the call itself made (ARCH03 stage 3a): the caller's
//! [`crate::uaccess::Space`] — the unprivileged, fault-recovered loads for an
//! EL0 program, the kernel address as it is for an in-kernel caller — and,
//! for the DTA, the space [`crate::bdos::fs::find::cur_dta_in`] names.
//!
//! ## Re-entrancy: the tracer must not record its own writes
//!
//! The flush writes the trace to `C:` — and `Fcreate`/`Fwrite`/`Fclose` are
//! GEMDOS calls. A naive GEMDOS tracer would therefore record its own writes,
//! and could recurse until the ring held nothing but the tracer watching
//! itself. Three independent mechanisms, in order of strength:
//!
//! 1. **Structural — the hook is on the dispatcher, not on the functions.**
//!    [`crate::gemdos::dispatch`] has exactly one caller, the kernel's
//!    synchronous-exception handler, so it only ever sees a real `svc` from a
//!    user process. The flush calls
//!    [`fcreate`](crate::bdos::fs::file::fcreate) /
//!    [`fwrite`](crate::bdos::fs::file::fwrite) /
//!    [`fclose`](crate::bdos::fs::file::fclose) *directly* as kernel functions
//!    (exactly as [`crate::aes::trace`] does), which never pass through the
//!    dispatcher. The tracer's own file traffic is therefore not merely
//!    filtered out — it never reaches the hook.
//! 2. **A re-entrancy guard.** [`imp::IN_HOOK`] is claimed by every hook and by
//!    the flush. Any nested entry becomes a no-op and increments
//!    [`imp::SUPPRESSED`], which the file header reports as `sup=`. So
//!    mechanism 1 is not a claim in a comment: if it ever stops holding, `sup=`
//!    is non-zero and the trace says so.
//! 3. **The flush is deferred off the dispatch path entirely.** The hook only
//!    sets a flag; the kernel calls [`flush_if_requested`] after the dispatch
//!    has returned, on the kernel exception stack.
//!
//! The flush does change what it measures in one unavoidable way — it writes a
//! file to the very drive under observation, so free space, the FAT and a
//! directory entry move. That is recorded, not hidden: a [`kind::MARK`] record
//! is pushed immediately before each write, so the decoder can point at the
//! exact place in the trace where the tracer disturbed the file system.
//!
//! ## Off by default, free when off
//!
//! Everything is behind the **`gemdos-trace` cargo feature**. With it off,
//! [`ENABLED`] is `false`, every hook has an empty body and the ring buffer
//! static does not exist — no code, no BSS. Build it in with
//! `cargo xtask build --board rpi4 --release --features gemdos-trace`.
//!
//! The instrument measures its own cost: `ovh=` / `ovhmax=` in the header are
//! the accumulated and worst-case capture ticks, so "cheap enough" is a number
//! in the file rather than an assertion here.

// Without the `gemdos-trace` feature only the empty hook wrappers survive. The
// record layout, the tick source and the size constants stay compiled in that
// configuration on purpose — so the format cannot rot silently and the layout
// assertion still runs — but nothing uses them, so dead-code is allowed for
// that configuration only.
#![cfg_attr(not(feature = "gemdos-trace"), allow(dead_code))]

use crate::trap::TrapArgs;
use tos_types::Long;

/// Whether the tracer is compiled in. `false` folds every hook — and every
/// argument expression at a call site guarded by this — out of the build.
pub const ENABLED: bool = cfg!(feature = "gemdos-trace");

// ─── Record format (little-endian, native AArch64 order) ─────────────

/// Record kind — the `kind` byte at offset 6.
#[allow(dead_code)]
mod kind {
    /// A GEMDOS dispatch that ran to completion: opcode, args, result.
    pub const CALL: u8 = 1;
    /// A tracer marker. `args[0]` carries the marker sub-type
    /// ([`mark`](super::mark)).
    pub const MARK: u8 = 2;
    /// A frame-mutating GEMDOS call the dispatcher never sees
    /// (`Pterm`/`Pterm0`/`Ptermres`/`Pexec`/`Pwait`, special-cased in the
    /// kernel's exception handler). Recorded **before** the handler runs, so
    /// it carries arguments but no result — see [`flags::NO_RESULT`].
    pub const FRAME: u8 = 3;
}

/// [`kind::MARK`] sub-types, in `args[0]`.
#[allow(dead_code)]
mod mark {
    /// The tracer is about to write the trace file: the file system is being
    /// disturbed by the instrument at this point in the trace. `args[1]` holds
    /// the records the write will contain.
    pub const FLUSH: u64 = 1;
}

/// `flags` bits — the byte at offset 7.
#[allow(dead_code)]
mod flags {
    /// The record was produced by an in-kernel (EL1) caller — informational
    /// duplicate of `slot == SLOT_KERNEL`.
    pub const KERNEL_CALLER: u8 = 0x01;
    /// The caller runs the ILP32 application ABI (ADR-020), so its pointer
    /// arguments are the low 32 bits of the register.
    pub const ILP32: u8 = 0x02;
    /// The capture hit its length bound; the payload is a prefix, not the
    /// whole object.
    pub const CAP_CLAMPED: u8 = 0x04;
    /// No result is recorded (a [`kind::FRAME`] call, which returns by
    /// mutating the trap frame rather than through the dispatcher).
    pub const NO_RESULT: u8 = 0x08;
}

/// What the `cap` payload holds — the `cap_kind` byte at offset 90.
#[allow(dead_code)]
mod capkind {
    /// Nothing captured.
    pub const NONE: u8 = 0;
    /// One NUL-terminated path/string, from argument `cap_src`.
    pub const PATH: u8 = 1;
    /// Two NUL-terminated paths; the second starts at `cap2_off`
    /// (`Frename` source + destination, `Pexec` path + command tail).
    pub const PATH2: u8 = 2;
    /// A NUL-terminated path, then the 44-byte DTA at `cap2_off`
    /// (`Fsfirst`).
    pub const PATH_DTA: u8 = 3;
    /// The raw 44-byte DTA (`Fsnext`).
    pub const DTA: u8 = 4;
    /// The head of a transferred data buffer (`Fread`/`Fwrite`), `cap_len`
    /// bytes out of the `ret` actually transferred.
    pub const DATA: u8 = 5;
    /// A `Dfree` buffer: 4 little-endian LONGs — free clusters, total
    /// clusters, bytes/sector, sectors/cluster (`bdos/fs/drive.rs:68`).
    pub const DFREE: u8 = 6;
    /// A `Fdatime` stamp: DOS time WORD then date WORD, 4 bytes.
    pub const DOSTIME: u8 = 7;
    /// A console string (`Cconws`), NUL-terminated.
    pub const CONSOLE: u8 = 8;
}

/// `slot` value meaning "no user process — an in-kernel (EL1) caller".
const SLOT_KERNEL: u8 = 0xFF;

/// Bytes of caller memory a record can carry. 160 holds a full 8.3 path with
/// room to spare, both `Frename` paths, a path plus its 44-byte DTA, and a
/// data head long enough to identify a file's content at a glance.
pub const CAP_LEN: usize = 160;

/// Longest path/string followed, matching the 128-byte cap
/// [`crate::bdos::fs::path_arg`] itself uses (`PATH_ARG_MAX`) so the tracer
/// can never walk further than the implementation did.
const MAX_PATH: usize = 128;

/// Size of a GEMDOS DTA (`DTAINFO`, EmuTOS `include/bdosdefs.h:48-55`:
/// 21 reserved + attrib + time + date + length + 14-byte name).
const DTA_LEN: usize = 44;

/// One trace record. **Fixed 256-byte little-endian layout**, mirrored by
/// `tools/gdtrace.py`; do not reorder fields without updating the decoder.
///
/// | off | size | field |
/// |----:|-----:|-------|
/// | 0 | 4 | `seq` — record number, gapless across a wrap |
/// | 4 | 2 | `opcode` — GEMDOS function id (`x8`) |
/// | 6 | 1 | `kind` |
/// | 7 | 1 | `flags` |
/// | 8 | 8 | `t_ticks` — `CNTPCT_EL0` at dispatch entry |
/// | 16 | 8 | `dur_ticks` — dispatch entry→exit |
/// | 24 | 8 | `ret` — the `Long` result, sign-extended |
/// | 32 | 56 | `args[7]` — `x0..x6` exactly as the caller passed them |
/// | 88 | 1 | `slot` — GEMDOS process slot, `0xFF` = in-kernel caller |
/// | 89 | 1 | `abi` — 0 = LP64, 1 = ILP32 |
/// | 90 | 1 | `cap_kind` |
/// | 91 | 1 | `cap_src` — argument the payload was read from; meaningful only
///   when `cap_kind != NONE` |
/// | 92 | 2 | `cap_len` — payload bytes valid in `cap` |
/// | 94 | 2 | `cap2_off` — offset of the second payload part |
/// | 96 | 160 | `cap` |
#[repr(C)]
#[derive(Clone, Copy)]
struct Rec {
    seq: u32,
    opcode: u16,
    kind: u8,
    flags: u8,
    t_ticks: u64,
    dur_ticks: u64,
    ret: i64,
    args: [u64; 7],
    slot: u8,
    abi: u8,
    cap_kind: u8,
    cap_src: u8,
    cap_len: u16,
    cap2_off: u16,
    cap: [u8; CAP_LEN],
}

impl Rec {
    /// The empty record. **Every field must stay zero.** A single non-zero
    /// initialiser would make `RING`'s 1 MiB array a `.data` object rather
    /// than a `.bss` one, i.e. it would be written into `kernel8.img` and
    /// loaded from the card. (Measured: an earlier `cap_src: 0xFF` did exactly
    /// that.) `cap_src` is therefore 0 here and is only meaningful when
    /// `cap_kind != NONE`.
    const ZERO: Rec = Rec {
        seq: 0,
        opcode: 0,
        kind: 0,
        flags: 0,
        t_ticks: 0,
        dur_ticks: 0,
        ret: 0,
        args: [0; 7],
        slot: 0,
        abi: 0,
        cap_kind: capkind::NONE,
        cap_src: 0,
        cap_len: 0,
        cap2_off: 0,
        cap: [0; CAP_LEN],
    };
}

/// On-disk record size. Pinned so the host decoder and the target agree.
pub const RECORD_SIZE: usize = 256;
const _: () = assert!(core::mem::size_of::<Rec>() == RECORD_SIZE);

/// Total records held (`CAPACITY * RECORD_SIZE` = 1 MiB of BSS, present only
/// with the `gemdos-trace` feature).
///
/// The buffer is **split in two** (see [`PROLOGUE`]): a one-shot prologue that
/// keeps the *first* records of the run, and a ring that keeps the *newest*. A
/// trace that overruns therefore loses only the middle, and the header says
/// exactly how much.
pub const CAPACITY: usize = 4096;

/// Records reserved for the **prologue**: filled once, from the first GEMDOS
/// call of the run, and never overwritten. The remaining `CAPACITY - PROLOGUE`
/// records are the ring.
///
/// An application's opening conversation with the file system — which drive,
/// which config file, which resource — is the part a later burst of directory
/// scanning would otherwise push out, and it is the part that explains
/// everything after it.
pub const PROLOGUE: usize = 1024;
const _: () = assert!(PROLOGUE < CAPACITY);

/// Opaque handle returned by [`call_begin`] and consumed by [`call_end`] — the
/// `CNTPCT_EL0` value at dispatch entry. Carries no information when the
/// tracer is compiled out.
#[derive(Clone, Copy)]
pub struct CallStart(u64);

// ─── Time source ─────────────────────────────────────────────────────

/// Read the ARM generic timer's physical count (`CNTPCT_EL0`). Raw: no 64-bit
/// divide on the capture path. Conversion to microseconds happens in the host
/// decoder from the `CNTFRQ_EL0` value recorded in the file header.
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

/// Generic-timer frequency in Hz (`CNTFRQ_EL0`) — 54 MHz on a Pi 4, 62.5 MHz
/// under QEMU. Recorded in the file header so the decoder can turn ticks into
/// microseconds.
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

#[cfg(not(feature = "gemdos-trace"))]
mod imp {
    use super::{CallStart, Long, TrapArgs};

    #[inline(always)]
    pub(super) fn call_begin() -> CallStart {
        CallStart(0)
    }
    #[inline(always)]
    pub(super) fn call_end(_a: &TrapArgs, _t0: CallStart, _ret: Long) {}
    #[inline(always)]
    pub(super) fn note_frame_call(_func: u16, _regs: &[u64; 7], _path: &[u8], _tail: &[u8]) {}
    #[inline(always)]
    pub(super) fn request_flush() {}
    #[inline(always)]
    pub(super) fn flush_if_requested() {}
    #[inline(always)]
    pub(super) fn flush() {}
}

// ─── Enabled build ───────────────────────────────────────────────────

#[cfg(feature = "gemdos-trace")]
mod imp {
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

    use super::{
        capkind, flags, kind, mark, ticks, tick_hz, CallStart, Long, Rec, TrapArgs, CAPACITY,
        CAP_LEN, DTA_LEN, MAX_PATH, PROLOGUE, RECORD_SIZE, SLOT_KERNEL,
    };
    use crate::uaccess::Space;

    /// No GEMDOS record for this long before an automatic flush may fire.
    /// A GEMDOS burst is a unit of work — open a window, load a file, scan a
    /// directory — and three seconds without one means that unit finished, so
    /// the write lands between bursts rather than inside one.
    const IDLE_US: u64 = 3_000_000;

    /// Minimum wall-clock gap between two automatic flushes, so a busy
    /// application does not spend its time rewriting the trace.
    const MIN_FLUSH_GAP_US: u64 = 20_000_000;

    /// The ring buffer plus its cursor. Touched only from the single-CPU,
    /// IRQ-off trap path — the same discipline the AES tracer and the BDOS
    /// statics use — and additionally serialised against itself by
    /// [`IN_HOOK`].
    struct Ring {
        recs: UnsafeCell<[Rec; CAPACITY]>,
        /// Total records ever produced. `next` beyond `CAPACITY` means the
        /// ring has wrapped (records dropped).
        next: AtomicU64,
    }
    // SAFETY: see the type comment — single-CPU trap path, re-entrancy-guarded.
    unsafe impl Sync for Ring {}

    static RING: Ring = Ring {
        recs: UnsafeCell::new([Rec::ZERO; CAPACITY]),
        next: AtomicU64::new(0),
    };

    /// Re-entrancy guard (mechanism 2 of the three in the module docs). Held
    /// by every hook and by the flush, so a nested call — which mechanism 1
    /// says cannot happen — is dropped instead of corrupting the ring.
    static IN_HOOK: AtomicBool = AtomicBool::new(false);
    /// Hook entries the guard rejected. Reported as `sup=` in the file header:
    /// it must read 0, and if it does not, the structural argument that the
    /// flush cannot re-enter the dispatcher has broken and the trace says so
    /// instead of quietly holding the tracer's own writes.
    static SUPPRESSED: AtomicU64 = AtomicU64::new(0);

    /// `CNTPCT_EL0` at the first record — the trace epoch.
    static T0: AtomicU64 = AtomicU64::new(0);
    /// `CNTPCT_EL0` at the newest record, for the idle-flush rule.
    static LAST_REC: AtomicU64 = AtomicU64::new(0);
    /// Ticks spent inside the **capture** hooks, so the file states the
    /// instrument's own cost instead of asserting it is small. Excludes the
    /// flush, which is accounted separately in [`FLUSH_TICKS`].
    static OVERHEAD: AtomicU64 = AtomicU64::new(0);
    /// Capture-hook invocations [`OVERHEAD`] is spread over.
    static HOOKS: AtomicU64 = AtomicU64::new(0);
    /// Worst single capture hook, in ticks. A mean alone cannot separate
    /// "systematically expensive" from "one host-scheduling outlier under
    /// QEMU", and the no-perturbation claim rests on that difference.
    static OVERHEAD_MAX: AtomicU64 = AtomicU64::new(0);
    /// Ticks spent inside the flush (file write), counted apart.
    static FLUSH_TICKS: AtomicU64 = AtomicU64::new(0);

    /// A flush has been requested and runs at the next
    /// [`flush_if_requested`], on the kernel exception stack.
    static FLUSH_REQ: AtomicBool = AtomicBool::new(false);
    /// `CNTPCT_EL0` of the last completed flush (0 = never).
    static LAST_FLUSH: AtomicU64 = AtomicU64::new(0);
    /// Flushes completed so far.
    static FLUSHES: AtomicU32 = AtomicU32::new(0);
    /// `RING.next` at the last completed flush — "is there anything new to
    /// write" without re-reading the file.
    static FLUSHED_AT: AtomicU64 = AtomicU64::new(0);

    /// Convert a microsecond budget into generic-timer ticks.
    #[inline]
    fn us_to_ticks(us: u64) -> u64 {
        let hz = tick_hz();
        if hz == 0 {
            return u64::MAX; // no timer: never trip a time-based threshold
        }
        ((us as u128 * hz as u128) / 1_000_000) as u64
    }

    /// Claim the hook. `false` means a hook is already running (a nested
    /// call — dropped and counted rather than corrupting the ring).
    #[inline]
    fn enter() -> bool {
        if IN_HOOK.swap(true, Ordering::AcqRel) {
            SUPPRESSED.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
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
    /// The first [`PROLOGUE`] records land in the prologue slots and stay there
    /// for the whole run; everything after cycles through the ring, overwriting
    /// the oldest. Nothing is lost silently: each record carries its `seq`, so
    /// the decoder sees the discontinuity, and the header reports
    /// produced-versus-held.
    fn push(mut r: Rec) {
        let seq = RING.next.fetch_add(1, Ordering::Relaxed);
        if seq == 0 {
            T0.store(r.t_ticks, Ordering::Relaxed);
        }
        LAST_REC.store(r.t_ticks, Ordering::Relaxed);
        r.seq = seq as u32;
        let idx = if (seq as usize) < PROLOGUE {
            seq as usize
        } else {
            PROLOGUE + ((seq - PROLOGUE as u64) % (CAPACITY - PROLOGUE) as u64) as usize
        };
        // SAFETY: single-CPU trap path serialised by `IN_HOOK`; `idx` is either
        // below `PROLOGUE` or reduced modulo the ring length, so it is inside
        // the record array either way.
        unsafe {
            (*RING.recs.get())[idx] = r;
        }
    }

    /// The GEMDOS process slot of the current caller, or [`SLOT_KERNEL`] for an
    /// in-kernel (EL1) caller.
    #[inline]
    fn caller_slot() -> u8 {
        match crate::bdos::process::slot_for(crate::bdos::process::current()) {
            Some(i) if i < 0xFF => i as u8,
            _ => SLOT_KERNEL,
        }
    }

    // ─── Bounded caller-memory reads (rules C1..C4, module docs) ─────

    /// Copy the NUL-terminated string at `src` into `dst`, at most
    /// `max`(≤ `dst.len() - 1`) characters, and NUL-terminate it. Returns the
    /// bytes written (including the terminator) and whether the bound was hit.
    ///
    /// # Safety
    ///
    /// `src` must be a caller pointer the just-completed call itself walked
    /// with [`crate::bdos::fs::path_arg`], which stops at the same 128-byte
    /// bound (`bdos/fs/mod.rs` `PATH_ARG_MAX`). Read in the caller's `space`
    /// (ARCH03 3a) — the same access the call made; should it fault anyway,
    /// nothing is captured and the fault record of the call stays first.
    unsafe fn copy_cstr(space: Space, dst: &mut [u8], src: u64, max: usize) -> (usize, bool) {
        if src == 0 || dst.is_empty() {
            return (0, false);
        }
        let lim = max.min(dst.len() - 1);
        let n = space.read_cstr(src, &mut dst[..lim]).unwrap_or(0);
        dst[n] = 0;
        (n + 1, n == lim)
    }

    /// Copy `len` raw bytes from `src` into `dst`.
    ///
    /// # Safety
    ///
    /// `src..src+len` must be memory the just-completed call itself read or
    /// wrote (rule C1), in `space`; `len` is clamped to `dst.len()`. A fault
    /// (which C1 rules out) captures nothing.
    unsafe fn copy_raw(space: Space, dst: &mut [u8], src: u64, len: usize) -> usize {
        if src == 0 {
            return 0;
        }
        let n = len.min(dst.len());
        match space.copy_from(&mut dst[..n], src) {
            Ok(()) => n,
            Err(_) => 0,
        }
    }

    /// Fill `r`'s payload for one completed GEMDOS call.
    ///
    /// **This is the capture table**: the `docs/TOS-REFERENCE-DUMPER.md` §6
    /// `capture` column ("copy `length` bytes from the buffer pointed to by
    /// argument `argindex`") expressed per opcode. It is deliberately a closed
    /// `match` with no default that dereferences anything — rule C4.
    ///
    /// Function ids are EmuTOS's (`bdos/bdosmain.c`, the `osif` jump table),
    /// mirrored by our own dispatch table in `gemdos/mod.rs`.
    fn capture(r: &mut Rec, a: &TrapArgs, ret: Long) {
        // Every arm states, in one line, why the bytes it copies were already
        // touched by the call (rule C1).
        match r.opcode {
            // Cconws (0x09) — the implementation walks the string to its NUL
            // (`gemdos/console.rs:505-535`); we copy a bounded prefix of the
            // same walk. An application's console diagnostics belong in a
            // file-system trace: they explain the calls around them.
            0x09 => cap_path(r, a, 0, capkind::CONSOLE),

            // Fopen / Fcreate / Fdelete / Fattrib — `path_arg(arg0)` is the
            // implementation's first statement (`bdos/fs/file.rs:155,212,523,
            // 558`).
            0x3D | 0x3C | 0x41 | 0x43 => cap_path(r, a, 0, capkind::PATH),

            // Dcreate / Ddelete / Dsetpath — same, `bdos/fs/dirops.rs:30,114,
            // 155`.
            0x39..=0x3B => cap_path(r, a, 0, capkind::PATH),

            // Frename (0x56) — `path_arg(arg1)` (source) and `path_arg(arg2)`
            // (destination), both read before anything else
            // (`bdos/fs/file.rs:627-634`).
            0x56 => {
                // SAFETY: rule C1/C2 — both strings were walked by the
                // implementation, with the same 128-byte bound, before it did
                // anything else.
                let (n1, c1) = unsafe { copy_cstr(a.space, &mut r.cap, a.arg_u64(1), MAX_PATH) };
                // SAFETY: as above, for the destination path.
                let (n2, c2) = unsafe { copy_cstr(a.space, &mut r.cap[n1..], a.arg_u64(2), MAX_PATH) };
                r.cap_kind = capkind::PATH2;
                r.cap_src = 1;
                r.cap2_off = n1 as u16;
                r.cap_len = (n1 + n2) as u16;
                if c1 || c2 {
                    r.flags |= flags::CAP_CLAMPED;
                }
            }

            // Dgetpath (0x47) — the implementation always writes the buffer,
            // at minimum a lone NUL on the unmountable-drive branch
            // (`bdos/fs/dirops.rs:244-253`), so the string is always readable.
            0x47 => cap_path(r, a, 0, capkind::PATH),

            // Fsfirst (0x4E) — the search path is `path_arg(arg0)`
            // (`bdos/fs/find.rs:451`); the DTA is added only on `E_OK`, the
            // branch in which `put_state` + `makbuf` wrote all 44 bytes
            // (`bdos/fs/find.rs:476-483`).
            0x4E => {
                let (n1, c1) = unsafe { copy_cstr(a.space, &mut r.cap, a.arg_u64(0), MAX_PATH) };
                r.cap_src = 0;
                r.cap_len = n1 as u16;
                r.cap_kind = capkind::PATH;
                if c1 {
                    r.flags |= flags::CAP_CLAMPED;
                }
                if ret == 0 {
                    let (dta, dspace) = crate::bdos::fs::find::cur_dta_in(a.space);
                    // SAFETY: rule C1 — on `E_OK` this call wrote all 44 bytes
                    // at exactly this address.
                    let n2 = unsafe { copy_raw(dspace, &mut r.cap[n1..], dta, DTA_LEN) };
                    if n2 == DTA_LEN {
                        r.cap_kind = capkind::PATH_DTA;
                        r.cap2_off = n1 as u16;
                        r.cap_len = (n1 + n2) as u16;
                    }
                }
            }

            // Fsnext (0x4F) — same DTA, same `E_OK` gate
            // (`bdos/fs/find.rs:505-511`).
            0x4F if ret == 0 => {
                let (dta, dspace) = crate::bdos::fs::find::cur_dta_in(a.space);
                // SAFETY: as for Fsfirst.
                let n = unsafe { copy_raw(dspace, &mut r.cap, dta, DTA_LEN) };
                if n == DTA_LEN {
                    r.cap_kind = capkind::DTA;
                    r.cap_len = n as u16;
                }
            }

            // Fread (0x3F) / Fwrite (0x40) — the transfer loop moved exactly
            // `done` bytes to/from `arg2`, and `done` is the return value
            // (`bdos/fs/file.rs:326-341,418-427`). Nothing is captured on a
            // failed or empty transfer, so the tracer never touches a buffer
            // the call did not.
            0x3F | 0x40 if ret > 0 => {
                let want = (ret as usize).min(CAP_LEN);
                // SAFETY: rule C1 — those bytes were just copied by the call
                // itself.
                let n = unsafe { copy_raw(a.space, &mut r.cap, a.arg_u64(2), want) };
                r.cap_kind = capkind::DATA;
                r.cap_src = 2;
                r.cap_len = n as u16;
                if (ret as usize) > n {
                    r.flags |= flags::CAP_CLAMPED;
                }
            }

            // Dfree (0x36) — the four LONGs are written on the success path
            // only (`bdos/fs/drive.rs:68-75`).
            0x36 if ret == 0 => {
                // SAFETY: rule C1 — 16 bytes just written by the call.
                let n = unsafe { copy_raw(a.space, &mut r.cap, a.arg_u64(0), 16) };
                r.cap_kind = capkind::DFREE;
                r.cap_src = 0;
                r.cap_len = n as u16;
            }

            // Fdatime (0x57) — 4 bytes at `arg0`, written by the get branch and
            // read by the set branch, both only after the handle resolved
            // (`bdos/fs/file.rs:786-806`).
            0x57 if ret == 0 => {
                // SAFETY: rule C1 — the call just copied these 4 bytes.
                let n = unsafe { copy_raw(a.space, &mut r.cap, a.arg_u64(0), 4) };
                r.cap_kind = capkind::DOSTIME;
                r.cap_src = 0;
                r.cap_len = n as u16;
            }

            // Everything else: arguments and result only. Fsetdta (0x1A) is
            // deliberately here — it *stores* the pointer without ever
            // dereferencing it (`bdos/fs/find.rs:107-110`), so following it
            // would be the tracer inventing an access the call never made.
            _ => {}
        }
    }

    /// Shared arm for "one NUL-terminated string from argument `idx`".
    fn cap_path(r: &mut Rec, a: &TrapArgs, idx: usize, k: u8) {
        // SAFETY: rule C1/C2 — the implementation walked this same string with
        // the same 128-byte bound before doing anything else.
        let (n, clamped) = unsafe { copy_cstr(a.space, &mut r.cap, a.arg_u64(idx), MAX_PATH) };
        if n == 0 {
            return;
        }
        r.cap_kind = k;
        r.cap_src = idx as u8;
        r.cap_len = n as u16;
        if clamped {
            r.flags |= flags::CAP_CLAMPED;
        }
    }

    // ─── Hooks ───────────────────────────────────────────────────────

    pub(super) fn call_begin() -> CallStart {
        CallStart(ticks())
    }

    pub(super) fn call_end(a: &TrapArgs, t0: CallStart, ret: Long) {
        if !enter() {
            return;
        }
        let t_in = ticks();

        let mut r = Rec::ZERO;
        r.kind = kind::CALL;
        r.t_ticks = t0.0;
        r.dur_ticks = t_in.wrapping_sub(t0.0);
        r.opcode = a.func_id();
        r.ret = ret as i64;
        r.args = [
            a.regs[0], a.regs[1], a.regs[2], a.regs[3], a.regs[4], a.regs[5], a.regs[6],
        ];
        r.slot = caller_slot();
        if r.slot == SLOT_KERNEL {
            r.flags |= flags::KERNEL_CALLER;
        }
        if crate::bdos::process::current_abi() == crate::bdos::process::Abi::Ilp32 {
            r.abi = 1;
            r.flags |= flags::ILP32;
        }
        capture(&mut r, a, ret);
        push(r);
        leave(t_in);
    }

    /// Record a frame-mutating GEMDOS call the dispatcher never sees.
    /// `path`/`tail` are byte slices the **caller** already read out of the
    /// application's address space with its own bounded reader, so this hook
    /// performs no caller-memory access of its own.
    pub(super) fn note_frame_call(func: u16, regs: &[u64; 7], path: &[u8], tail: &[u8]) {
        if !enter() {
            return;
        }
        let t_in = ticks();
        let mut r = Rec::ZERO;
        r.kind = kind::FRAME;
        r.t_ticks = t_in;
        r.opcode = func;
        r.args = *regs;
        r.flags |= flags::NO_RESULT;
        r.slot = caller_slot();
        if r.slot == SLOT_KERNEL {
            r.flags |= flags::KERNEL_CALLER;
        }
        if crate::bdos::process::current_abi() == crate::bdos::process::Abi::Ilp32 {
            r.abi = 1;
            r.flags |= flags::ILP32;
        }
        if !path.is_empty() {
            let n1 = path.len().min(CAP_LEN - 1);
            r.cap[..n1].copy_from_slice(&path[..n1]);
            r.cap[n1] = 0;
            let mut end = n1 + 1;
            r.cap_kind = capkind::PATH;
            r.cap_src = 1; // Pexec's `x1` is the path
            r.cap2_off = end as u16;
            if !tail.is_empty() && end < CAP_LEN - 1 {
                let n2 = tail.len().min(CAP_LEN - 1 - end);
                r.cap[end..end + n2].copy_from_slice(&tail[..n2]);
                r.cap[end + n2] = 0;
                end += n2 + 1;
                r.cap_kind = capkind::PATH2;
            }
            r.cap_len = end as u16;
        }
        push(r);
        leave(t_in);
    }

    pub(super) fn request_flush() {
        FLUSH_REQ.store(true, Ordering::Release);
    }

    /// Run a pending flush, and decide whether the idle rule has come due.
    ///
    /// Called by the kernel after **every** trap, not only a GEMDOS one, which
    /// is what makes the idle rule work at all: a GEM application's event loop
    /// hammers the AES, so the tail of the trap handler is a reliable clock
    /// even while the file system is completely quiet. In the common case this
    /// costs two relaxed loads and returns.
    pub(super) fn flush_if_requested() {
        if FLUSH_REQ.swap(false, Ordering::AcqRel) {
            flush();
            return;
        }
        // Nothing new since the last write? Nothing to gain, and a needless
        // SD write to lose.
        let produced = RING.next.load(Ordering::Relaxed);
        if produced == 0 || produced <= FLUSHED_AT.load(Ordering::Relaxed) {
            return;
        }
        let now = ticks();
        // The burst must be over: no GEMDOS record for IDLE_US.
        if now.wrapping_sub(LAST_REC.load(Ordering::Relaxed)) < us_to_ticks(IDLE_US) {
            return;
        }
        // And the user must not be mid-gesture — the same reasoning the AES
        // tracer uses, because on a desktop a held button means a drag is in
        // flight and a multi-hundred-KiB SD write is the last thing it needs.
        if crate::vdi::mouse_buttons() != 0 {
            return;
        }
        let last = LAST_FLUSH.load(Ordering::Relaxed);
        if last != 0 && now.wrapping_sub(last) < us_to_ticks(MIN_FLUSH_GAP_US) {
            return;
        }
        flush();
    }

    // ─── The file ────────────────────────────────────────────────────

    /// Sink for the trace. Absolute so it does not depend on the running
    /// process's current drive/directory.
    const PATH: &[u8] = b"C:\\GDTRACE.BIN\0";

    /// Companion status file, written **after** `GDTRACE.BIN` is closed, so
    /// its presence means the binary file is complete. Readable at the machine
    /// without any tool: neither file → the trigger never fired (or `C:` is not
    /// writable); `.BIN` but no `.TXT` → the write was interrupted; both → the
    /// trace is whole, and the `.TXT` says how many records it holds.
    const STATUS_PATH: &[u8] = b"C:\\GDTRACE.TXT\0";

    /// Header length in bytes — one ASCII line, space-padded, `\n`-terminated.
    const HDR_LEN: usize = 256;

    /// Bytes reserved for the human-readable status file.
    const STATUS_LEN: usize = 768;

    /// No `core::fmt`, so the write path stays as dumb (and as auditable) as
    /// the rest of the instrument. Appends are silently dropped once full — it
    /// can never overflow.
    /// A tiny fixed-capacity ASCII builder. `s`/`u` are `#[inline(never)]`
    /// on purpose: inlined, the fixed-size buffer made the compiler unroll a
    /// copy per call site and `flush` grew to 38 KB of text. It runs once per
    /// file write; a call is free at that scale.
    struct Asc<const N: usize> {
        buf: [u8; N],
        len: usize,
    }
    impl<const N: usize> Asc<N> {
        fn new(fill: u8) -> Self {
            Asc { buf: [fill; N], len: 0 }
        }
        #[inline(never)]
        fn s(&mut self, s: &[u8]) {
            for &b in s {
                if self.len < N {
                    self.buf[self.len] = b;
                    self.len += 1;
                }
            }
        }
        #[inline(never)]
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
        /// The full fixed-size buffer, last byte forced to `\n` — the binary
        /// file's header is a fixed-width field, not a string.
        fn fixed(&mut self) -> &[u8] {
            self.buf[N - 1] = b'\n';
            &self.buf
        }
        /// Only the bytes written so far — for the free-form status file.
        fn written(&self) -> &[u8] {
            &self.buf[..self.len]
        }
    }

    /// Rewrite `C:\GDTRACE.BIN` with the header and the records in
    /// chronological order, then write `C:\GDTRACE.TXT`. Runs on the kernel
    /// exception stack. Every failure is swallowed — the instrument must never
    /// hang or fault the machine it is measuring.
    pub(super) fn flush() {
        if !enter() {
            return;
        }
        let t_in = ticks();

        // The write is about to allocate clusters, move the FAT and rewrite a
        // directory entry on the very drive under observation. Say so, in the
        // trace, at the exact position where it happens — the alternative is a
        // reader wondering later why the free-cluster count moved.
        let mut m = Rec::ZERO;
        m.kind = kind::MARK;
        m.t_ticks = t_in;
        m.slot = caller_slot();
        m.args[0] = mark::FLUSH;
        m.args[1] = RING.next.load(Ordering::Relaxed) + 1;
        push(m);

        let produced = RING.next.load(Ordering::Relaxed);
        let held = produced.min(CAPACITY as u64);
        let dropped = produced.saturating_sub(held);
        // Prologue records actually present, and the ring's share of `held`.
        let pro = produced.min(PROLOGUE as u64);
        let ring_held = held - pro;

        let mut h = Asc::<HDR_LEN>::new(b' ');
        h.s(b"NEWTOS-GDTRACE v1 le rec=");
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
        h.s(b" sup=");
        h.u(SUPPRESSED.load(Ordering::Relaxed));

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
        // the gap between the two spans without being told where it is.
        if ok && pro > 0 {
            // SAFETY: `pro <= PROLOGUE <= CAPACITY`, so the span lies inside
            // the record array; `Rec` is `repr(C)` plain data whose every field
            // (padding included) is written by `Rec::ZERO`, so its bytes are
            // valid to write out.
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
                // inside the record array (see the prologue write above for why
                // `Rec`'s bytes are valid to write out).
                let ptr = unsafe { base.add(PROLOGUE + idx) } as u64;
                if !write(handle, ptr, (run * RECORD_SIZE) as i64) {
                    ok = false;
                    break;
                }
                done += run as u64;
            }
        }
        close(handle);

        LAST_FLUSH.store(ticks(), Ordering::Relaxed);
        FLUSHED_AT.store(produced, Ordering::Relaxed);
        let nflush = FLUSHES.fetch_add(1, Ordering::Relaxed) + 1;
        if ok {
            write_status(nflush, produced, held, dropped, pro);
        }
        leave_flush(t_in);
    }

    /// Write `C:\GDTRACE.TXT`: the plain-language confirmation that the binary
    /// file above was written completely. Deliberately the **last** thing a
    /// flush does, so its presence means the `.BIN` is finished.
    fn write_status(nflush: u32, produced: u64, held: u64, dropped: u64, pro: u64) {
        let mut a = Asc::<STATUS_LEN>::new(0);
        a.s(b"NewTOS GEMDOS trap trace -- written successfully.\r\n\r\n");
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
        a.s(b"\r\nsuppressed   : ");
        a.u(SUPPRESSED.load(Ordering::Relaxed));
        a.s(b" (re-entrant hook entries; must be 0)");
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
        a.s(b" ticks total\r\ndata file    : C:\\GDTRACE.BIN, ");
        a.u(HDR_LEN as u64 + held * RECORD_SIZE as u64);
        a.s(b" bytes\r\n\r\nDecode on the development machine with:\r\n");
        a.s(b"    python3 tools/gdtrace.py GDTRACE.BIN\r\n");
        let handle = match create(STATUS_PATH) {
            Some(x) => x,
            None => return,
        };
        let body = a.written();
        write(handle, body.as_ptr() as u64, body.len() as i64);
        close(handle);
    }

    /// `Fcreate(path, 0)` through the kernel-internal FS entry point — **not** a
    /// nested `svc` and **not** [`crate::gemdos::dispatch`], which is what keeps
    /// the tracer's own writes out of its own hook (mechanism 1, module docs).
    /// `None` if `C:` is not writable.
    fn create(path: &[u8]) -> Option<u64> {
        let args = TrapArgs {
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
        let args = TrapArgs {
            regs: [handle, len as u64, buf, 0, 0, 0, 0, 0],
            func: 0x40,
            svc_imm: 1,
            space: crate::uaccess::Space::Kernel,
        };
        crate::bdos::fs::file::fwrite(&args) == len as i32
    }

    fn close(handle: u64) {
        let args = TrapArgs {
            regs: [handle, 0, 0, 0, 0, 0, 0, 0],
            func: 0x3E,
            svc_imm: 1,
            space: crate::uaccess::Space::Kernel,
        };
        let _ = crate::bdos::fs::file::fclose(&args);
    }
}

// ─── Public surface (identical in both configurations) ───────────────

/// Called by [`crate::gemdos::dispatch`] immediately before the table lookup.
/// Returns the entry timestamp to hand back to [`call_end`].
#[inline(always)]
pub(crate) fn call_begin() -> CallStart {
    imp::call_begin()
}

/// Called by [`crate::gemdos::dispatch`] with the completed call's arguments
/// and its `Long` result.
#[inline(always)]
pub(crate) fn call_end(args: &TrapArgs, t0: CallStart, ret: Long) {
    imp::call_end(args, t0, ret)
}

/// Record a frame-mutating GEMDOS call — `Pterm`/`Pterm0`/`Ptermres` (0x4C /
/// 0x00 / 0x31), `Pexec` (0x4B), `Pwait` (0x4D) — which the kernel handles
/// before [`crate::gemdos::dispatch`] because they return by rewriting the trap
/// frame. `regs` is `x0..x6`; `path`/`tail` are byte slices the **kernel** has
/// already read out of the application's address space with its own bounded,
/// EL0-permission-checked reader, so this hook performs no caller-memory access
/// of its own. Pass empty slices when there is nothing to attach.
#[inline(always)]
pub fn note_frame_call(func: u16, regs: &[u64; 7], path: &[u8], tail: &[u8]) {
    imp::note_frame_call(func, regs, path, tail)
}

/// Ask for the trace to be written at the next [`flush_if_requested`]. Safe to
/// call from anywhere; the write itself is deferred to the kernel exception
/// stack.
#[inline(always)]
pub fn request_flush() {
    imp::request_flush()
}

/// Perform a pending flush, or start one if the application has gone idle.
/// Called by the kernel's trap handler after the dispatch has returned, so the
/// FAT write path runs on the kernel exception stack. Cheap (two relaxed loads)
/// when there is nothing to do, and compiled out entirely without the
/// `gemdos-trace` feature.
#[inline(always)]
pub fn flush_if_requested() {
    imp::flush_if_requested()
}

/// Write the trace now — used by `Pterm` so the file survives the application
/// exiting, and by the kernel's panic / unhandled-fault paths, which are the
/// last chance to get a trace off the machine.
#[inline(always)]
pub fn flush_on_exit() {
    imp::flush()
}
