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
/// 3. Reject if the new range `[start, start+size)` overlaps the
///    ST-RAM pool. EmuTOS implements this with two checks against
///    `start_stram` / `end_stram`; we use one half-open intersection
///    test.
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

    // 3. Reject overlap with ST-RAM. Half-open intervals overlap iff
    //    `a.start < b.end && b.start < a.end`.
    let (st_start, st_end) = init::stram_range();
    if st_end > st_start && start < st_end && st_start < end {
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
    let cfg = PoolConfig::AARCH64;
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
        unsafe { (*m).m_start }
    }
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

    // 4. Free.
    let mut supply = unsafe { init::supply() };
    // SAFETY: caller holds BDOS lock; `p` is an MD on `mp_mal`.
    unsafe { pool::freeit(p, mpb, &mut supply) };
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
    let mask = pool::PoolConfig::AARCH64.align_mask;
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
        unsafe { pool::freeit(p, mpb, &mut supply) };
        return E_OK;
    }

    // 5. Real shrink — split off the tail.
    let mut supply = unsafe { init::supply() };
    // SAFETY: caller holds BDOS lock; `p` is on `mp_mal`; `len`
    // satisfies `0 < len <= m_len` by the checks above.
    let rc = unsafe { pool::shrinkit(p, mpb, len, &mut supply) };
    if rc < 0 {
        ERROR
    } else {
        E_OK
    }
}
