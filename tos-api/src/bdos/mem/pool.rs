//! `MPB` operations: `ffit`, `freeit`, `shrinkit`, `largest_free`.
//!
//! Direct port of `bdos/iumem.c` from EmuTOS. The functions are
//! `unsafe` because they manipulate raw `MD`/`MPB` pointers that
//! escape Rust's borrow checker, but the algorithms themselves are
//! pure data-structure operations on a singly-linked list and can
//! be reasoned about independently of any kernel state.
//!
//! Spec: [`docs/api/structures/md.md`](../../../../../docs/api/structures/md.md)
//! § "Allocator semantics".

use core::ffi::c_void;
use core::ptr;

use tos_types::{Long, MD, MPB};

use super::mdpool::MdSupply;

/// Per-pool configuration. The only knob today is alignment.
///
/// EmuTOS uses 2-byte ST-RAM and 4-byte alt-RAM alignment. NewTOS
/// rounds to 16 bytes for both pools (AArch64 SIMD baseline); see
/// [`docs/api/gemdos/memory/4a-mshrink.md`](../../../../../docs/api/gemdos/memory/4a-mshrink.md)
/// for the documented divergence.
#[derive(Clone, Copy, Debug)]
pub struct PoolConfig {
    /// Mask used to round allocation sizes up:
    /// `aligned = (amount + mask) & !mask`. Must be `2^n - 1`.
    pub align_mask: Long,
}

impl PoolConfig {
    /// 16-byte alignment, the NewTOS default for both pools.
    pub const AARCH64: Self = Self { align_mask: 15 };

    /// The grain in force: [`Self::AARCH64`], or one 4-KiB page while memory
    /// protection is on (ARCH03 3b, E16 — FreeMiNT rounds regions to its
    /// `QUANTUM`, `sys/mint/mem.h:152`, so no page has two owners).
    pub fn current() -> Self {
        Self { align_mask: crate::bdos::process::memprot::pool_align_mask() }
    }
}

/// First-fit allocator — port of `MD *ffit(long amount, MPB *mp)` in
/// `bdos/iumem.c`.
///
/// Searches `mp->mp_mfl` for the first free MD whose `m_length` is
/// at least `amount` (rounded up per `cfg.align_mask`). On success,
/// the MD is split if it is larger than the request, removed from
/// the free list, prepended to `mp->mp_mal`, tagged with `owner`,
/// and returned.
///
/// Returns `null_mut` when no free block fits, when the free list
/// is empty, or when the meta-pool is exhausted during the split.
///
/// To probe for the largest free block, use [`largest_free`] —
/// EmuTOS overloads `ffit(-1, mp)` for this; NewTOS keeps the two
/// operations separate.
///
/// # Safety
///
/// `mp` must point to a valid, properly-linked MPB. All MDs reachable
/// from `mp->mp_mfl` and `mp->mp_mal` must remain live for the
/// duration of the call. `supply` must not alias any MD on either
/// list. The caller holds the BDOS lock.
pub unsafe fn ffit(
    amount: Long,
    mp: *mut MPB,
    cfg: PoolConfig,
    owner: *mut c_void,
    supply: &mut dyn MdSupply,
) -> *mut MD {
    debug_assert!(amount >= 0, "use largest_free for the probe path");
    debug_assert!(!mp.is_null());

    // Round size up to the configured alignment grain.
    let mask = cfg.align_mask;
    let amount = amount
        .checked_add(mask)
        .map(|v| v & !mask)
        .unwrap_or(Long::MAX);

    // SAFETY: caller guarantees `mp` is a valid MPB.
    let mp_ref = unsafe { &mut *mp };

    // Walk the free list, tracking `prev_link` — the address of the
    // pointer that points at the current node. This is the Rust
    // equivalent of EmuTOS's `p = (MD*)mp; q = mp->mp_mfl; ...; p =
    // q; q = p->m_link;` trick where the MPB header is treated as
    // an honorary MD whose `m_link` field is `mp_mfl`.
    let mut prev_link: *mut *mut MD = &mut mp_ref.mp_mfl;
    let mut q = mp_ref.mp_mfl;

    while !q.is_null() {
        // SAFETY: `q` is a valid MD on the free list.
        let q_len = unsafe { (*q).m_length };
        if q_len >= amount {
            break;
        }
        // SAFETY: see above.
        prev_link = unsafe { &mut (*q).m_link };
        q = unsafe { (*q).m_link };
    }

    if q.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: `q` is a valid MD by the loop invariant.
    let q_len = unsafe { (*q).m_length };

    if q_len == amount {
        // Take the whole block: snip it out of the free list.
        // SAFETY: `prev_link` is a live pointer location into either
        // the MPB header or a previous free MD, and `q->m_link` is
        // a valid pointer field.
        unsafe {
            *prev_link = (*q).m_link;
        }
    } else {
        // Split the block. Allocate a new MD describing the tail.
        // SAFETY: trait contract.
        let p1 = unsafe { supply.xmgetmd() };
        if p1.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: `q` and `p1` are valid, distinct MDs.
        unsafe {
            (*p1).m_length = q_len - amount;
            (*p1).m_start = (*q).m_start + amount;
            (*p1).m_link = (*q).m_link;
            (*p1).m_own = ptr::null_mut();
            *prev_link = p1;
            (*q).m_length = amount;
        }
    }

    // Prepend `q` to the allocated list and tag the owner.
    // SAFETY: `q` is detached from the free list; `mp_ref` is live.
    unsafe {
        (*q).m_link = mp_ref.mp_mal;
        mp_ref.mp_mal = q;
        (*q).m_own = owner;
    }
    q
}

/// Return the size, in bytes, of the largest free block in `mp`.
///
/// EmuTOS encodes this as `ffit(-1, mp)` returning a pointer-shaped
/// integer; NewTOS exposes a clean `Long` instead.
///
/// # Safety
///
/// `mp` must point to a valid, properly-linked MPB. The free list
/// must remain stable for the duration of the call.
pub unsafe fn largest_free(mp: *mut MPB) -> Long {
    debug_assert!(!mp.is_null());
    // SAFETY: caller contract.
    let mut q = unsafe { (*mp).mp_mfl };
    let mut maxval: Long = 0;
    while !q.is_null() {
        // SAFETY: list invariants.
        let len = unsafe { (*q).m_length };
        if len > maxval {
            maxval = len;
        }
        // SAFETY: list invariants.
        q = unsafe { (*q).m_link };
    }
    maxval
}

/// Free `m` back to its MPB. Port of
/// `void freeit(MD *m, MPB *mp)` in `bdos/iumem.c`.
///
/// Locates the MD on `mp->mp_mal` by `m_start` equality (matches
/// EmuTOS exactly — pointer equality would not survive a caller
/// holding a stale alias), removes it, inserts it into the
/// address-ordered free list, and coalesces with any contiguous
/// neighbours. The redundant MD created by coalescing is returned
/// to `supply`.
///
/// On a wild `m` (no allocated MD has the same `m_start`) the
/// function silently returns — matching the Atari ABI. The trap
/// wrapper [`super::traps::mfree`] (D10) is responsible for the
/// `EIMBA` error code.
///
/// # Safety
///
/// `m` must point to a valid `MD` whose `m_start` is what the
/// caller intends to free. `mp` must be the MPB that allocated
/// `m`. Both lists must be well-formed.
pub unsafe fn freeit(m: *mut MD, mp: *mut MPB, supply: &mut dyn MdSupply) {
    debug_assert!(!m.is_null());
    debug_assert!(!mp.is_null());

    // SAFETY: caller contract.
    let m_start = unsafe { (*m).m_start };

    // Find the MD on the allocated list whose start matches.
    // SAFETY: caller contract; lists are well-formed.
    let mp_ref = unsafe { &mut *mp };
    let mut prev: *mut MD = ptr::null_mut();
    let mut p = mp_ref.mp_mal;
    while !p.is_null() {
        // SAFETY: list invariant.
        if unsafe { (*p).m_start } == m_start {
            break;
        }
        prev = p;
        // SAFETY: list invariant.
        p = unsafe { (*p).m_link };
    }
    if p.is_null() {
        // EmuTOS prints a debug line and returns. We do the same;
        // the caller distinguishes "wild pointer" via its own
        // pre-check and reports `EIMBA`.
        return;
    }

    // Detach `p` from the allocated list.
    // SAFETY: `prev` is null or a valid MD; `p` is valid.
    unsafe {
        let p_link = (*p).m_link;
        if !prev.is_null() {
            (*prev).m_link = p_link;
        } else {
            mp_ref.mp_mal = p_link;
        }
    }

    // Find insertion point on the address-ordered free list.
    let p_start = unsafe { (*p).m_start };
    let mut q: *mut MD = ptr::null_mut();
    let mut f = mp_ref.mp_mfl;
    while !f.is_null() {
        // SAFETY: list invariant.
        if p_start <= unsafe { (*f).m_start } {
            break;
        }
        q = f;
        // SAFETY: list invariant.
        f = unsafe { (*f).m_link };
    }

    // Insert `p` between `q` (or list head) and `f`.
    // SAFETY: `p`, `q`, `f` are valid or null; `mp_ref` is live.
    unsafe {
        (*p).m_link = f;
        if !q.is_null() {
            (*q).m_link = p;
        } else {
            mp_ref.mp_mfl = p;
        }
        // Reset owner so probes see "free".
        (*p).m_own = ptr::null_mut();
    }

    // Coalesce upwards: p + p.length == f.start.
    if !f.is_null() {
        // SAFETY: list invariants.
        unsafe {
            let p_len = (*p).m_length;
            if (*p).m_start + p_len == (*f).m_start {
                (*p).m_length = p_len + (*f).m_length;
                (*p).m_link = (*f).m_link;
                supply.xmfremd(f);
            }
        }
    }

    // Coalesce downwards: q + q.length == p.start.
    if !q.is_null() {
        // SAFETY: list invariants.
        unsafe {
            let q_len = (*q).m_length;
            if (*q).m_start + q_len == (*p).m_start {
                (*q).m_length = q_len + (*p).m_length;
                (*q).m_link = (*p).m_link;
                supply.xmfremd(p);
            }
        }
    }
}

/// Shrink `m` to `newlen` bytes; the released tail is returned to
/// the free list. Port of
/// `WORD shrinkit(MD *m, MPB *mp, LONG newlen)` in `bdos/iumem.c`.
///
/// Returns `0` on success, `-1` when the meta-pool is exhausted.
///
/// EmuTOS implements this as "create a temporary allocated MD for
/// the tail, then call `freeit` on it"; NewTOS does the same so the
/// coalescing logic stays in one place.
///
/// # Safety
///
/// `m` must be an allocated MD on `mp->mp_mal`. `newlen` must be
/// `>= 0` and `<= m->m_length`; the trap wrapper validates this.
pub unsafe fn shrinkit(
    m: *mut MD,
    mp: *mut MPB,
    newlen: Long,
    supply: &mut dyn MdSupply,
) -> i16 {
    debug_assert!(!m.is_null());
    debug_assert!(!mp.is_null());
    debug_assert!(newlen >= 0);

    // SAFETY: trait contract.
    let f = unsafe { supply.xmgetmd() };
    if f.is_null() {
        return -1;
    }

    // SAFETY: `m` and `f` are valid, distinct MDs.
    unsafe {
        let m_start = (*m).m_start;
        let m_len = (*m).m_length;
        debug_assert!(newlen <= m_len);
        (*f).m_start = m_start + newlen;
        (*f).m_length = m_len - newlen;
        (*f).m_own = (*m).m_own;
        // Splice the temporary MD onto the allocated list.
        (*f).m_link = (*mp).mp_mal;
        (*mp).mp_mal = f;
        // Update the original MD.
        (*m).m_length = newlen;
    }

    // SAFETY: `f` is now an allocated MD on `mp->mp_mal`.
    unsafe {
        freeit(f, mp, supply);
    }
    0
}

/// Give the first `pad` bytes of the allocated block `m` back to the free
/// list, so the block afterwards starts `pad` bytes later — the mirror image
/// of [`shrinkit`], with the same "temporary allocated MD, then [`freeit`]"
/// technique so the coalescing logic stays in one place.
///
/// Not an EmuTOS primitive: EmuTOS never needs a block to start on a
/// particular boundary. NewTOS does exactly once — the program loader needs
/// its TPA to begin on a 4-KiB page (a position-independent image keeps its
/// link-time page offsets, ARCH03 stage 1) — and gets it with
/// `Mxalloc(len + page)`, then this, then [`shrinkit`]. The result is an
/// ordinary allocated block; nothing about it is observable except its
/// address.
///
/// Returns `0` on success, `-1` when the meta-pool is exhausted (the block
/// is then unchanged).
///
/// # Safety
///
/// `m` must be an allocated MD on `mp->mp_mal`; `0 < pad < m->m_length`.
pub unsafe fn trim_front(
    m: *mut MD,
    mp: *mut MPB,
    pad: Long,
    supply: &mut dyn MdSupply,
) -> i16 {
    debug_assert!(!m.is_null());
    debug_assert!(!mp.is_null());
    debug_assert!(pad > 0);

    // SAFETY: trait contract.
    let f = unsafe { supply.xmgetmd() };
    if f.is_null() {
        return -1;
    }
    // SAFETY: `m` and `f` are valid, distinct MDs.
    unsafe {
        let m_start = (*m).m_start;
        let m_len = (*m).m_length;
        debug_assert!(pad < m_len);
        (*f).m_start = m_start;
        (*f).m_length = pad;
        (*f).m_own = (*m).m_own;
        (*f).m_link = (*mp).mp_mal;
        (*mp).mp_mal = f;
        (*m).m_start = m_start + pad;
        (*m).m_length = m_len - pad;
    }
    // SAFETY: `f` is now an allocated MD on `mp->mp_mal`.
    unsafe {
        freeit(f, mp, supply);
    }
    0
}

