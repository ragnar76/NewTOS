//! MD meta-allocator (`xmgetmd` / `xmfremd`).
//!
//! Mirrors EmuTOS `bdos/osmem.c`: `MD`s are not allocated one at a
//! time — they live in `MDBlock`s holding three MDs plus a chain
//! link. See
//! [`docs/api/structures/md.md`](../../../../../docs/api/structures/md.md)
//! § "MD meta-allocator".
//!
//! ## Growth (MEM05)
//!
//! EmuTOS carves its `MDBLOCK`s out of a fixed OS pool (`bdos/osmem.c:36-39`,
//! at most 354 MDs). NewTOS has no fixed supply (sanctioned deviation,
//! `docs/COMPATIBILITY.md` "No fixed memory-descriptor limit"; design
//! `docs/analysis/mem05-design-2026-10-13.md` §2.2): the blocks live in
//! pages of [`BLOCKS_PER_PAGE`] blocks. The first page is static (boot and
//! the host tests need no page source); when every block is in use the
//! supply takes one more page from the kernel heap through
//! [`super::pages`], against the MEM05 page budget. A refused page is the
//! old exhaustion path: `xmgetmd` returns null. Pages are never moved or
//! given back, so an MD pointer stays valid as long as the MD is in use.
//!
//! The pool primitives in [`super::pool`] are written against the
//! [`MdSupply`] trait, so they can be tested against any source of fresh
//! MDs.

use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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

/// One slot in an `MdBlock`. The `slot` word *must* immediately
/// follow `md` in memory: EmuTOS's `xmfremd` reconstructs the index
/// by reading `*(WORD *)(md + 1)`, and so does ours (MEM05: O(1)
/// free) — `md` sits at offset 0 of this `repr(C)` entry.
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

/// `MdBlock`s per page: one MEM05 page minus the page header.
pub const BLOCKS_PER_PAGE: usize =
    (super::pages::PAGE_BYTES - core::mem::size_of::<*mut u8>()) / core::mem::size_of::<MdBlock>();

/// MDs per page.
pub const MDS_PER_PAGE: usize = BLOCKS_PER_PAGE * MDS_PER_BLOCK;

/// One page of MD blocks; `next` chains the pages in the order taken.
#[repr(C)]
struct MdPage {
    next: *mut MdPage,
    blocks: [MdBlock; BLOCKS_PER_PAGE],
}

// The layout the page arithmetic relies on (design §10: checked, not
// assumed): 32-byte entries, 104-byte blocks, 39 blocks in one page.
const _: () = assert!(core::mem::size_of::<MdEntry>() == 32);
const _: () = assert!(core::mem::size_of::<MdBlock>() == 104);
const _: () = assert!(BLOCKS_PER_PAGE == 39);
const _: () = assert!(core::mem::size_of::<MdPage>() <= super::pages::PAGE_BYTES);
const _: () = assert!(core::mem::align_of::<MdPage>() <= 16);

/// The MD supply. Held in an `UnsafeCell` because the allocator mutates
/// it through the [`StaticMdSupply`] handle while the kernel as a whole
/// treats it as `Sync` (mutation is serialised by the BDOS lock — Phase 04
/// D6).
struct MdArena {
    /// The static first page.
    first: MdPage,
    /// The page fresh blocks are drawn from; null = `first`.
    cur: *mut MdPage,
    /// Bump cursor for fresh blocks in `cur`.
    next_fresh: usize,
    /// Pages taken from the page source (besides `first`).
    grown: usize,
    /// Head of the chain of partially-used blocks (= `mdbroot` in
    /// EmuTOS). A block leaves this chain when fully allocated and
    /// re-joins when an MD inside it is freed.
    mdbroot: *mut MdBlock,
}

// SAFETY: `MdArena` is accessed exclusively under the BDOS giant
// lock; see Phase 04 D6. The raw pointers reference only blocks of
// the static first page and of pages that are never given back.
unsafe impl Sync for MdArenaCell {}

struct MdArenaCell(UnsafeCell<MdArena>);

impl MdArenaCell {
    const fn new() -> Self {
        Self(UnsafeCell::new(MdArena {
            first: MdPage {
                next: ptr::null_mut(),
                blocks: [const { MdBlock::EMPTY }; BLOCKS_PER_PAGE],
            },
            cur: ptr::null_mut(),
            next_fresh: 0,
            grown: 0,
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

/// MDs handed out and not yet returned.
static IN_USE: AtomicUsize = AtomicUsize::new(0);
/// Block entries `xmfremd` looked at, over all calls since boot — the
/// instrumentation of its O(1) cost (MEM05 design §2.3, test 2).
static FREE_STEPS: AtomicU64 = AtomicU64::new(0);

/// A snapshot of the MD supply (diagnostics, boot log, tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MdPoolStats {
    /// MDs the supply can hold without asking for more memory.
    pub capacity: usize,
    /// Pages taken from the page source (the static first page not
    /// counted).
    pub pages: usize,
    /// MDs handed out by `xmgetmd` and not returned.
    pub in_use: usize,
    /// Entries `xmfremd` inspected since boot (one per call when O(1)).
    pub free_steps: u64,
}

/// The MD supply's counters.
pub fn stats() -> MdPoolStats {
    // SAFETY: a read of one counter; writes happen under the BDOS lock.
    let grown = unsafe { (*GLOBAL_MD_ARENA.0.get()).grown };
    MdPoolStats {
        capacity: (1 + grown) * MDS_PER_PAGE,
        pages: grown,
        in_use: IN_USE.load(Ordering::Relaxed),
        free_steps: FREE_STEPS.load(Ordering::Relaxed),
    }
}

/// Allocate one `MD` from the global meta-pool.
///
/// Returns `null_mut` when every block is full and no further page can be
/// had (MEM05 page budget reached, or the kernel heap has no room).
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
        // we draw the next fresh block of the current page, and take a
        // new page when that one is used up (MEM05).
        if self.mdbroot.is_null() {
            if self.next_fresh >= BLOCKS_PER_PAGE && !self.grow() {
                return ptr::null_mut();
            }
            let page = self.cur_page();
            // SAFETY: `page` is the static first page or a held page;
            // `next_fresh < BLOCKS_PER_PAGE`.
            let mdb: *mut MdBlock = unsafe { &mut (*page).blocks[self.next_fresh] };
            self.next_fresh += 1;
            // SAFETY: `mdb` is a block nobody uses yet. Make its state
            // explicit.
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
        IN_USE.fetch_add(1, Ordering::Relaxed);
        chosen
    }

    /// # Safety
    ///
    /// `md` must have been returned by `xmgetmd` on this arena.
    unsafe fn xmfremd(&mut self, md: *mut MD) {
        if md.is_null() {
            return;
        }

        // Recover the entry, then the parent block, in O(1) over the slot
        // index that sits right behind the MD — EmuTOS `xmfremd`
        // (`bdos/osmem.c:229-243`: `i = *(WORD*)(md+1)`, `i < 0 || i >=
        // MDS_PER_BLOCK` → ignore, `entry = (MDEXT*)md - i`). MEM05 design
        // §2.3. `MdEntry` is `repr(C)` with `md` at offset 0, so `md` *is*
        // the entry; a freed slot reads -1, so a double free is ignored too.
        let entry = md.cast::<MdEntry>();
        FREE_STEPS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: `md` came from `xmgetmd`, so it is the head of an entry.
        let idx = unsafe { (*entry).slot };
        if idx < 0 || idx as usize >= MDS_PER_BLOCK {
            return; // wild or already free; EmuTOS just logs and returns
        }
        let idx = idx as usize;
        // SAFETY: entry `idx` of its block: `entry - idx` is `entries[0]`,
        // and `entries` sits at a fixed offset in the `repr(C)` block.
        let mdb: *mut MdBlock = unsafe {
            entry
                .sub(idx)
                .cast::<u8>()
                .sub(core::mem::offset_of!(MdBlock, entries))
                .cast::<MdBlock>()
        };
        debug_assert!(self.owns_block(mdb), "xmfremd: MD outside the supply");

        // SAFETY: `mdb`/`idx` are valid by construction.
        unsafe {
            (*mdb).entries[idx].slot = -1;
        }
        IN_USE.fetch_sub(1, Ordering::Relaxed);

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

    /// The page fresh blocks come from.
    fn cur_page(&mut self) -> *mut MdPage {
        if self.cur.is_null() {
            &mut self.first
        } else {
            self.cur
        }
    }

    /// Take one more page of blocks from the MEM05 page budget and make it
    /// the current one. `false` when no page can be had.
    fn grow(&mut self) -> bool {
        // SAFETY: the arena is borrowed under the BDOS lock.
        let p = unsafe { super::pages::take(1, super::pages::PageUse::Md) }.cast::<MdPage>();
        if p.is_null() {
            return false;
        }
        // SAFETY: `p` is a fresh, exclusive, 16-byte-aligned page of
        // `PAGE_BYTES >= size_of::<MdPage>()` bytes.
        unsafe {
            (*p).next = ptr::null_mut();
            for b in 0..BLOCKS_PER_PAGE {
                ptr::addr_of_mut!((*p).blocks[b]).write(MdBlock::EMPTY);
            }
            (*self.cur_page()).next = p;
        }
        self.cur = p;
        self.next_fresh = 0;
        self.grown += 1;
        true
    }

    /// Debug check: is `mdb` one of the supply's blocks?
    fn owns_block(&mut self, mdb: *mut MdBlock) -> bool {
        let mut page: *mut MdPage = &mut self.first;
        while !page.is_null() {
            // SAFETY: the page chain holds the first page and held pages.
            let lo = unsafe { ptr::addr_of!((*page).blocks) } as usize;
            let hi = lo + BLOCKS_PER_PAGE * core::mem::size_of::<MdBlock>();
            let p = mdb as usize;
            if p >= lo && p < hi && (p - lo).is_multiple_of(core::mem::size_of::<MdBlock>()) {
                return true;
            }
            // SAFETY: see above.
            page = unsafe { (*page).next };
        }
        false
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
