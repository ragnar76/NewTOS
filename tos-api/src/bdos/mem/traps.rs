//! GEMDOS memory-trap entry points.
//!
//! These functions sit between the GEMDOS dispatcher
//! ([`crate::gemdos::dispatch`]) and the BDOS pool primitives
//! ([`super::pool`]). Each one corresponds to one GEMDOS function
//! ID and one extracted spec under
//! [`docs/api/gemdos/memory/`](../../../../../docs/api/gemdos/memory/).
//!
//! D7 covers `Maddalt` (0x14). D8..D11 add the rest.

use core::ffi::c_void;
use core::ptr;

use tos_types::error::{EGSBF, EIMBA, ENSMEM, ERROR, E_OK};
use tos_types::{
    Long, MD, MPB, MX_MODEMASK, MX_PREFSTRAM, MX_PREFTTRAM, MX_STRAM, MX_TTRAM,
};

use super::init;
use super::mdpool::xmgetmd_global;
use super::pool::{self, PoolConfig};

// =====================================================================
// PHASE-04 D12 — owner attribution (`MD::m_own`)
// =====================================================================
//
// Every successful BDOS allocation must record an owner cookie in the
// resulting MD's `m_own` field. The cookie identifies the entity that
// owns the block, so the Phase-06 `Pterm` cleanup path
// (`free_all_owned`) can reap exactly the MDs the terminating
// process allocated and leave kernel-owned blocks alone.
//
// Spec: `docs/api/structures/md.md` § "Allocator semantics" and
// "NewTOS implementation notes" — "Until Phase 06 introduces real
// PDs, every alloc gets a sentinel `KERNEL_PD` pointer so the
// `Pterm`-cleanup mechanism is structurally correct from the
// start."
//
// We implement that contract here with a single static and a tiny
// indirection through [`current_owner`]. Phase 06 will reuse the same
// API surface; only the body of `current_owner` changes (it will
// consult the running PD).

/// Sentinel "owner" cookie for kernel-owned BDOS allocations.
///
/// The **address** of this static is the cookie — its value (`0u8`)
/// is irrelevant. Every Phase-04 allocation gets this address stamped
/// into [`MD::m_own`] via [`current_owner`]; Phase 06's `Pterm`
/// cleanup walks `mp_mal` looking for MDs whose `m_own ==
/// terminating_PD` and so naturally **skips** kernel-owned MDs (a
/// real PD pointer can never equal the address of this rodata byte).
///
/// Why a static `u8` rather than a magic numeric constant or an
/// `Option<NonNull<...>>`:
/// - The address is unique, stable for the kernel lifetime, and
///   chosen by the linker (no risk of colliding with a real PD
///   pointer that the heap might hand out).
/// - It lives in `.rodata`, so a stray write through `m_own` would
///   fault — useful for catching ownership bugs early.
/// - The pattern matches the spec note 1:1 ("sentinel `KERNEL_PD`
///   pointer"), which keeps the Phase 06 transition mechanical.
///
/// **Do not** read or write this byte; the only legitimate operation
/// is `&KERNEL_PD_SENTINEL as *const _ as *mut c_void`.
static KERNEL_PD_SENTINEL: u8 = 0;

/// Owner cookie to stamp into [`MD::m_own`] on every successful BDOS
/// allocation.
///
/// **Phase 06 wiring (D9):** consult [`crate::bdos::process::current`]
/// — the `RUN` `AtomicPtr<BasePage>` set by `Pexec`. If a user
/// process is active, its [`BasePage`] kernel VA is the cookie;
/// otherwise (boot, between processes) we fall back to the
/// [`KERNEL_PD_SENTINEL`] address, so allocations made by the boot
/// CPU outside any process are tagged kernel-owned and survive
/// every future `Pterm`.
///
/// The cookie is **never dereferenced**: it's a pointer-equality tag
/// the `Pterm`-cleanup walker (`free_all_owned`, Phase 06 D6)
/// matches against the terminating process's basepage. Distinct
/// processes get distinct cookies because each `Pexec` allocates a
/// fresh `BasePage` at a fresh phys address.
///
/// The return type is `*mut c_void` to match [`MD::m_own`] verbatim;
/// callers cast to/from `*mut BasePage` once Phase 06 lands.
///
/// # Safety
///
/// Pure function — safe to call any time. Cookie comparison is
/// pointer-equality only; the cookie value is never dereferenced by
/// any code path.
pub fn current_owner() -> *mut c_void {
    let run = crate::bdos::process::current();
    if run.is_null() {
        // No running process — kernel-owned allocation (boot path,
        // driver init, between-process kernel work). Cast `*const u8`
        // → `*mut c_void`: we never write through the resulting
        // pointer, the cast just drops the immutability so the value
        // type-checks against `MD::m_own`'s `*mut` signature.
        (&KERNEL_PD_SENTINEL as *const u8 as *mut u8).cast()
    } else {
        // A user process is the implicit owner — its `BasePage` VA
        // is the cookie.
        run.cast()
    }
}

/// Owner cookie of memory a terminated process left resident with
/// `Ptermres` — the system's TSR holder (ARCH03 stage 2c, plan §3.8 /
/// Frage 16: FreeMiNT keeps a TSR's regions attached to the process in its
/// `TSR_Q`, `sys/k_exit.c:307-336`, so they stay allocated and owned rather
/// than become nobody's). Like [`KERNEL_PD_SENTINEL`], only its address
/// matters; no PD can ever equal it.
static TSR_OWNER_SENTINEL: u8 = 0;

/// The owner cookie of resident (`Ptermres`) memory, see
/// [`TSR_OWNER_SENTINEL`].
#[inline]
pub fn tsr_owner() -> *mut c_void {
    (&TSR_OWNER_SENTINEL as *const u8 as *mut u8).cast()
}

/// `true` iff `m_own` is the TSR holder ([`tsr_owner`]).
#[inline]
pub fn is_resident(m_own: *mut c_void) -> bool {
    m_own == tsr_owner()
}

/// The kernel's owner cookie ([`KERNEL_PD_SENTINEL`]) — what a kernel
/// caller's allocations carry, and the holder of `F_KEEP` blocks whose last
/// owner terminated (FreeMiNT's `rootproc`, [`super::region`]).
#[inline]
pub fn kernel_owner() -> *mut c_void {
    (&KERNEL_PD_SENTINEL as *const u8 as *mut u8).cast()
}

/// `true` iff `m_own` is the kernel-owned sentinel cookie.
///
/// Used by Phase-06 `Pterm` cleanup to skip kernel-owned MDs cheaply
/// (one pointer compare per MD). The check is independent of the
/// running-process state — even if `Pexec` just set `RUN`, an MD
/// allocated earlier with `m_own == KERNEL_PD_SENTINEL` keeps that
/// tag forever, which is what we want.
#[inline]
pub fn is_kernel_owned(m_own: *mut c_void) -> bool {
    m_own == (&KERNEL_PD_SENTINEL as *const u8 as *mut u8).cast()
}

/// `Maddalt(start, size)` — register an alt-RAM region. GEMDOS 0x14.
///
/// Spec: [`docs/api/gemdos/memory/14-maddalt.md`](../../../../../docs/api/gemdos/memory/14-maddalt.md).
/// Reference: `bdos/umem.c:455 xmaddalt`.
///
/// Behaviour, in this order (matching EmuTOS `xmaddalt`):
///
/// 1. Round `size` *down* to a multiple of 4 (`size &= -4`).
///    EmuTOS does this unconditionally; the rounded size is what
///    enters the pool.
/// 2. Reject `size <= 0` with `-1`.
/// 3. Reject any block whose `start` lies below `end_stram`. EmuTOS
///    (`bdos/umem.c:467-469`) rejects
///    `(start < start_stram && start+size > start_stram) || start < end_stram`.
///    Because `start_stram <= end_stram`, the first clause is subsumed
///    by the second, so the effective rule is: **accept only blocks
///    that start at or above `end_stram`.** A block lying entirely
///    below ST-RAM (`start+size <= start_stram`) is therefore rejected,
///    not accepted.
/// 4. Walk `pmdalt.mp_mfl` and merge with an adjacent free block
///    (either end). On a successful merge, return `0` without
///    consuming a fresh MD.
/// 5. Otherwise allocate a new MD, install it at the head of
///    `pmdalt.mp_mfl`, and return `0`.
/// 6. If `xmgetmd` is exhausted, return `ENSMEM` (-39).
///
/// Caller-side parameter decoding lives in the dispatcher; this
/// function takes the already-decoded `start` (as a host pointer
/// reinterpreted as `Long` per Atari ABI) and `size`.
///
/// # Safety
///
/// Caller holds the BDOS giant lock. Callers must also have
/// validated that the bytes `[start, start+size)` are physical
/// addresses owned by the kernel (i.e. they came from the
/// device-tree memory map, not user-supplied), since `Maddalt` is
/// effectively a kernel-trust operation.
pub unsafe fn maddalt(start: Long, size: Long) -> Long {
    // 1. Round size down to a multiple of 4.
    let size = size & !3;

    // 2. Reject non-positive.
    if size <= 0 {
        return -1;
    }

    // Compute end with overflow protection. A wrap means the range
    // is invalid; treat as overlap.
    let end = match start.checked_add(size) {
        Some(e) => e,
        None => return -1,
    };

    // 3. Reject any block that starts below the ST-RAM pool end.
    //    EmuTOS accepts a new alt-RAM region only when `start >=
    //    end_stram`; a block wholly below ST-RAM (`end <= st_start`)
    //    still has `start < end_stram` and must be rejected. The
    //    `st_end > st_start` guard means "ST-RAM is populated" — with
    //    no ST-RAM pool there is nothing to sit below.
    let (st_start, st_end) = init::stram_range();
    if st_end > st_start && start < st_end {
        return -1;
    }

    // 4. Try to merge with an existing free MD on `pmdalt.mp_mfl`.
    let pmdalt = init::pmdalt();
    if pmdalt.is_null() {
        return ENSMEM;
    }
    // SAFETY: `pmdalt` is a valid MPB once `init::init` has run.
    let mp = unsafe { &mut *pmdalt };
    let mut p = mp.mp_mfl;
    while !p.is_null() {
        // SAFETY: list invariant.
        let p_start = unsafe { (*p).m_start };
        // SAFETY: list invariant.
        let p_len = unsafe { (*p).m_length };
        if p_start + p_len == start {
            // New block is just after a free one — extend at end.
            // SAFETY: list invariant.
            unsafe {
                (*p).m_length = p_len + size;
            }
            return E_OK;
        }
        if end == p_start {
            // New block is just before a free one — extend at start.
            // SAFETY: list invariant.
            unsafe {
                (*p).m_start = start;
                (*p).m_length = p_len + size;
            }
            return E_OK;
        }
        // SAFETY: list invariant.
        p = unsafe { (*p).m_link };
    }

    // 5. Allocate a fresh MD and prepend.
    // SAFETY: caller holds BDOS lock.
    let md = unsafe { xmgetmd_global() };
    if md.is_null() {
        return ENSMEM;
    }
    // SAFETY: `md` is a fresh, exclusive MD reference.
    unsafe {
        (*md).m_start = start;
        (*md).m_length = size;
        (*md).m_own = ptr::null_mut();
        (*md).m_link = mp.mp_mfl;
    }
    mp.mp_mfl = md;

    E_OK
}

/// `Mxalloc(amount, mode)` — GEMDOS 0x44.
///
/// Spec:
/// [`docs/api/gemdos/memory/44-mxalloc.md`](../../../../../docs/api/gemdos/memory/44-mxalloc.md).
/// Reference: `bdos/umem.c:234 xmxalloc`.
///
/// Behaviour mirrors EmuTOS:
///
/// 1. Mask `mode` with `MX_MODEMASK` (only the low two bits select
///    a pool / preference).
/// 2. If `amount == -1`, return the largest free block in the
///    selected pool(s) as a `Long` (cast as a pointer payload).
///    `MX_PREFSTRAM` / `MX_PREFTTRAM` return the maximum across
///    *both* pools — verified against TOS3 in EmuTOS.
/// 3. If `amount <= 0`, return `0` (NULL pointer).
/// 4. Otherwise call [`pool::ffit`] on the pool selected by `mode`,
///    falling back to the other pool for the `PREF*` modes.
/// 5. On success return `m_start`; on failure return `0`.
///
/// Returned `Long` is the start address of the allocated block (or
/// the largest free size for the probe path), packed as the AAPCS64
/// `x0` payload.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
pub unsafe fn mxalloc(amount: Long, mode: i16) -> Long {
    // ARCH03 3b: the protection nibble (FreeMiNT `m_xalloc`,
    // `sys/dosmem.c:209-221`) — `0` takes the caller's PRGFLAGS mode; a
    // kernel caller (no process) allocates for itself: `PROT_S`
    // (`sys/memory.c:150-159`).
    let prot = {
        let run = crate::bdos::process::current_kva();
        if run.is_null() && mode & tos_types::F_PROTMODE == 0 {
            tos_types::ProtMode::S
        } else {
            let flags = if run.is_null() {
                0
            } else {
                // SAFETY: `run` is the kernel alias of the running basepage.
                unsafe { (*run).p_flags }
            };
            tos_types::prot_from_mxalloc(mode, flags)
        }
    };
    // FreeMiNT `F_KEEP` (`sys/dosmem.c:131-135,237-242`): the block outlives
    // its last owner's termination ([`super::region::mark_keep`]).
    let keep = mode & tos_types::F_KEEP != 0;
    let mode = mode & MX_MODEMASK;
    let pmd = init::pmd();
    let pmdalt = init::pmdalt();

    // 2. Largest-free probe.
    if amount == -1 {
        let st = if !pmd.is_null() {
            // SAFETY: caller holds BDOS lock.
            unsafe { pool::largest_free(pmd) }
        } else {
            0
        };
        let alt = if !pmdalt.is_null() {
            // SAFETY: caller holds BDOS lock.
            unsafe { pool::largest_free(pmdalt) }
        } else {
            0
        };
        return match mode {
            MX_STRAM => st,
            MX_TTRAM => alt,
            MX_PREFSTRAM | MX_PREFTTRAM => {
                if st >= alt {
                    st
                } else {
                    alt
                }
            }
            _ => 0,
        };
    }

    // 3. Reject zero / negative.
    if amount <= 0 {
        return 0;
    }

    // 4. Pool selection.
    let cfg = PoolConfig::current();
    // SAFETY: pmd / pmdalt are stable for the BDOS lock duration.
    // Owner cookie: Phase 04 stamps every allocation as kernel-owned
    // via `current_owner()` (see KERNEL_PD_SENTINEL above). Phase 06
    // changes `current_owner()` to read `run`, with no further
    // changes to this call site.
    let owner = current_owner();
    let mut supply = unsafe { init::supply() };

    let try_pool = |mp: *mut tos_types::MPB,
                    sup: &mut super::mdpool::StaticMdSupply|
     -> *mut MD {
        if mp.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: caller holds BDOS lock; ffit walks only this pool.
        unsafe { pool::ffit(amount, mp, cfg, owner, sup) }
    };

    let m: *mut MD = match mode {
        MX_STRAM => try_pool(pmd, &mut supply),
        MX_TTRAM => try_pool(pmdalt, &mut supply),
        MX_PREFSTRAM => {
            let m = try_pool(pmd, &mut supply);
            if m.is_null() {
                try_pool(pmdalt, &mut supply)
            } else {
                m
            }
        }
        MX_PREFTTRAM => {
            let m = try_pool(pmdalt, &mut supply);
            if m.is_null() {
                try_pool(pmd, &mut supply)
            } else {
                m
            }
        }
        _ => ptr::null_mut(),
    };

    if m.is_null() {
        0
    } else {
        // SAFETY: m is a freshly returned MD on the allocated list.
        let (start, len) = unsafe { ((*m).m_start, (*m).m_length) };
        // A fresh block has no further owners (a stale record of a freed
        // block at the same address must not carry over).
        // SAFETY: caller holds the BDOS lock.
        unsafe {
            super::region::forget(start);
            if keep {
                // Cannot fail: one place per MD, every flag names a live block.
                super::region::mark_keep(start);
            }
            // Its protection mode, and its owner's rights (ARCH03 3b).
            crate::bdos::process::memprot::on_alloc(start as u32 as u64, len as u32 as u64, prot);
        }
        start
    }
}

/// Allocate `amount` bytes from the pool(s) `mode` selects, as [`mxalloc`]
/// does, but with the block **starting on an `align`-byte boundary**
/// (`align` a power of two ≥ the pool grain). Returns the block's `m_start`
/// (a multiple of `align`), or `0` when no pool can supply
/// `amount + align` bytes. The block is an ordinary allocated block —
/// `Mfree`/`Mshrink` take its address, `Pterm` reclaims it by owner.
///
/// Kernel-internal, not a GEMDOS function: the program loader takes the TPA
/// with it, because a position-independent image must keep its link-time
/// page offsets (ARCH03 stage 1). It over-allocates by `align`, gives the
/// slack in front of the boundary back ([`pool::trim_front`]) and then the
/// slack behind the block ([`pool::shrinkit`]).
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
pub unsafe fn mxalloc_aligned(amount: Long, mode: i16, align: Long) -> Long {
    if amount <= 0 || align <= 0 || align & (align - 1) != 0 {
        return 0;
    }
    let Some(want) = amount.checked_add(align) else {
        return 0;
    };
    // SAFETY: caller holds the BDOS lock.
    let raw = unsafe { mxalloc(want, mode) };
    if raw == 0 {
        return 0;
    }
    // SAFETY: caller holds the BDOS lock.
    let mpb = unsafe { find_mpb(raw) };
    // SAFETY: caller holds the BDOS lock; walk the allocated list.
    let mut p = unsafe { (*mpb).mp_mal };
    while !p.is_null() && unsafe { (*p).m_start } != raw {
        // SAFETY: list invariant.
        p = unsafe { (*p).m_link };
    }
    if p.is_null() {
        return 0; // cannot happen: `raw` was just allocated
    }
    let base = (raw + align - 1) & !(align - 1);
    // SAFETY: list invariant.
    let raw_len = unsafe { (*p).m_length };
    let mut supply = unsafe { init::supply() };
    if base > raw {
        // SAFETY: `p` is on `mp_mal`; `0 < base - raw < align < m_length`.
        if unsafe { pool::trim_front(p, mpb, base - raw, &mut supply) } < 0 {
            // SAFETY: `p` is still the whole block at `raw`.
            unsafe {
                pool::freeit(p, mpb, &mut supply);
                crate::bdos::process::memprot::on_freed(raw as u32 as u64, raw_len as u32 as u64);
            }
            return 0;
        }
    }
    let mask = PoolConfig::current().align_mask;
    let keep = (amount + mask) & !mask;
    // SAFETY: list invariant; `p` now starts at `base`.
    let have = unsafe { (*p).m_length };
    if have > keep {
        // A failed tail split only leaves the block longer than asked.
        // SAFETY: `p` is on `mp_mal`; `0 < keep < have`.
        let _ = unsafe { pool::shrinkit(p, mpb, keep, &mut supply) };
    }
    // ARCH03 3b: the record follows the block to its aligned start; the
    // slack given back on either side is invalid for everybody again.
    // SAFETY: caller holds the BDOS lock.
    unsafe {
        crate::bdos::process::memprot::rekey(raw as u32 as u64, base as u32 as u64);
        crate::bdos::process::memprot::touch(raw as u32 as u64, (raw as u32 as u64) + raw_len as u32 as u64);
        crate::bdos::process::memprot::sync();
    }
    base
}

/// The largest free block of `mp` as `(m_start, m_length)` — the block
/// EmuTOS `alloc_tpa` gets with `ffit(ffit(-1L, mp), mp)`
/// (`bdos/proc.c:466,476,486,493`): `ffit(-1)` reports the largest length,
/// and `ffit(length)` then takes the **first** free block at least that long,
/// i.e. the first one of maximal length. `None` for an empty free list.
///
/// # Safety
///
/// Caller holds the BDOS giant lock (read-only list walk).
pub unsafe fn largest_free_block(mp: *mut MPB) -> Option<(Long, Long)> {
    if mp.is_null() {
        return None;
    }
    let mut best: Option<(Long, Long)> = None;
    // SAFETY: caller holds the BDOS lock.
    let mut q = unsafe { (*mp).mp_mfl };
    while !q.is_null() {
        // SAFETY: list invariant.
        let (start, len) = unsafe { ((*q).m_start, (*q).m_length) };
        if len > 0 && best.is_none_or(|(_, l)| len > l) {
            best = Some((start, len));
        }
        // SAFETY: list invariant.
        q = unsafe { (*q).m_link };
    }
    best
}

/// Allocate the **whole** largest free block of one pool (`MX_STRAM` or
/// `MX_TTRAM`), starting on an `align`-byte boundary: the program TPA of
/// EmuTOS `alloc_tpa` (`md = ffit(size, &pmd); return md->m_start`,
/// `bdos/proc.c:486-487,493-494`), whose length becomes `p_hitpa - p_lowtpa`
/// (`*avail = size`, `init_pd_fields`, `:374`).
///
/// The block is taken with [`mxalloc`] (so it carries the caller's owner tag
/// and its protection record like every allocation) and its front slack up to
/// the boundary goes back to the pool ([`pool::trim_front`]) — a NewTOS step:
/// the position-independent image keeps its link-time page offsets (ARCH03
/// stage 1). With protection on the pool grain is already one page, so there
/// is no slack. Returns `(m_start, m_length)` of the aligned block, or `None`
/// when the pool has no free block longer than its slack.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
pub unsafe fn mxalloc_largest(mode: i16, align: Long) -> Option<(Long, Long)> {
    if align <= 0 || align & (align - 1) != 0 {
        return None;
    }
    let mpb = match mode {
        MX_STRAM => init::pmd(),
        MX_TTRAM => init::pmdalt(),
        _ => return None,
    };
    // SAFETY: caller holds the BDOS lock.
    let (_, len) = unsafe { largest_free_block(mpb) }?;
    // `ffit` rounds the amount up to the grain; a block length is a multiple
    // of it, so this takes exactly the largest block (the first of that length).
    let mask = PoolConfig::current().align_mask;
    let amount = len & !mask;
    if amount <= 0 {
        return None;
    }
    // SAFETY: caller holds the BDOS lock.
    let raw = unsafe { mxalloc(amount, mode) };
    if raw == 0 {
        return None;
    }
    // SAFETY: caller holds the BDOS lock; `raw` was just allocated on `mpb`.
    let mut p = unsafe { (*mpb).mp_mal };
    while !p.is_null() && unsafe { (*p).m_start } != raw {
        // SAFETY: list invariant.
        p = unsafe { (*p).m_link };
    }
    if p.is_null() {
        return None; // cannot happen: `raw` was just allocated
    }
    let base = (raw + align - 1) & !(align - 1);
    // SAFETY: list invariant.
    let raw_len = unsafe { (*p).m_length };
    if base > raw {
        let mut supply = unsafe { init::supply() };
        // SAFETY: `p` is on `mp_mal`; `base - raw < align`, and the block
        // must keep at least one grain behind the boundary.
        if base - raw >= raw_len || unsafe { pool::trim_front(p, mpb, base - raw, &mut supply) } < 0 {
            // SAFETY: `p` is still the whole block at `raw`.
            unsafe {
                pool::freeit(p, mpb, &mut supply);
                super::region::forget(raw);
                crate::bdos::process::memprot::on_freed(raw as u32 as u64, raw_len as u32 as u64);
            }
            return None;
        }
        // ARCH03 3b: the record follows the block to its aligned start; the
        // slack given back is invalid for everybody again.
        // SAFETY: caller holds the BDOS lock.
        unsafe {
            crate::bdos::process::memprot::rekey(raw as u32 as u64, base as u32 as u64);
            crate::bdos::process::memprot::touch(raw as u32 as u64, base as u32 as u64);
            crate::bdos::process::memprot::sync();
        }
    }
    // SAFETY: list invariant; `p` now starts at `base`.
    let have = unsafe { (*p).m_length };
    Some((base, have))
}

/// Is `addr` inside a block on either pool's **allocated** list?
///
/// The "live user memory" test of the stage-1 shared address space
/// ([`crate::bdos::process::usermem::resolve_kva`]): a pool address is
/// mapped for every process, but only an allocated block is anybody's.
///
/// # Safety
///
/// Caller holds the BDOS giant lock (read-only list walk).
pub unsafe fn is_allocated(addr: u64) -> bool {
    for mpb in [init::pmd(), init::pmdalt()] {
        if mpb.is_null() {
            continue;
        }
        // SAFETY: caller holds the BDOS lock.
        let mut p = unsafe { (*mpb).mp_mal };
        while !p.is_null() {
            // SAFETY: list invariant.
            let (start, len) = unsafe { ((*p).m_start as u64, (*p).m_length as u64) };
            if addr >= start && addr - start < len {
                return true;
            }
            // SAFETY: list invariant.
            p = unsafe { (*p).m_link };
        }
    }
    false
}

/// `Malloc(amount)` — GEMDOS 0x48.
///
/// Spec:
/// [`docs/api/gemdos/memory/48-malloc.md`](../../../../../docs/api/gemdos/memory/48-malloc.md).
/// Reference: `bdos/umem.c:116 xmalloc`.
///
/// Policy wrapper around [`mxalloc`]: in EmuTOS the mode comes from
/// the running process's `PF_TTRAMMEM` flag. Until Phase 06
/// introduces real PDs, the spec NewTOS-notes (§ "NewTOS
/// implementation notes") declare the kernel sentinel PD to have
/// `PF_TTRAMMEM = 1`, so we forward unconditionally with
/// `MX_PREFTTRAM`: TT-RAM preferred, fall back to ST-RAM. This also
/// makes `Malloc(-1)` return the largest free block across both
/// pools, matching the NewTOS-notes contract.
///
/// Phase 06 replaces this with `if run->p_flags & PF_TTRAMMEM {
/// MX_PREFTTRAM } else { MX_STRAM }`.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
pub unsafe fn malloc(amount: Long) -> Long {
    // SAFETY: forwarded.
    unsafe { mxalloc(amount, MX_PREFTTRAM) }
}

/// Locate the MPB owning the byte at `addr`, mirroring
/// `bdos/umem.c:84 find_mpb`.
///
/// EmuTOS routes ST-RAM addresses to `pmd` and everything else to
/// `pmdalt`. We replicate that test using the cached ST-RAM range
/// from [`init::stram_range`]; the alt-RAM pool serves as the
/// catch-all for non-ST-RAM addresses.
///
/// `init::pmd` / `init::pmdalt` return pointers into the static MPB
/// cells, never null — even before `init::init` runs the cells exist
/// with empty free/alloc lists. The address-range short-circuit is
/// sound because [`maddalt`] rejects any alt-RAM region that
/// overlaps `[stram_start, stram_end)`, so addresses cannot
/// interleave between pools.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
unsafe fn find_mpb(addr: Long) -> *mut MPB {
    let (st_start, st_end) = init::stram_range();
    if st_end > st_start && addr >= st_start && addr < st_end {
        init::pmd()
    } else {
        init::pmdalt()
    }
}

/// Re-tag the owner cookie of the allocated block starting at `addr`,
/// mirroring EmuTOS `set_owner` (`bdos/umem.c:587`). Walks the owning
/// pool's allocated list for the MD whose `m_start == addr` and stamps
/// its `m_own`. Returns `true` if the block was found.
///
/// `Pexec` uses this to hand a freshly-`Mxalloc`'d environment block to
/// the process that should free it: the **child** for `PE_LOADGO`
/// (`owner = p`, `bdos/proc.c:305`), so the child's `Pterm`
/// (`free_all_owned`) reclaims it. Blocks allocated with the wrong
/// running-process cookie would otherwise leak.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
pub unsafe fn set_owner(addr: Long, owner: *mut c_void) -> bool {
    // SAFETY: caller holds the BDOS lock.
    let mpb = unsafe { find_mpb(addr) };
    if mpb.is_null() {
        return false;
    }
    // SAFETY: caller holds the BDOS lock; walk the allocated list.
    let mut p = unsafe { (*mpb).mp_mal };
    while !p.is_null() {
        // SAFETY: list invariant.
        if unsafe { (*p).m_start } == addr {
            // SAFETY: `p` is a live MD on `mp_mal`.
            unsafe {
                (*p).m_own = owner;
                crate::bdos::process::memprot::on_owner_change(addr as u32 as u64);
            }
            return true;
        }
        // SAFETY: list invariant.
        p = unsafe { (*p).m_link };
    }
    false
}

/// `Mfree(addr)` — GEMDOS 0x49.
///
/// Spec:
/// [`docs/api/gemdos/memory/49-mfree.md`](../../../../../docs/api/gemdos/memory/49-mfree.md).
/// Reference: `bdos/umem.c:135 xmfree`.
///
/// Behaviour:
/// 1. Locate the owning pool via [`find_mpb`].
/// 2. Walk `mp_mal` for an MD whose `m_start == addr`.
/// 3. Return [`EIMBA`] if no pool matches or the address is not on
///    the allocated list.
/// 4. Otherwise hand the MD to [`pool::freeit`] (which performs
///    address-ordered insertion into `mp_mfl` and merges adjacent
///    free MDs via `xmfremd`).
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
pub unsafe fn mfree(addr: Long) -> Long {
    // 1. Pool selection.
    let mpb = unsafe { find_mpb(addr) };
    if mpb.is_null() {
        return EIMBA;
    }

    // 2. Locate MD on the allocated list.
    // SAFETY: caller holds BDOS lock.
    let mut p = unsafe { (*mpb).mp_mal };
    while !p.is_null() {
        // SAFETY: list invariant.
        if unsafe { (*p).m_start } == addr {
            break;
        }
        // SAFETY: list invariant.
        p = unsafe { (*p).m_link };
    }
    if p.is_null() {
        return EIMBA;
    }

    // 4. Free — and forget the block's further owners (ARCH03 2d) and its
    //    rights: nobody may reach it any more (ARCH03 3b, I8).
    // SAFETY: list invariant.
    let len = unsafe { (*p).m_length };
    let mut supply = unsafe { init::supply() };
    // SAFETY: caller holds BDOS lock; `p` is an MD on `mp_mal`.
    unsafe {
        pool::freeit(p, mpb, &mut supply);
        super::region::forget(addr);
        crate::bdos::process::memprot::on_freed(addr as u32 as u64, len as u32 as u64);
    }
    E_OK
}

/// `Mshrink(_n, blk, len)` — GEMDOS 0x4A.
///
/// Spec:
/// [`docs/api/gemdos/memory/4a-mshrink.md`](../../../../../docs/api/gemdos/memory/4a-mshrink.md).
/// Reference: `bdos/umem.c:170 xsetblk`.
///
/// Behaviour mirrors EmuTOS `xsetblk`:
///
/// 1. Locate the owning pool via [`find_mpb`]; return [`EIMBA`] if
///    `blk` is not in any pool.
/// 2. Walk `mp_mal` for an MD whose `m_start == blk`; return
///    [`EIMBA`] if no allocated block starts at `blk`.
/// 3. Round `len` up to the pool's alignment grain (NewTOS uses
///    16 bytes for both pools per [`pool::PoolConfig::AARCH64`];
///    EmuTOS uses 2 / 4 bytes).
/// 4. Compare against current block length:
///    - `m_length < len` → [`EGSBF`] (cannot grow).
///    - `m_length == len` → `E_OK` (no-op).
///    - `len == 0` → equivalent to `Mfree` (`freeit`).
///    - `m_length > len` → [`pool::shrinkit`].
/// 5. Return `E_OK` on success; [`ERROR`] (-1) when `shrinkit`
///    cannot allocate the tail MD.
///
/// The `_n` argument is the historic CP/M block-number parameter;
/// EmuTOS ignores it and so do we.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
pub unsafe fn mshrink(_n: i16, blk: Long, len: Long) -> Long {
    // 1. Pool selection.
    let mpb = unsafe { find_mpb(blk) };
    if mpb.is_null() {
        return EIMBA;
    }

    // 2. Locate MD on the allocated list.
    // SAFETY: caller holds BDOS lock.
    let mut p = unsafe { (*mpb).mp_mal };
    while !p.is_null() {
        // SAFETY: list invariant.
        if unsafe { (*p).m_start } == blk {
            break;
        }
        // SAFETY: list invariant.
        p = unsafe { (*p).m_link };
    }
    if p.is_null() {
        return EIMBA;
    }

    // 3. Round `len` up to the alignment grain. Saturating on
    //    overflow keeps the comparison branches deterministic; the
    //    grow-check below will reject anything bigger than the
    //    block's current length anyway.
    let mask = pool::PoolConfig::current().align_mask;
    let len = len
        .checked_add(mask)
        .map(|v| v & !mask)
        .unwrap_or(Long::MAX);

    // 4. Three-way compare.
    // SAFETY: list invariant.
    let m_len = unsafe { (*p).m_length };
    if m_len < len {
        return EGSBF;
    }
    if m_len == len {
        return E_OK;
    }
    if len == 0 {
        // Equivalent to Mfree(blk).
        let mut supply = unsafe { init::supply() };
        // SAFETY: caller holds BDOS lock; `p` is on `mp_mal`.
        unsafe {
            pool::freeit(p, mpb, &mut supply);
            super::region::forget(blk);
            crate::bdos::process::memprot::on_freed(blk as u32 as u64, m_len as u32 as u64);
        }
        return E_OK;
    }

    // 5. Real shrink — split off the tail.
    let mut supply = unsafe { init::supply() };
    // SAFETY: caller holds BDOS lock; `p` is on `mp_mal`; `len`
    // satisfies `0 < len <= m_len` by the checks above.
    let rc = unsafe { pool::shrinkit(p, mpb, len, &mut supply) };
    // The tail went back to the pool: invalid for everybody (ARCH03 3b, I8 —
    // FreeMiNT `shrink_region` marks it `PROT_I`, `sys/memory.c:1076,1097`).
    // SAFETY: caller holds BDOS lock.
    unsafe {
        let b = blk as u32 as u64;
        crate::bdos::process::memprot::touch(b, b + m_len as u32 as u64);
        crate::bdos::process::memprot::sync();
    }
    if rc < 0 {
        ERROR
    } else {
        E_OK
    }
}
