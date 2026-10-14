//! The program heap: a `GlobalAlloc` over GEMDOS `Malloc` **hunks**.
//!
//! Every GEMDOS block is one memory descriptor of the process, and the
//! kernel allows at most 8192 of them per process (MEM05,
//! `docs/sdk/05-abi-pitfalls.md` §A3, ADR-025). A program that asked GEMDOS
//! for every `Box`/`Vec`/`String` would hit that limit and pay a trap per
//! allocation. So, like every TOS C library, this heap takes memory from
//! GEMDOS in large hunks and carves the program's objects out of them.
//!
//! The hunk policy is mintlib's (`sdk/libc/overlay/mintlib/malloc.c`
//! `__malloc`, `sdk/libc/mintlib/unix/sbrk.c` `__sbrk`), as NewTOS ships it
//! (TASK/MEM05 part B):
//!
//! * the first hunk is 8 KiB (`MINHUNK`), and every further hunk doubles,
//!   up to 16 MiB (`MAXHUNK`, NewTOS's raised ceiling);
//! * a request that does not fit the next scheduled hunk gets a block of its
//!   own, of its own (rounded) size, without advancing the schedule;
//! * hunk sizes are rounded up to mintlib's TOS page size, 8 KiB
//!   (`DEF_PAGESIZE`, `unix/getpagesize.c`);
//! * inside a hunk: first fit over an address-ordered free list, the
//!   allocation is cut from the **tail** of the free chunk, freed chunks are
//!   merged with both neighbours;
//! * a hunk whose last object is freed goes back to GEMDOS (`Mfree`).
//!
//! One deliberate difference: mintlib returns an empty hunk at once, so a
//! loop that allocates and frees a single object pays a `Malloc`/`Mfree`
//! pair per iteration and, because every `Malloc` doubles the schedule,
//! drives the hunk size to the ceiling. This heap keeps **one** empty hunk
//! in reserve and returns any further empty one. The difference is not
//! observable through the TOS API (it only changes when the program's own
//! blocks are released).
//!
//! The internals are this crate's own (not a port of mintlib's
//! `mem_chunk` code): per-hunk free lists, 16-byte granularity, and
//! alignments above 16 by over-allocation.
//!
//! Single-threaded by construction (a NewTOS process has one thread). The
//! heap still guards itself with a flag: re-entry — possible only from a
//! signal handler the program installs itself — makes `alloc` fail (null)
//! and `dealloc` leak, instead of corrupting the lists.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::gemdos;

/// mintlib `MINHUNK`: the first hunk.
const MINHUNK: usize = 8 * 1024;
/// NewTOS mintlib `MAXHUNK` (TASK/MEM05 part B, `__newtos_malloc_maxhunk`).
const MAXHUNK: usize = 16 * 1024 * 1024;
/// mintlib's TOS page size (`DEF_PAGESIZE`): hunk sizes are multiples.
const HUNK_ROUND: usize = 8 * 1024;
/// Chunk granularity and the alignment every chunk payload has.
const GRAIN: usize = 16;
/// Header in front of every chunk (free or allocated).
const CHUNK_HDR: usize = 16;
/// The smallest chunk: a header plus one grain.
const MIN_CHUNK: usize = CHUNK_HDR + GRAIN;
/// The hunk header, rounded up to [`GRAIN`].
const HUNK_HDR: usize = (core::mem::size_of::<Hunk>() + GRAIN - 1) & !(GRAIN - 1);
/// Slack for aligning the hunk header and rounding the chunk span: GEMDOS
/// guarantees only an even block address (EmuTOS rounds to 2; NewTOS's
/// pools happen to round to 16, which this heap does not rely on). Up to
/// `GRAIN - 1` bytes are lost in front of the header and up to `GRAIN - 1`
/// at the end, so a hunk of `need + HUNK_HDR + HUNK_SLACK` bytes always has
/// a chunk span of at least `need`.
const HUNK_SLACK: usize = 2 * GRAIN;

/// One GEMDOS block. The header sits at the block's first 16-byte boundary;
/// the chunks follow it.
#[repr(C)]
struct Hunk {
    /// Next hunk, oldest first.
    next: *mut Hunk,
    /// The GEMDOS block address (for `Mfree`).
    block: *mut u8,
    /// The GEMDOS block size.
    size: usize,
    /// Bytes available to chunks (from the first chunk to the block end,
    /// rounded down to [`GRAIN`]).
    span: usize,
    /// Bytes of allocated chunks (headers included).
    used: usize,
    /// First free chunk (address order).
    free: *mut Chunk,
}

/// A chunk header. `size` is the whole chunk, header included. While the
/// chunk is free `link` is the next free chunk of the hunk; while it is
/// allocated `link` is its hunk.
#[repr(C)]
struct Chunk {
    size: usize,
    link: *mut u8,
}

/// Counters, for the program to show and for the acceptance runs.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeapStats {
    /// GEMDOS `Malloc` calls that returned a hunk.
    pub mallocs: u32,
    /// GEMDOS `Malloc` calls that failed.
    pub malloc_failures: u32,
    /// GEMDOS `Mfree` calls.
    pub mfrees: u32,
    /// Hunks currently held.
    pub hunks: u32,
    /// Most hunks held at once.
    pub peak_hunks: u32,
    /// Bytes currently held from GEMDOS.
    pub hunk_bytes: usize,
    /// Successful `alloc` calls.
    pub allocs: u64,
    /// `dealloc` calls.
    pub frees: u64,
    /// Allocations currently live.
    pub live: u64,
    /// Bytes of live chunks (headers included).
    pub live_bytes: usize,
    /// Size of the next scheduled hunk.
    pub next_hunk: usize,
}

struct State {
    hunks: *mut Hunk,
    next_hunk: usize,
    /// Empty hunks currently kept (0 or 1).
    empty: u32,
    stats: HeapStats,
}

/// The program heap. Installed as the global allocator in `lib.rs`.
pub struct Heap {
    busy: AtomicBool,
    state: UnsafeCell<State>,
}

// SAFETY: every access to `state` happens with `busy` held (see `lock`).
unsafe impl Sync for Heap {}

const fn round_up(v: usize, to: usize) -> Option<usize> {
    match v.checked_add(to - 1) {
        Some(s) => Some(s & !(to - 1)),
        None => None,
    }
}

impl Heap {
    pub const fn new() -> Self {
        Self {
            busy: AtomicBool::new(false),
            state: UnsafeCell::new(State {
                hunks: null_mut(),
                next_hunk: MINHUNK,
                empty: 0,
                stats: HeapStats {
                    mallocs: 0,
                    malloc_failures: 0,
                    mfrees: 0,
                    hunks: 0,
                    peak_hunks: 0,
                    hunk_bytes: 0,
                    allocs: 0,
                    frees: 0,
                    live: 0,
                    live_bytes: 0,
                    next_hunk: MINHUNK,
                },
            }),
        }
    }

    /// Run `f` on the heap state, or return `None` when the heap is already
    /// in use (re-entry).
    fn lock<R>(&self, f: impl FnOnce(&mut State) -> R) -> Option<R> {
        if self.busy.swap(true, Ordering::Acquire) {
            return None;
        }
        // SAFETY: `busy` was false and is now ours; nobody else touches
        // `state` until we release it.
        let r = f(unsafe { &mut *self.state.get() });
        self.busy.store(false, Ordering::Release);
        Some(r)
    }

    /// A snapshot of the counters.
    pub fn stats(&self) -> HeapStats {
        self.lock(|s| {
            let mut st = s.stats;
            st.next_hunk = s.next_hunk;
            st
        })
        .unwrap_or_default()
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// A chunk of `need` bytes (a multiple of [`GRAIN`], ≥ [`MIN_CHUNK`],
    /// header included); returns the payload address or null.
    unsafe fn take(&mut self, need: usize) -> *mut u8 {
        let mut h = self.hunks;
        while !h.is_null() {
            if let Some(p) = unsafe { self.take_from(h, need) } {
                return p;
            }
            h = unsafe { (*h).next };
        }
        let h = self.new_hunk(need);
        if h.is_null() {
            return null_mut();
        }
        unsafe { self.take_from(h, need) }.unwrap_or(null_mut())
    }

    /// First fit inside hunk `h`.
    unsafe fn take_from(&mut self, h: *mut Hunk, need: usize) -> Option<*mut u8> {
        unsafe {
            let mut prev: *mut *mut Chunk = &mut (*h).free;
            let mut c = (*h).free;
            while !c.is_null() {
                let size = (*c).size;
                if size >= need {
                    let (chunk, got) = if size - need >= MIN_CHUNK {
                        // Cut from the tail; the free chunk stays in place.
                        (*c).size = size - need;
                        ((c as *mut u8).add(size - need) as *mut Chunk, need)
                    } else {
                        *prev = (*c).link as *mut Chunk;
                        (c, size)
                    };
                    (*chunk).size = got;
                    (*chunk).link = h as *mut u8;
                    if (*h).used == 0 {
                        self.empty -= 1;
                    }
                    (*h).used += got;
                    self.stats.live_bytes += got;
                    return Some((chunk as *mut u8).add(CHUNK_HDR));
                }
                prev = &mut (*c).link as *mut *mut u8 as *mut *mut Chunk;
                c = (*c).link as *mut Chunk;
            }
            None
        }
    }

    /// Get a new hunk from GEMDOS big enough for a chunk of `need` bytes
    /// and append it to the list. Null when GEMDOS has no such block.
    fn new_hunk(&mut self, need: usize) -> *mut Hunk {
        let Some(want) = need.checked_add(HUNK_HDR + HUNK_SLACK) else { return null_mut() };
        let size = if want > self.next_hunk {
            // mintlib: `n + BORDER_EXTRA > MINHUNK` -> a block of its own size.
            match round_up(want, HUNK_ROUND) {
                Some(s) => s,
                None => return null_mut(),
            }
        } else {
            let s = self.next_hunk;
            if self.next_hunk < MAXHUNK {
                self.next_hunk *= 2;
            }
            s
        };
        let Some(block) = gemdos::malloc(size) else {
            self.stats.malloc_failures += 1;
            return null_mut();
        };
        let block = block.as_ptr();
        let base = (block as usize + GRAIN - 1) & !(GRAIN - 1);
        let first_at = base + HUNK_HDR;
        let span = (block as usize + size - first_at) & !(GRAIN - 1);
        let h = base as *mut Hunk;
        // SAFETY: GEMDOS gave us `size` bytes at `block`; header and chunks
        // lie inside it (`size >= need + HUNK_HDR + HUNK_SLACK`).
        unsafe {
            let first = first_at as *mut Chunk;
            (*first).size = span;
            (*first).link = null_mut();
            h.write(Hunk { next: null_mut(), block, size, span, used: 0, free: first });
            // Append (oldest first: first fit prefers the older hunks).
            let mut tail: *mut *mut Hunk = &mut self.hunks;
            while !(*tail).is_null() {
                tail = &mut (**tail).next;
            }
            *tail = h;
        }
        self.empty += 1;
        self.stats.mallocs += 1;
        self.stats.hunks += 1;
        self.stats.peak_hunks = self.stats.peak_hunks.max(self.stats.hunks);
        self.stats.hunk_bytes += size;
        h
    }

    /// Return the chunk whose payload is `p`.
    unsafe fn give(&mut self, p: *mut u8) {
        unsafe {
            let c = p.sub(CHUNK_HDR) as *mut Chunk;
            let size = (*c).size;
            let h = (*c).link as *mut Hunk;
            (*h).used -= size;
            self.stats.live_bytes -= size;
            if (*h).used == 0 {
                self.hunk_emptied(h);
                return;
            }
            // Insert in address order and merge with the neighbours.
            let mut prev: *mut Chunk = null_mut();
            let mut next = (*h).free;
            while !next.is_null() && (next as usize) < (c as usize) {
                prev = next;
                next = (*next).link as *mut Chunk;
            }
            let mut size = size;
            let mut link = next;
            if !next.is_null() && (c as usize) + size == next as usize {
                size += (*next).size;
                link = (*next).link as *mut Chunk;
            }
            if !prev.is_null() && (prev as usize) + (*prev).size == c as usize {
                (*prev).size += size;
                (*prev).link = link as *mut u8;
            } else {
                (*c).size = size;
                (*c).link = link as *mut u8;
                if prev.is_null() {
                    (*h).free = c;
                } else {
                    (*prev).link = c as *mut u8;
                }
            }
        }
    }

    /// Hunk `h` holds no object any more: keep it as the one reserve, or
    /// give it back to GEMDOS.
    unsafe fn hunk_emptied(&mut self, h: *mut Hunk) {
        unsafe {
            if self.empty == 0 {
                // Keep it, as one free chunk spanning the whole hunk.
                let first = (h as *mut u8).add(HUNK_HDR) as *mut Chunk;
                (*first).size = (*h).span;
                (*first).link = null_mut();
                (*h).free = first;
                self.empty = 1;
                return;
            }
            let mut pp: *mut *mut Hunk = &mut self.hunks;
            while *pp != h {
                pp = &mut (**pp).next;
            }
            *pp = (*h).next;
            let size = (*h).size;
            let rc = gemdos::mfree((*h).block);
            self.stats.mfrees += 1;
            if rc == 0 {
                self.stats.hunks -= 1;
                self.stats.hunk_bytes -= size;
            }
        }
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Payload size; an alignment above GRAIN is bought with slack in
        // front of the object and the chunk's payload address stored in
        // the word just below the aligned pointer.
        let over = if layout.align() > GRAIN { layout.align() + GRAIN } else { 0 };
        let Some(payload) = layout.size().checked_add(over) else { return null_mut() };
        let Some(need) = round_up(payload.max(1), GRAIN).and_then(|n| n.checked_add(CHUNK_HDR))
        else {
            return null_mut();
        };
        self.lock(|s| {
            // SAFETY: the lock is held; `need` is a valid chunk size.
            let p = unsafe { s.take(need) };
            if p.is_null() {
                return p;
            }
            s.stats.allocs += 1;
            s.stats.live += 1;
            if over == 0 {
                return p;
            }
            let user = ((p as usize + GRAIN + layout.align() - 1) & !(layout.align() - 1)) as *mut u8;
            // SAFETY: `user - 8 >= p + 8` lies in the chunk's payload.
            unsafe { (user.sub(8) as *mut usize).write(p as usize) };
            user
        })
        .unwrap_or(null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let p = if layout.align() > GRAIN {
            // SAFETY: `alloc` stored the chunk payload address here.
            unsafe { (ptr.sub(8) as *const usize).read() as *mut u8 }
        } else {
            ptr
        };
        let _ = self.lock(|s| {
            // SAFETY: `p` is a payload this heap handed out.
            unsafe { s.give(p) };
            s.stats.frees += 1;
            s.stats.live -= 1;
        });
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // In place when the chunk already has room (a `Vec` that shrinks, or
        // grows inside its rounding); otherwise allocate, copy, free.
        if layout.align() <= GRAIN {
            // SAFETY: `ptr` is a payload of this heap; its header precedes it.
            let have = unsafe { (*(ptr.sub(CHUNK_HDR) as *const Chunk)).size } - CHUNK_HDR;
            if new_size <= have && new_size > 0 {
                return ptr;
            }
        }
        // SAFETY: GlobalAlloc::realloc contract (same alignment, non-zero size).
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new = unsafe { self.alloc(new_layout) };
        if !new.is_null() {
            unsafe {
                ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
        }
        new
    }
}
