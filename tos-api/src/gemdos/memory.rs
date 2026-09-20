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
use tos_types::error::{EGSBF, EIMBA, E_OK};
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

/// The largest `amount` an EL0 `Mxalloc`/`Malloc` can actually satisfy, given
/// the selected pool's largest free block and the process's remaining heap
/// window — i.e. the answer the `-1` probe owes its caller.
///
/// `Mxalloc(-1)` exists so a program can ask for the biggest block and then
/// take it; EmuTOS' `ffit(-1,…)` returns the length of a real free MD, and
/// `ffit(len,…)` on the next line succeeds (`bdos/umem.c:243-278` and
/// `:305-327`, with the grain round-up in `bdos/iumem.c:35`). The EL0 path
/// must keep that property,
/// and it costs two page-granular corrections:
///
/// * [`el0_alloc`] over-allocates **one extra page** so the page-aligned
///   window base still lies inside the 16-byte-aligned pool block, so the
///   biggest whole-page request the pool can still serve is
///   `floor_page(pool_free - PAGE)`, not `pool_free`.
/// * the window is mapped page-by-page, so the window term is already
///   floor-paged by [`usermem::window_remaining`].
///
/// A pool with less than two pages free therefore reports `0` — at EL0
/// nothing smaller than a page can be handed out at all, and `0` is the
/// answer `Mxalloc` gives for "no block available" anyway.
#[inline]
fn el0_largest_allocatable(pool_free: u64, window_remaining: u64) -> u64 {
    let pool_bound = pool_free.saturating_sub(PAGE) & !(PAGE - 1);
    pool_bound.min(window_remaining)
}

/// Allocate `amount` bytes from the ST/TT pool selected by `mode`, map the
/// block into `slot`'s EL0 heap window, and return the **user VA** — the ADR-021
/// EL0 path shared by `Mxalloc` and `Malloc`.
///
/// * `amount == -1` → the largest-free query, bounded by what this path can
///   actually hand out: [`el0_largest_allocatable`] (ADR-021 §4.5.6).
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
        let bound = el0_largest_allocatable(
            pool_free as u64,
            usermem::window_remaining(slot),
        );
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

/// Free the block an EL0 process names by the user VA `user_va` — the
/// common tail of [`mfree`] and a `Mshrink` to length 0.
///
/// The user VA is translated to the pool key through the process's block
/// table (`EIMBA` if no tracked block starts there — EmuTOS's "not on
/// `mp_mal`", `bdos/umem.c:148-153`) and the pool block is freed. Once it
/// is gone, the AES is told ([`crate::aes::resource::on_mfree`]): the key
/// may be a resource blob or icon buffer `rsrc_load` registered, which is
/// an ordinary process-owned block in EmuTOS too (`rs_readit` →
/// `dos_alloc_anyram`, `aes/gemrslib.c:775`) and freed by whoever passes
/// its address — TeraDesk's `xrsrc_free` = `Mfree(ap_rscmem)` (A76,
/// `docs/api/gemdos/memory/49-mfree.md` § "AES resource block"). The AES
/// entry must not free that key a second time on `rsrc_free`.
///
/// # Safety
///
/// BDOS lock held; `slot` is the running EL0 process.
unsafe fn el0_free(slot: usize, user_va: u64) -> Long {
    let Some(m) = usermem::free(slot, user_va) else {
        return EIMBA;
    };
    // SAFETY: BDOS lock held; `m` came from `Mxalloc` and is on the
    // allocated list.
    let rc = unsafe { traps::mfree(m) };
    if rc == E_OK {
        crate::aes::resource::on_mfree(slot, m);
    }
    rc
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
/// `mp_mal`") and free that ([`el0_free`]). A kernel caller frees the raw
/// address directly.
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
    // SAFETY: BDOS lock held; `slot` is the running EL0 process.
    unsafe { el0_free(slot, addr as u64) }
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
        // SAFETY: BDOS lock held; `slot` is the running EL0 process.
        return unsafe { el0_free(slot, blk as u64) };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Pool bytes [`el0_alloc`] asks `traps::mxalloc` for when the app
    /// requests `amount` — the property the `-1` probe has to respect.
    fn pool_demand(amount: u64) -> u64 {
        page_up(amount) + PAGE
    }

    /// Window bytes [`usermem::map`] consumes for the same request.
    fn window_demand(amount: u64) -> u64 {
        page_up(amount)
    }

    /// `Mxalloc(Mxalloc(-1, mode), mode)` must succeed — the whole point of
    /// the probe. Checked against the two figures a Pi 4 actually produces:
    /// the ST-RAM pool as seeded at boot (`[mm] BDOS ST-RAM … 6209536`) and
    /// the heap window a freshly-started TeraDesk has left after its
    /// `desktop.rsc` (39043 B → 10 pages) is mapped.
    #[test]
    fn probe_result_is_allocatable_when_the_pool_is_the_binding_term() {
        let pool_free = 6_209_536u64; // real ST-RAM pool, page-aligned
        let window = 16_736_256u64; // 16 MiB window minus desktop.rsc
        let bound = el0_largest_allocatable(pool_free, window);

        assert!(bound > 0);
        assert!(pool_demand(bound) <= pool_free, "pool cannot serve the probe");
        assert!(window_demand(bound) <= window, "window cannot map the probe");
        // Exactly one page is given up to the alignment slack.
        assert_eq!(bound, pool_free - PAGE);
    }

    #[test]
    fn probe_result_is_allocatable_when_the_window_is_the_binding_term() {
        let pool_free = 1_054_867_456u64; // real TT-RAM pool at boot
        let window = 16_736_256u64;
        let bound = el0_largest_allocatable(pool_free, window);

        assert_eq!(bound, window);
        assert!(pool_demand(bound) <= pool_free);
        assert!(window_demand(bound) <= window);
    }

    /// The pre-fix formula was `min(pool_free, window_remaining)`, which on
    /// the pool-bound path reported a size the pool provably cannot serve.
    #[test]
    fn the_naive_minimum_would_report_an_unallocatable_size() {
        let pool_free = 6_209_536u64;
        let window = 16_736_256u64;
        let naive = pool_free.min(window);

        assert!(pool_demand(naive) > pool_free);
        assert!(el0_largest_allocatable(pool_free, window) < naive);
    }

    /// A pool whose largest free MD is not a whole number of pages still has
    /// to yield a page-aligned, allocatable answer.
    #[test]
    fn unaligned_pool_free_is_rounded_down_to_a_mappable_size() {
        let window = 16 * 1024 * 1024;
        for extra in [1u64, 16, PAGE - 1] {
            let pool_free = 6_209_536 + extra;
            let bound = el0_largest_allocatable(pool_free, window);
            assert_eq!(bound % PAGE, 0);
            assert!(pool_demand(bound) <= pool_free);
        }
    }

    /// Below two pages there is nothing an EL0 caller could map, so the probe
    /// answers `0` — the same value `Mxalloc` returns for "no block".
    #[test]
    fn a_pool_smaller_than_two_pages_reports_nothing() {
        let window = 16 * 1024 * 1024;
        assert_eq!(el0_largest_allocatable(0, window), 0);
        assert_eq!(el0_largest_allocatable(PAGE, window), 0);
        assert_eq!(el0_largest_allocatable(2 * PAGE - 1, window), 0);
        assert_eq!(el0_largest_allocatable(2 * PAGE, window), PAGE);
    }

    /// An exhausted window reports `0` however much RAM the pool still holds.
    #[test]
    fn an_exhausted_window_reports_nothing() {
        assert_eq!(el0_largest_allocatable(1_054_867_456, 0), 0);
    }
}
