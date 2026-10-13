//! Kernel pages for the growing MD supply and the tables sized by it
//! (MEM05; design `docs/analysis/mem05-design-2026-10-13.md` §2.1, §2.5;
//! user decisions E1/E2 and the follow-up decision B, `TASK/MEM05.md`).
//!
//! EmuTOS draws its MDs from a fixed OS pool (`bdos/osmem.c:36-39`). NewTOS
//! grows its MD supply page by page on demand ([`super::mdpool`]), and the
//! tables that hold one entry per allocated block grow with it: the
//! protection records and the per-sync block snapshot
//! ([`crate::bdos::process::memprot`]) and the `F_KEEP` flags
//! ([`super::region`]).
//!
//! ## Where the pages come from (E1)
//!
//! From the kernel heap ([`HeapPageSource`], `alloc::alloc`), never from the
//! BDOS pools: with `memprot=off` every pool page is mapped for EL0, and MD
//! chains there would be writable by any program. The host tests install
//! their own source ([`set_source`]) to force exhaustion.
//!
//! A "page" is a unit of [`PAGE_BYTES`] bytes with 16-byte alignment, not a
//! page-aligned frame: nothing here needs page alignment (the supply never
//! shrinks, E8), and the kernel heap serves stricter alignments only by
//! over-allocating (`kernel/src/bios/machine/heap.rs`, module docs).
//!
//! ## The budget (decision B)
//!
//! **Every** page taken here — MD pages, protection records, snapshot,
//! `F_KEEP` flags — counts against one budget of [`MD_PAGE_BUDGET`] pages
//! (2 MiB of the kernel heap). A request that would exceed it is refused
//! exactly like a heap that has no room: the caller sees "no page" and its
//! allocation fails ([`super::traps::mxalloc`] returns `0` before any state
//! changes; `Mshrink` returns `ERROR`, `Maddalt` `ENSMEM`).
//!
//! All functions run under the BDOS giant lock (single CPU, trap / boot
//! context). The heap lock is taken inside; the heap never calls back into
//! the BDOS, so the lock order is always BDOS → heap.

use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Bytes of one MEM05 page.
pub const PAGE_BYTES: usize = 4096;

/// Pages MEM05 may hold at most (decision B: MDs, protection records,
/// snapshot and `F_KEEP` flags together; 512 × 4 KiB = 2 MiB of the 8-MiB
/// kernel heap). [`set_budget`] can only lower it.
pub const MD_PAGE_BUDGET: usize = 512;

/// Alignment of a page from [`HeapPageSource`] (the kernel heap's own grain).
const PAGE_ALIGN: usize = 16;

/// Where MEM05 pages come from.
pub trait MdPageSource: Sync {
    /// `n` contiguous pages (`n * PAGE_BYTES` bytes, at least 16-byte
    /// aligned, zero-filled); null when there is no room.
    fn alloc_pages(&self, n: usize) -> *mut u8;

    /// Give back what [`MdPageSource::alloc_pages`] returned for `n`.
    ///
    /// # Safety
    ///
    /// `p` came from `alloc_pages(n)` on this source and is not used
    /// afterwards.
    unsafe fn free_pages(&self, p: *mut u8, n: usize);
}

/// The kernel heap as the page source (`alloc::alloc`, E1). On the host the
/// global allocator is `std`'s.
pub struct HeapPageSource;

impl HeapPageSource {
    fn layout(n: usize) -> Option<Layout> {
        let size = n.checked_mul(PAGE_BYTES)?;
        if size == 0 {
            return None;
        }
        Layout::from_size_align(size, PAGE_ALIGN).ok()
    }
}

impl MdPageSource for HeapPageSource {
    fn alloc_pages(&self, n: usize) -> *mut u8 {
        match Self::layout(n) {
            // SAFETY: non-zero size, valid alignment.
            Some(l) => unsafe { alloc::alloc::alloc_zeroed(l) },
            None => core::ptr::null_mut(),
        }
    }

    unsafe fn free_pages(&self, p: *mut u8, n: usize) {
        if let Some(l) = Self::layout(n) {
            // SAFETY: caller contract — `p` came from `alloc_pages(n)`.
            unsafe { alloc::alloc::dealloc(p, l) };
        }
    }
}

static HEAP_SOURCE: HeapPageSource = HeapPageSource;

struct SourceCell(UnsafeCell<&'static dyn MdPageSource>);
// SAFETY: written only by `set_source` under the BDOS giant lock.
unsafe impl Sync for SourceCell {}
static SOURCE: SourceCell = SourceCell(UnsafeCell::new(&HEAP_SOURCE));

/// What a page is used for (the counters of [`stats`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PageUse {
    /// MD blocks ([`super::mdpool`]).
    Md = 0,
    /// Protection records, one per allocated block (`memprot` `RECS`).
    Recs = 1,
    /// The per-sync snapshot of all allocated blocks (`memprot` `SNAP`).
    Snap = 2,
    /// `F_KEEP` flags ([`super::region`] `KEPT`).
    Kept = 3,
}

const USES: usize = 4;

static BUDGET: AtomicUsize = AtomicUsize::new(MD_PAGE_BUDGET);
static HELD: [AtomicUsize; USES] = [const { AtomicUsize::new(0) }; USES];
static PEAK: AtomicUsize = AtomicUsize::new(0);
static REFUSED_BUDGET: AtomicU64 = AtomicU64::new(0);
static REFUSED_SOURCE: AtomicU64 = AtomicU64::new(0);

/// A snapshot of the MEM05 page accounting (boot log, diagnostics, tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageStats {
    /// Pages MEM05 may hold.
    pub budget: usize,
    /// Pages held now, all uses together.
    pub held: usize,
    /// Highest `held` since boot.
    pub peak: usize,
    /// Pages held for MD blocks.
    pub md: usize,
    /// Pages held for protection records.
    pub recs: usize,
    /// Pages held for the block snapshot.
    pub snap: usize,
    /// Pages held for `F_KEEP` flags.
    pub kept: usize,
    /// Requests refused because the budget was reached.
    pub refused_budget: u64,
    /// Requests refused because the source had no room.
    pub refused_source: u64,
}

/// The page accounting.
pub fn stats() -> PageStats {
    let h = |u: PageUse| HELD[u as usize].load(Ordering::Relaxed);
    PageStats {
        budget: BUDGET.load(Ordering::Relaxed),
        held: held(),
        peak: PEAK.load(Ordering::Relaxed),
        md: h(PageUse::Md),
        recs: h(PageUse::Recs),
        snap: h(PageUse::Snap),
        kept: h(PageUse::Kept),
        refused_budget: REFUSED_BUDGET.load(Ordering::Relaxed),
        refused_source: REFUSED_SOURCE.load(Ordering::Relaxed),
    }
}

fn held() -> usize {
    HELD.iter().map(|h| h.load(Ordering::Relaxed)).sum()
}

/// Lower the budget to `pages` (clamped to [`MD_PAGE_BUDGET`]); pages held
/// already stay held. For the host tests (budget exhaustion).
pub fn set_budget(pages: usize) {
    BUDGET.store(pages.min(MD_PAGE_BUDGET), Ordering::Relaxed);
}

/// Install another page source. For the host tests (source exhaustion);
/// pages taken from the previous source must not be given back after this.
///
/// # Safety
///
/// BDOS giant lock held; no page of the previous source is given back later.
pub unsafe fn set_source(src: &'static dyn MdPageSource) {
    // SAFETY: caller contract (exclusive under the lock).
    unsafe { *SOURCE.0.get() = src };
}

/// Can `n` more pages be taken without exceeding the budget?
pub fn fits(n: usize) -> bool {
    held().checked_add(n).is_some_and(|t| t <= BUDGET.load(Ordering::Relaxed))
}

/// Take `n` contiguous zero-filled pages for `what`; null when the budget is
/// reached or the source has no room (both counted in [`stats`]).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn take(n: usize, what: PageUse) -> *mut u8 {
    if n == 0 {
        return core::ptr::null_mut();
    }
    if !fits(n) {
        REFUSED_BUDGET.fetch_add(1, Ordering::Relaxed);
        return core::ptr::null_mut();
    }
    // SAFETY: lock held; read of the installed source.
    let src = unsafe { *SOURCE.0.get() };
    let p = src.alloc_pages(n);
    if p.is_null() {
        REFUSED_SOURCE.fetch_add(1, Ordering::Relaxed);
        return p;
    }
    HELD[what as usize].fetch_add(n, Ordering::Relaxed);
    PEAK.fetch_max(held(), Ordering::Relaxed);
    p
}

/// Give back `n` pages [`take`] returned for `what`.
///
/// # Safety
///
/// BDOS giant lock held; `p` came from `take(n, what)` and is not used
/// afterwards.
pub unsafe fn give_back(p: *mut u8, n: usize, what: PageUse) {
    if p.is_null() || n == 0 {
        return;
    }
    // SAFETY: lock held.
    let src = unsafe { *SOURCE.0.get() };
    // SAFETY: caller contract.
    unsafe { src.free_pages(p, n) };
    HELD[what as usize].fetch_sub(n, Ordering::Relaxed);
}

// ─── A table that grows page by page ─────────────────────────────────

/// Directory size of a [`PagedTable`]: one page per entry, as many as the
/// whole budget.
const DIR: usize = MD_PAGE_BUDGET;

/// A table of `T` stored densely in pages taken from the budget: entries
/// `0..len` are live, removal swaps the last entry in, so every scan costs
/// the live entries, not the capacity (design §2.4 point 2). Pages never
/// move and are never given back (the table only grows, like the MD supply,
/// E8). Used for the protection records and the `F_KEEP` flags.
pub struct PagedTable<T: Copy> {
    dir: [*mut T; DIR],
    pages: usize,
    len: usize,
    what: PageUse,
}

impl<T: Copy> PagedTable<T> {
    /// Entries per page.
    pub const PER_PAGE: usize = PAGE_BYTES / core::mem::size_of::<T>();

    /// An empty table whose pages are booked as `what`.
    pub const fn new(what: PageUse) -> Self {
        Self { dir: [core::ptr::null_mut(); DIR], pages: 0, len: 0, what }
    }

    /// Live entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// No live entry?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Entries the pages held can take.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.pages * Self::PER_PAGE
    }

    /// Pages held.
    #[inline]
    pub fn pages(&self) -> usize {
        self.pages
    }

    fn slot(&self, i: usize) -> *mut T {
        debug_assert!(i < self.capacity());
        // SAFETY: `i / PER_PAGE < pages`, each page holds `PER_PAGE` entries.
        unsafe { self.dir[i / Self::PER_PAGE].add(i % Self::PER_PAGE) }
    }

    /// Entry `i` (`i < len`).
    #[inline]
    pub fn get(&mut self, i: usize) -> &mut T {
        assert!(i < self.len);
        // SAFETY: a live, initialised entry of a held page.
        unsafe { &mut *self.slot(i) }
    }

    /// Make room for `extra` more entries beyond the live ones, taking pages
    /// from the budget. `false` (nothing changed but the pages already
    /// taken) when the budget or the source refuses.
    ///
    /// # Safety
    ///
    /// BDOS giant lock held.
    pub unsafe fn reserve(&mut self, extra: usize) -> bool {
        let Some(need) = self.len.checked_add(extra) else {
            return false;
        };
        while self.capacity() < need {
            if self.pages >= DIR {
                return false;
            }
            // SAFETY: lock held.
            let p = unsafe { take(1, self.what) };
            if p.is_null() {
                return false;
            }
            self.dir[self.pages] = p.cast();
            self.pages += 1;
        }
        true
    }

    /// Append `v`; grows by a page if full. `false` when no page could be
    /// had.
    ///
    /// # Safety
    ///
    /// BDOS giant lock held.
    pub unsafe fn push(&mut self, v: T) -> bool {
        // SAFETY: lock held.
        if !unsafe { self.reserve(1) } {
            return false;
        }
        // SAFETY: `len < capacity`, a slot of a held page.
        unsafe { self.slot(self.len).write(v) };
        self.len += 1;
        true
    }

    /// Remove entry `i` (`i < len`): the last entry takes its place.
    pub fn swap_remove(&mut self, i: usize) {
        assert!(i < self.len);
        let last = self.len - 1;
        if i != last {
            // SAFETY: both are live entries of held pages.
            unsafe { self.slot(i).write(self.slot(last).read()) };
        }
        self.len = last;
    }

    /// The index of the first live entry `f` accepts (a scan of the live
    /// entries only).
    pub fn position(&self, mut f: impl FnMut(&T) -> bool) -> Option<usize> {
        // SAFETY: live entries of held pages.
        (0..self.len).find(|&i| f(unsafe { &*self.slot(i) }))
    }

    /// Apply `f` to every live entry.
    pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for i in 0..self.len {
            // SAFETY: a live entry of a held page.
            f(unsafe { &mut *self.slot(i) });
        }
    }
}

/// A scratch buffer of `T` in contiguous pages from the budget, for data
/// that is rebuilt on every use (the `memprot` block snapshot, which must be
/// one slice to be sorted and searched). Growing gives the old pages back
/// first — their content is not kept — then takes the larger run.
pub struct ScratchRun<T: Copy> {
    ptr: *mut T,
    pages: usize,
    what: PageUse,
}

impl<T: Copy> ScratchRun<T> {
    /// Entries per page.
    pub const PER_PAGE: usize = PAGE_BYTES / core::mem::size_of::<T>();

    /// No pages yet.
    pub const fn new(what: PageUse) -> Self {
        Self { ptr: core::ptr::null_mut(), pages: 0, what }
    }

    /// Entries the run holds.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.pages * Self::PER_PAGE
    }

    /// Pages held.
    #[inline]
    pub fn pages(&self) -> usize {
        self.pages
    }

    /// Make the run hold at least `n` entries. The content is lost when it
    /// grows. `false` when the budget or the source refuses; the run then
    /// keeps (or gets back) its old size if it can.
    ///
    /// # Safety
    ///
    /// BDOS giant lock held; no slice from [`ScratchRun::slice`] alive.
    pub unsafe fn ensure(&mut self, n: usize) -> bool {
        if n <= self.capacity() {
            return true;
        }
        let want = n.div_ceil(Self::PER_PAGE);
        let (old, old_pages) = (self.ptr.cast::<u8>(), self.pages);
        // The new run must fit next to nothing: the old one goes first.
        // SAFETY: lock held; the old run is not referenced (caller contract).
        unsafe { give_back(old, old_pages, self.what) };
        self.ptr = core::ptr::null_mut();
        self.pages = 0;
        // SAFETY: lock held.
        let p = unsafe { take(want, self.what) };
        if !p.is_null() {
            self.ptr = p.cast();
            self.pages = want;
            return true;
        }
        // Keep what there was, if the heap still has it.
        // SAFETY: lock held.
        let back = unsafe { take(old_pages, self.what) };
        if !back.is_null() {
            self.ptr = back.cast();
            self.pages = old_pages;
        }
        false
    }

    /// Store `v` as entry `i` (`i < capacity()`).
    ///
    /// # Safety
    ///
    /// BDOS giant lock held; no slice from [`ScratchRun::filled`] alive.
    pub unsafe fn write(&mut self, i: usize, v: T) {
        assert!(i < self.capacity());
        // SAFETY: within the held run.
        unsafe { self.ptr.add(i).write(v) };
    }

    /// Entries `0..n` as a slice.
    ///
    /// # Safety
    ///
    /// BDOS giant lock held; entries `0..n` were stored with
    /// [`ScratchRun::write`] since the last [`ScratchRun::ensure`]; the
    /// slice is dropped before the next `ensure`/`write`.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn filled(&mut self, n: usize) -> &'static mut [T] {
        assert!(n <= self.capacity());
        if n == 0 {
            return &mut [];
        }
        // SAFETY: caller contract — `n` initialised entries of the held run.
        unsafe { core::slice::from_raw_parts_mut(self.ptr, n) }
    }
}
