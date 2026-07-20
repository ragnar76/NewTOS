//! GEMDOS memory group (`Maddalt`, `Mxalloc`, `Malloc`, `Mfree`,
//! `Mshrink`).
//!
//! The trap entries in this module decode the AAPCS64 argument
//! registers and forward into [`crate::bdos::mem::traps`]. Argument
//! ordering follows the Atari ABI exactly so source-level ports see
//! the same parameters in the same positions.
//!
//! ## EL0 realization (ADR-021)
//!
//! The pool allocator ([`crate::bdos::mem::traps`]) returns **physical
//! addresses** reachable only from the kernel high half. An **EL0** app that
//! dereferenced one would fault, so for an SVC-originated (EL0) caller these
//! entries wrap the unchanged pool: allocate from the real ST/TT pool, **map
//! the block into a per-process EL0 heap window**
//! ([`crate::bdos::process::usermem`]), and return the **user VA**;
//! `Mfree`/`Mshrink` translate that user VA back to the pool key first. The
//! discrimination mirrors [`crate::aes::resource`] exactly — a non-null
//! [`crate::bdos::process::current`] means an EL0 app is running; a kernel /
//! desktop caller (which calls the pool directly, never through these trap
//! readers) resolves to `null` and keeps **raw physical addresses**.

use crate::bdos::mem::traps;
use crate::bdos::process::{self, usermem};
use crate::trap::TrapArgs;
use tos_types::error::{EGSBF, EIMBA};
use tos_types::{Long, MX_PREFTTRAM, MX_STRAM, PF_TTRAMMEM};

/// One extra page over-allocated from the pool so the page-aligned window
/// base (`round_up(m_start)`) plus the mapped span still lies inside the
/// 16-byte-aligned pool block. Mirrors [`crate::bdos::process::arena`].
const PAGE: u64 = usermem::PAGE_SIZE;

/// Round `v` up to the next whole [`PAGE`].
#[inline]
fn page_up(v: u64) -> u64 {
    (v + PAGE - 1) & !(PAGE - 1)
}

/// Allocate `amount` bytes from the ST/TT pool selected by `mode`, map the
/// block into `slot`'s EL0 heap window, and return the **user VA** — the ADR-021
/// EL0 path shared by `Mxalloc` and `Malloc`.
///
/// * `amount == -1` → the largest-free query, bounded by the mappable window:
///   `min(pool-largest-free-for-mode, window-remaining)` (ADR-021 §4.5.6).
/// * `amount <= 0` → `0` (NULL), as EmuTOS.
/// * otherwise → over-allocate one page for alignment, allocate from the
///   requested pool (ST/TT preference intact — the block still comes from
///   that pool), map, and hand back the window VA; `0` on `ENSMEM` / an
///   exhausted window.
///
/// # Safety
///
/// Caller holds the BDOS giant lock (single-CPU SVC context); `slot` is the
/// running EL0 process's slot.
unsafe fn el0_alloc(slot: usize, amount: Long, mode: i16) -> Long {
    // Largest-free probe, clamped to what the app can actually map.
    if amount == -1 {
        // SAFETY: BDOS lock held.
        let pool_free = unsafe { traps::mxalloc(-1, mode) };
        if pool_free <= 0 {
            return pool_free; // NULL / unknown mode → pass through
        }
        let bound = (pool_free as u64).min(usermem::window_remaining(slot));
        return bound.min(Long::MAX as u64) as Long;
    }
    if amount <= 0 {
        return 0;
    }

    // Over-allocate one page so the page-aligned window base fits inside the
    // 16-byte-aligned pool block, then allocate from the requested pool.
    let map_len = page_up(amount as u64);
    let want = (map_len + PAGE).min(Long::MAX as u64) as Long;
    // SAFETY: BDOS lock held.
    let raw = unsafe { traps::mxalloc(want, mode) };
    if raw == 0 {
        return 0;
    }
    // SAFETY: BDOS lock held; `raw` addresses a live pool block of
    // `map_len + one page` bytes owned by this process.
    match unsafe { usermem::map(slot, raw, map_len, usermem::Kind::Heap) } {
        Some(user_va) => user_va.min(Long::MAX as u64) as Long,
        None => {
            // Table full or window exhausted — return the block, fail ENSMEM.
            // SAFETY: BDOS lock held; `raw` is on the allocated list.
            unsafe { traps::mfree(raw) };
            0
        }
    }
}

/// `Maddalt(start, size)` — GEMDOS 0x14.
///
/// Spec:
/// [`docs/api/gemdos/memory/14-maddalt.md`](../../../../docs/api/gemdos/memory/14-maddalt.md).
///
/// Argument register layout:
/// - `x0` = `start`  (Atari `LONG`, sign-extended to `i64` on entry)
/// - `x1` = `size`   (Atari `LONG`)
///
/// `Maddalt` registers kernel-trusted physical RAM and is never an EL0-app
/// operation, so it always forwards to the raw pool primitive.
pub fn maddalt(args: &TrapArgs) -> Long {
    let start = args.arg_i32(0);
    let size = args.arg_i32(1);
    // SAFETY: NewTOS is single-CPU pre-Phase 06 and SVC handlers run
    // with IRQs masked, so the BDOS layer is exclusively held for
    // the duration of the dispatch. Phase 06 will replace this with
    // an explicit BDOS giant lock.
    unsafe { crate::bdos::mem::traps::maddalt(start, size) }
}

/// `Mxalloc(amount, mode)` — GEMDOS 0x44.
///
/// Spec:
/// [`docs/api/gemdos/memory/44-mxalloc.md`](../../../../docs/api/gemdos/memory/44-mxalloc.md).
///
/// Argument register layout:
/// - `x0` = `amount` (Atari `LONG`)
/// - `x1` = `mode`   (Atari `WORD`)
///
/// For an EL0 caller the returned pointer is a **user VA** into the process's
/// heap window; for a kernel caller it is the raw pool physical address
/// (ADR-021).
pub fn mxalloc(args: &TrapArgs) -> Long {
    let amount = args.arg_i32(0);
    let mode = args.arg_i16(1);
    let run = process::current();
    if run.is_null() {
        // EL1 / kernel caller: raw physical address, unchanged.
        // SAFETY: see `maddalt`.
        return unsafe { traps::mxalloc(amount, mode) };
    }
    // EL0 app: pool block mapped into the process's heap window.
    let Some(slot) = process::slot_for(run) else {
        return 0;
    };
    // SAFETY: BDOS lock held; `slot` is the running EL0 process.
    unsafe { el0_alloc(slot, amount, mode) }
}

/// `Malloc(amount)` — GEMDOS 0x48.
///
/// Spec:
/// [`docs/api/gemdos/memory/48-malloc.md`](../../../../docs/api/gemdos/memory/48-malloc.md).
///
/// Argument register layout:
/// - `x0` = `amount` (Atari `LONG`)
///
/// Policy wrapper around `Mxalloc`: EmuTOS `xmalloc` picks the mode from the
/// running process's `PF_TTRAMMEM` flag. For an EL0 caller we read that flag
/// from its basepage and take the ADR-021 window path; a kernel caller keeps
/// the raw-PA `traps::malloc` (kernel sentinel = `PF_TTRAMMEM`).
pub fn malloc(args: &TrapArgs) -> Long {
    let amount = args.arg_i32(0);
    let run = process::current();
    if run.is_null() {
        // EL1 / kernel caller: raw physical address, unchanged.
        // SAFETY: see `maddalt`.
        return unsafe { traps::malloc(amount) };
    }
    let Some(slot) = process::slot_for(run) else {
        return 0;
    };
    // EmuTOS `xmalloc`: `PF_TTRAMMEM ? MX_PREFTTRAM : MX_STRAM`, read from the
    // running process's own basepage.
    // SAFETY: `run` is the current process's basepage kernel VA (live).
    let flags = unsafe { (*run).p_flags };
    let mode = if flags & PF_TTRAMMEM != 0 {
        MX_PREFTTRAM
    } else {
        MX_STRAM
    };
    // SAFETY: BDOS lock held; `slot` is the running EL0 process.
    unsafe { el0_alloc(slot, amount, mode) }
}

/// `Mfree(addr)` — GEMDOS 0x49.
///
/// Spec:
/// [`docs/api/gemdos/memory/49-mfree.md`](../../../../docs/api/gemdos/memory/49-mfree.md).
///
/// Argument register layout:
/// - `x0` = `addr` (Atari `LONG`, the block start address)
///
/// For an EL0 caller `addr` is a **user VA**: translate it to the pool key
/// (`EIMBA` if it is not a tracked block start — matching EmuTOS's "not on
/// `mp_mal`") and free that. A kernel caller frees the raw address directly.
pub fn mfree(args: &TrapArgs) -> Long {
    let addr = args.arg_i32(0);
    let run = process::current();
    if run.is_null() {
        // SAFETY: see `maddalt`.
        return unsafe { traps::mfree(addr) };
    }
    let Some(slot) = process::slot_for(run) else {
        return EIMBA;
    };
    match usermem::free(slot, addr as u64) {
        // SAFETY: BDOS lock held; `m` came from `Mxalloc` and is on the
        // allocated list.
        Some(m) => unsafe { traps::mfree(m) },
        None => EIMBA,
    }
}

/// `Mshrink(n, blk, len)` — GEMDOS 0x4A.
///
/// Spec:
/// [`docs/api/gemdos/memory/4a-mshrink.md`](../../../../docs/api/gemdos/memory/4a-mshrink.md).
///
/// Argument register layout:
/// - `x0` = `n`   (Atari `WORD`, ignored — historic CP/M field)
/// - `x1` = `blk` (Atari `LONG`, block start address)
/// - `x2` = `len` (Atari `LONG`, new block length in bytes)
///
/// For an EL0 caller `blk` is a **user VA** (including the basepage's user
/// VA 0 — the TPA, seeded as block 0 so this no longer fails `EIMBA`). We
/// translate it to the pool key and add the alignment `prefix` to `len`, then
/// shrink the real pool block. `len == 0` frees the whole block (EmuTOS's
/// `Mshrink(len=0) == Mfree`). A kernel caller shrinks the raw block directly.
pub fn mshrink(args: &TrapArgs) -> Long {
    let n = args.arg_i16(0);
    let blk = args.arg_i32(1);
    let len = args.arg_i32(2);
    let run = process::current();
    if run.is_null() {
        // SAFETY: see `maddalt`.
        return unsafe { traps::mshrink(n, blk, len) };
    }
    let Some(slot) = process::slot_for(run) else {
        return EIMBA;
    };
    let Some(tb) = usermem::translate(slot, blk as u64) else {
        return EIMBA; // not a tracked block start
    };
    if len == 0 {
        // Mshrink-to-0 == Mfree the whole block (drop the window entry too so
        // no stale slot lingers, then free the pool block including its
        // alignment slack).
        return match usermem::free(slot, blk as u64) {
            // SAFETY: BDOS lock held; `m` is on the allocated list.
            Some(m) => unsafe { traps::mfree(m) },
            None => EIMBA,
        };
    }
    // The window maps exactly `tb.size` bytes of this block, so the app can
    // never hold more than that — a `Mshrink` to a longer length cannot be
    // honoured (EmuTOS's "cannot grow" → EGSBF), even though the over-
    // allocated pool block is one page larger.
    if len as u64 > tb.size {
        return EGSBF;
    }
    // Keeping the user's `[0, len)` means keeping `[m_start, m_start + prefix
    // + len)` of the pool block (the block starts `prefix` bytes below the
    // page-aligned base the user VA maps to). `traps::mshrink` rounds to the
    // pool grain and does the EGSBF/E_OK/shrink decision.
    let pool_len = tb
        .prefix
        .saturating_add(len as u64)
        .min(Long::MAX as u64) as Long;
    // SAFETY: BDOS lock held; `tb.m_start` is on the allocated list.
    unsafe { traps::mshrink(n, tb.m_start, pool_len) }
}
