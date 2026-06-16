//! MD meta-allocator (`xmgetmd` / `xmfremd`).
//!
//! Mirrors EmuTOS `bdos/osmem.c`: `MD`s are not allocated one at a
//! time — they live in `MDBlock`s holding three MDs plus a chain
//! link, drawn from a static arena. See
//! [`docs/api/structures/md.md`](../../../../../docs/api/structures/md.md)
//! § "MD meta-allocator".
//!
//! D5 populates the concrete `MDBlock` arena and `xmgetmd`/`xmfremd`
//! functions. This module already exports the [`MdSupply`] trait so
//! that the pool primitives in [`super::pool`] can be written and
//! tested against any source of fresh MDs (a static arena in the
//! kernel, a `Vec`-backed mock on the host).

use core::cell::UnsafeCell;
use core::ptr;

use tos_types::MD;

/// Source of fresh `MD` slots for the pool primitives.
///
/// EmuTOS uses two free-standing functions, `xmgetmd()` and
/// `xmfremd(MD*)`, that share global state (`mdbroot`). NewTOS
/// abstracts them as a trait so the allocator algorithms remain
/// pure functions of their inputs.
///
/// All implementors must be safe to call from inside the BDOS giant
/// lock; see PHASE-04 D6 for locking discipline.
pub trait MdSupply {
    /// Allocate a fresh, uninitialised `MD` slot. Returns `null_mut`
    /// when the meta-pool is exhausted.
    ///
    /// # Safety
    ///
    /// The returned pointer is valid for read/write of an `MD` until
    /// the matching [`MdSupply::xmfremd`] call. The caller must
    /// initialise every field before the MD is observed by any other
    /// code path.
    unsafe fn xmgetmd(&mut self) -> *mut MD;

    /// Return an `MD` slot to the meta-pool.
    ///
    /// # Safety
    ///
    /// `md` must have been obtained from [`MdSupply::xmgetmd`] on the
    /// same supply instance and must not be referenced afterwards.
    unsafe fn xmfremd(&mut self, md: *mut MD);
}

/// Number of MDs per `MdBlock`. Matches EmuTOS `MDS_PER_BLOCK = 3`.
pub const MDS_PER_BLOCK: usize = 3;

/// Number of `MdBlock`s in the static arena. With 3 MDs each this
/// gives 256 MDs head-room for ~86 blocks plus one spare; ample for
/// pool seeding (one MD per pool plus split-tails) and Phase 06
/// per-process allocations.
///
/// EmuTOS sizes its `osmem` pool for 118 blocks but shares it with
/// OFD/DND tables. NewTOS gives MDs their own arena.
pub const MD_BLOCK_COUNT: usize = 86;

/// One slot in an `MdBlock`. The `slot` word *must* immediately
/// follow `md` in memory: EmuTOS's `xmfremd` reconstructs the index
/// by reading `*(WORD *)(md + 1)`. The Rust port does not rely on
/// that pointer arithmetic — we walk the entries — but preserving
/// the layout means C ports linked against `tos-types` see identical
/// offsets.
#[repr(C)]
struct MdEntry {
    md: MD,
    /// Slot index within the parent `MdBlock` (0..3) when allocated,
    /// `-1` when free. The `i16` width matches EmuTOS `WORD`.
    slot: i16,
    _pad: i16,
}

/// Three MDs grouped with a chain link. Mirrors `struct _mdb` in
/// `bdos/osmem.c`.
#[repr(C)]
struct MdBlock {
    next: *mut MdBlock,
    entries: [MdEntry; MDS_PER_BLOCK],
}

impl MdBlock {
    const EMPTY: MdBlock = MdBlock {
        next: ptr::null_mut(),
        entries: [
            MdEntry {
                md: MD {
                    m_link: ptr::null_mut(),
                    m_start: 0,
                    m_length: 0,
                    m_own: ptr::null_mut(),
                },
                slot: -1,
                _pad: 0,
            },
            MdEntry {
                md: MD {
                    m_link: ptr::null_mut(),
                    m_start: 0,
                    m_length: 0,
                    m_own: ptr::null_mut(),
                },
                slot: -1,
                _pad: 0,
            },
            MdEntry {
                md: MD {
                    m_link: ptr::null_mut(),
                    m_start: 0,
                    m_length: 0,
                    m_own: ptr::null_mut(),
                },
                slot: -1,
                _pad: 0,
            },
        ],
    };
}

/// Static arena backing the MD meta-allocator. Held in an
/// `UnsafeCell` because the allocator mutates it through the
/// [`StaticMdSupply`] handle while the kernel as a whole treats it
/// as `Sync` (mutation is serialised by the BDOS lock — Phase 04
/// D6).
struct MdArena {
    blocks: [MdBlock; MD_BLOCK_COUNT],
    /// Bump cursor for fresh blocks pulled from `blocks`.
    next_fresh: usize,
    /// Head of the chain of partially-used blocks (= `mdbroot` in
    /// EmuTOS). A block leaves this chain when fully allocated and
    /// re-joins when an MD inside it is freed.
    mdbroot: *mut MdBlock,
}

// SAFETY: `MdArena` is accessed exclusively under the BDOS giant
// lock; see Phase 04 D6. The raw pointer in `mdbroot` references
// only `blocks` cells, which live for `'static`.
unsafe impl Sync for MdArenaCell {}

struct MdArenaCell(UnsafeCell<MdArena>);

impl MdArenaCell {
    const fn new() -> Self {
        Self(UnsafeCell::new(MdArena {
            blocks: [const { MdBlock::EMPTY }; MD_BLOCK_COUNT],
            next_fresh: 0,
            mdbroot: ptr::null_mut(),
        }))
    }

    /// Borrow the arena mutably. The caller asserts that the BDOS
    /// lock is held.
    ///
    /// # Safety
    ///
    /// Must not be called concurrently with another `borrow_mut` on
    /// the same arena. Phase 04 D6 enforces this via the BDOS lock.
    #[allow(clippy::mut_from_ref)]
    unsafe fn borrow_mut(&self) -> &mut MdArena {
        // SAFETY: caller contract.
        unsafe { &mut *self.0.get() }
    }
}

static GLOBAL_MD_ARENA: MdArenaCell = MdArenaCell::new();

/// Allocate one `MD` from the global meta-pool.
///
/// Returns `null_mut` when the arena is exhausted (i.e. all
/// [`MD_BLOCK_COUNT`] blocks are full and no slot is free).
///
/// # Safety
///
/// Caller must hold the BDOS giant lock (see Phase 04 D6).
pub unsafe fn xmgetmd_global() -> *mut MD {
    // SAFETY: caller contract.
    let arena = unsafe { GLOBAL_MD_ARENA.borrow_mut() };
    arena.xmgetmd()
}

/// Return one `MD` to the global meta-pool.
///
/// # Safety
///
/// `md` must have been obtained from [`xmgetmd_global`] and must not
/// be referenced afterwards. Caller must hold the BDOS giant lock.
pub unsafe fn xmfremd_global(md: *mut MD) {
    // SAFETY: caller contract.
    let arena = unsafe { GLOBAL_MD_ARENA.borrow_mut() };
    // SAFETY: trait contract.
    unsafe { arena.xmfremd(md) };
}

/// Singleton `MdSupply` adapter over the global arena.
///
/// `MdSupply` methods are `unsafe`; callers obtain a fresh
/// [`StaticMdSupply`] each time they enter the BDOS layer. The type
/// is zero-sized so threading one through a stack of pool
/// primitives costs nothing.
pub struct StaticMdSupply;

impl StaticMdSupply {
    /// Create a handle to the global MD arena. Caller must hold the
    /// BDOS giant lock for the lifetime of the returned handle.
    ///
    /// # Safety
    ///
    /// See Phase 04 D6. The handle is `!Send` by virtue of holding
    /// the lock externally.
    pub const unsafe fn new() -> Self {
        Self
    }
}

impl MdSupply for StaticMdSupply {
    unsafe fn xmgetmd(&mut self) -> *mut MD {
        // SAFETY: forwarded from `StaticMdSupply::new` precondition.
        unsafe { xmgetmd_global() }
    }

    unsafe fn xmfremd(&mut self, md: *mut MD) {
        // SAFETY: forwarded.
        unsafe { xmfremd_global(md) };
    }
}

impl MdArena {
    fn xmgetmd(&mut self) -> *mut MD {
        // Make sure there is a partially-used block on the chain.
        // EmuTOS lazily allocates one via `MGET(MDBLOCK)` on demand;
        // we draw from `blocks[next_fresh..]`.
        if self.mdbroot.is_null() {
            if self.next_fresh >= MD_BLOCK_COUNT {
                return ptr::null_mut();
            }
            let mdb: *mut MdBlock = &mut self.blocks[self.next_fresh];
            self.next_fresh += 1;
            // SAFETY: `mdb` points into `self.blocks`; `EMPTY` already
            // initialised every field. Make explicit to be safe.
            unsafe {
                (*mdb).next = ptr::null_mut();
                for i in 0..MDS_PER_BLOCK {
                    (*mdb).entries[i].slot = -1;
                }
            }
            self.mdbroot = mdb;
        }

        let mdb = self.mdbroot;
        // SAFETY: `mdb` is a valid block pointer.
        let entries: *mut MdEntry = unsafe { (*mdb).entries.as_mut_ptr() };

        // Pick the first free slot, count the rest to decide whether
        // the block is now full.
        let mut chosen: *mut MD = ptr::null_mut();
        let mut avail: usize = 0;
        for i in 0..MDS_PER_BLOCK {
            // SAFETY: in-bounds element of a valid block.
            let entry = unsafe { entries.add(i) };
            // SAFETY: see above.
            let slot = unsafe { (*entry).slot };
            if slot < 0 {
                if chosen.is_null() {
                    // SAFETY: see above.
                    unsafe {
                        (*entry).slot = i as i16;
                        chosen = &mut (*entry).md;
                    }
                } else {
                    avail += 1;
                }
            }
        }

        if chosen.is_null() {
            // Can only happen if a block sat on `mdbroot` while
            // already full — invariant violation, but we degrade
            // gracefully by unlinking it and trying again.
            self.unlink_mdblock(mdb);
            return self.xmgetmd();
        }

        // If this allocation made the block full, drop it from the
        // chain. It will rejoin via `xmfremd` when something is freed.
        if avail == 0 {
            self.unlink_mdblock(mdb);
        }

        // Initialise the MD payload to a clean state. Allocator
        // callers overwrite this immediately, but a clean default
        // makes debugging strays easier.
        // SAFETY: `chosen` is a fresh, exclusive MD reference.
        unsafe {
            (*chosen).m_link = ptr::null_mut();
            (*chosen).m_start = 0;
            (*chosen).m_length = 0;
            (*chosen).m_own = ptr::null_mut();
        }
        chosen
    }

    /// # Safety
    ///
    /// `md` must have been returned by `xmgetmd` on this arena.
    unsafe fn xmfremd(&mut self, md: *mut MD) {
        if md.is_null() {
            return;
        }

        // Recover the entry, then the parent block. We do not use
        // EmuTOS's `*(WORD*)(md+1)` arithmetic; instead we walk the
        // arena and locate the block whose entries enclose `md`.
        // This is O(MD_BLOCK_COUNT) but well-bounded (256 / 3 ≈ 86
        // iterations max) and avoids relying on layout invariants.
        // The layout invariant remains documented for ABI parity.
        let mut found: Option<(*mut MdBlock, usize)> = None;
        for blk_idx in 0..self.next_fresh {
            let blk: *mut MdBlock = &mut self.blocks[blk_idx];
            // SAFETY: in-bounds block.
            let entries: *mut MdEntry = unsafe { (*blk).entries.as_mut_ptr() };
            for i in 0..MDS_PER_BLOCK {
                // SAFETY: in-bounds entry.
                let md_in_entry: *mut MD = unsafe { &mut (*entries.add(i)).md };
                if core::ptr::eq(md_in_entry, md) {
                    found = Some((blk, i));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }

        let (mdb, idx) = match found {
            Some(x) => x,
            None => return, // wild MD; EmuTOS just logs and returns
        };

        // SAFETY: `mdb`/`idx` are valid by construction.
        unsafe {
            (*mdb).entries[idx].slot = -1;
        }

        // Count remaining free slots.
        let mut avail = 0usize;
        for i in 0..MDS_PER_BLOCK {
            // SAFETY: in-bounds entry.
            if unsafe { (*mdb).entries[i].slot } < 0 {
                avail += 1;
            }
        }

        match avail {
            // 1 free → block was full and is now partially used; add
            // back to the chain.
            1 => {
                // SAFETY: block not currently on chain (was full).
                unsafe {
                    (*mdb).next = self.mdbroot;
                }
                self.mdbroot = mdb;
            }
            // 3 free → block is empty. Unlink and forget. We do not
            // return it to a free-block pool because the arena uses a
            // bump cursor; the empty block stays parked in `blocks`
            // and will be re-picked the next time `xmgetmd` needs a
            // fresh block. To keep that simple, we *also* leave it
            // where it is on `mdbroot` only if it was on it.
            //
            // To match EmuTOS semantics (block returns to free chain),
            // we do unlink it from `mdbroot`; subsequent allocations
            // will allocate a *new* fresh block from the bump cursor
            // until that runs out, then re-encounter empty blocks.
            // The simplest safe behaviour: leave it on `mdbroot` so
            // `xmgetmd` reuses the slot immediately.
            3 => {
                // No-op: the block already has free slots and is
                // therefore on the chain (or about to be in the
                // 1→2→3 transition). Falling through.
                let _ = mdb;
            }
            // 2 free → block was already on chain, do nothing.
            _ => {}
        }
    }

    /// Unlink `mdb` from the `mdbroot` chain. Returns true if it was
    /// found.
    fn unlink_mdblock(&mut self, mdb: *mut MdBlock) -> bool {
        if mdb.is_null() {
            return false;
        }
        // SAFETY: `mdb` is a valid block pointer.
        let next = unsafe { (*mdb).next };
        // SAFETY: see above.
        unsafe {
            (*mdb).next = ptr::null_mut();
        }

        if core::ptr::eq(mdb, self.mdbroot) {
            self.mdbroot = next;
            return true;
        }

        let mut prev = self.mdbroot;
        while !prev.is_null() {
            // SAFETY: list invariants.
            let prev_next = unsafe { (*prev).next };
            if core::ptr::eq(prev_next, mdb) {
                // SAFETY: see above.
                unsafe {
                    (*prev).next = next;
                }
                return true;
            }
            prev = prev_next;
        }
        false
    }
}
