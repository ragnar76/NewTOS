//! Kernel heap (Phase 03 D5).
//!
//! # Algorithm
//!
//! First-fit linked-list allocator with coalescing on free.
//! The heap is one or more 4-KiB frames pulled in a single
//! contiguous run from the TT-RAM pool ([`mm::frames`]) at boot.
//! All allocations come out of that arena; the heap does not grow
//! at runtime.
//!
//! Each block carries an 8-byte size header; the size is the *total*
//! block size including the header and is always a multiple of
//! [`ALIGN`]. Free blocks additionally store a `next` pointer at the
//! word right after the header, forming an address-sorted singly
//! linked list anchored at [`Heap::head`].
//!
//! ```text
//! used block:   [ size | payload .................... ]
//!                  ^header                              ^ size bytes total
//! free block:   [ size | next | unused .............. ]
//!                          ^link
//! ```
//!
//! # Constraints
//!
//! - `core::alloc::GlobalAlloc` ZST policy: an allocation with
//!   `layout.size() == 0` returns `layout.align()` cast to `*mut u8`
//!   (a non-null, well-aligned dangling pointer). It must never be
//!   freed back through this allocator (`dealloc` is a no-op for
//!   pointers that are not inside the arena).
//! - All allocations are 16-byte aligned. Layouts that request a
//!   stricter alignment fall back to over-allocate-and-search.
//! - Re-entrancy is forbidden inside the lock; the allocator does not
//!   call any function that could allocate.
//!
//! # Phase 06 / per-process heaps
//!
//! `Malloc`/`Mfree`/`Mxalloc` (Phase 03 D6) currently delegate to
//! this single kernel heap. Per-process heaps (one per BASEPAGE)
//! land with Phase 06; the migration is recorded in `docs/STATUS.md`.

#![allow(dead_code)] // surface used by GEMDOS memory syscalls (D6).

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};

use super::frames::{self, FRAME_SIZE, Pool};
use crate::sync::SpinMutex;

/// Number of 4-KiB frames carved out of TT-RAM for the kernel heap.
/// 512 frames = 2 MiB. Sized for the kernel's working set through
/// Phase 06; revisit when filesystem caches (Phase 05) land.
pub const HEAP_FRAMES: usize = 512;
/// Total heap arena size in bytes.
pub const HEAP_BYTES: u64 = (HEAP_FRAMES as u64) * FRAME_SIZE;

/// Allocation alignment. 16 covers every primitive type we emit
/// (`u128`, AArch64 NEON values). Stricter requests are handled by
/// over-allocation.
const ALIGN: usize = 16;

/// Minimum block size: header + a usable payload that can also serve
/// as a free-list `next` pointer.
const MIN_BLOCK: usize = align_up(core::mem::size_of::<Header>() + core::mem::size_of::<*mut u8>(), ALIGN);

#[repr(C)]
struct Header {
    /// Total block size including the header, multiple of `ALIGN`.
    size: usize,
}

/// First-fit free list. Nodes are stored *inside* the free blocks at
/// `(block_ptr + size_of::<Header>())`.
struct Heap {
    arena_start: usize,
    arena_end: usize,
    /// Singly linked, address-sorted list of free blocks (pointers
    /// to the [`Header`] of each free block).
    head: *mut Header,
    /// Used bytes (counts headers as well as payload).
    used: usize,
}

unsafe impl Send for Heap {}

impl Heap {
    const fn new() -> Self {
        Self {
            arena_start: 0,
            arena_end: 0,
            head: ptr::null_mut(),
            used: 0,
        }
    }

    /// Install a contiguous arena `[start, start+size)` as the heap.
    /// `size` must be a multiple of `ALIGN`. Idempotent only in the
    /// sense that calling it twice will leak the previous arena.
    unsafe fn install(&mut self, start: usize, size: usize) {
        debug_assert!(size >= MIN_BLOCK);
        debug_assert!(start.is_multiple_of(ALIGN));
        debug_assert!(size.is_multiple_of(ALIGN));

        self.arena_start = start;
        self.arena_end = start + size;
        self.used = 0;

        let header = start as *mut Header;
        unsafe {
            (*header).size = size;
            // Free-list `next` pointer lives right after the header.
            *next_ptr_slot(header) = ptr::null_mut();
        }
        self.head = header;
    }

    /// First-fit allocation. Returns `None` on OOM or alignment
    /// failure. `layout.size() == 0` is handled by the caller.
    fn alloc(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let align = layout.align().max(ALIGN);
        let size = align_up(layout.size().max(1), ALIGN);

        // Walk the free list. `prev_link` is the slot that points at
        // the current candidate, so we can splice a node out.
        let mut prev_link: *mut *mut Header = &mut self.head;
        loop {
            let cur = unsafe { *prev_link };
            if cur.is_null() {
                return None;
            }

            let block_size = unsafe { (*cur).size };
            let block_addr = cur as usize;
            let payload_addr = block_addr + core::mem::size_of::<Header>();
            // Where would the user payload start, after honouring `align`?
            let aligned_payload = align_up(payload_addr, align);
            // Padding inserted before the payload becomes part of the
            // returned block (we do not split it off).
            let head_pad = aligned_payload - payload_addr;
            let total_needed = core::mem::size_of::<Header>() + head_pad + size;
            let total_needed = align_up(total_needed, ALIGN);

            if block_size >= total_needed {
                let remainder = block_size - total_needed;
                if remainder >= MIN_BLOCK {
                    // Split: shrink current block, push tail back as a
                    // new free node spliced into `cur`'s slot.
                    let tail_addr = block_addr + total_needed;
                    let tail = tail_addr as *mut Header;
                    unsafe {
                        (*tail).size = remainder;
                        *next_ptr_slot(tail) = *next_ptr_slot(cur);
                        *prev_link = tail;
                        (*cur).size = total_needed;
                    }
                } else {
                    // Whole block is consumed; splice it out.
                    unsafe {
                        *prev_link = *next_ptr_slot(cur);
                    }
                }
                self.used += unsafe { (*cur).size };
                return NonNull::new(aligned_payload as *mut u8);
            }

            prev_link = unsafe { next_ptr_slot(cur) };
        }
    }

    /// Free a pointer previously returned by [`alloc`]. The pointer
    /// must be inside this arena; out-of-arena pointers are ignored
    /// (used to silently drop the ZST sentinel).
    unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: Layout) {
        let addr = ptr as usize;
        if addr < self.arena_start || addr >= self.arena_end {
            return;
        }
        // The block header sits at `block_addr`; we recover it by
        // walking back from the payload. Because we do not store
        // per-allocation alignment metadata, we scan the free list to
        // find the predecessor and infer the block boundaries from
        // `addr` and the existing free-list neighbours.
        //
        // This is `O(n)` in the number of free blocks. Acceptable for
        // a kernel heap with O(100) live blocks; replaced when we
        // need slabs.
        //
        // Recovery: the header is the largest aligned address `<= addr -
        // size_of::<Header>()` that is still `>= arena_start` and lies
        // outside any current free block.
        let max_header_addr = addr - core::mem::size_of::<Header>();
        // Find the block by linear walk over free + used regions:
        // the simplest correct recovery is to walk the arena from the
        // start, since blocks are contiguous and tagged with `size`.
        // That is also `O(n)` and avoids the alignment-recovery
        // ambiguity.
        let mut cursor = self.arena_start;
        let mut block_addr: usize = 0;
        while cursor < self.arena_end {
            let h = cursor as *mut Header;
            let s = unsafe { (*h).size };
            debug_assert!(s >= MIN_BLOCK && s % ALIGN == 0);
            if cursor + s > max_header_addr {
                block_addr = cursor;
                break;
            }
            cursor += s;
        }
        debug_assert!(block_addr != 0, "heap dealloc: pointer not in any block");

        let block = block_addr as *mut Header;
        let block_size = unsafe { (*block).size };
        debug_assert!(self.used >= block_size);
        self.used -= block_size;

        // Insert into address-sorted free list, then coalesce with
        // neighbours.
        unsafe {
            let mut prev_link: *mut *mut Header = &mut self.head;
            loop {
                let cur = *prev_link;
                if cur.is_null() || (cur as usize) > block_addr {
                    *next_ptr_slot(block) = cur;
                    *prev_link = block;
                    break;
                }
                prev_link = next_ptr_slot(cur);
            }

            // Coalesce with the next free block if adjacent.
            let next = *next_ptr_slot(block);
            if !next.is_null() && block_addr + block_size == next as usize {
                let next_size = (*next).size;
                (*block).size = block_size + next_size;
                *next_ptr_slot(block) = *next_ptr_slot(next);
            }

            // Coalesce with the previous free block if adjacent.
            // Find the predecessor by scanning from `head`.
            let mut prev: *mut Header = ptr::null_mut();
            let mut walk = self.head;
            while !walk.is_null() && walk != block {
                prev = walk;
                walk = *next_ptr_slot(walk);
            }
            if !prev.is_null() {
                let prev_addr = prev as usize;
                let prev_size = (*prev).size;
                if prev_addr + prev_size == block_addr {
                    (*prev).size = prev_size + (*block).size;
                    *next_ptr_slot(prev) = *next_ptr_slot(block);
                }
            }
        }
    }

    fn stats(&self) -> HeapStats {
        let total = self.arena_end - self.arena_start;
        let mut largest_free = 0usize;
        let mut free_blocks = 0usize;
        let mut walk = self.head;
        while !walk.is_null() {
            let s = unsafe { (*walk).size };
            if s > largest_free {
                largest_free = s;
            }
            free_blocks += 1;
            walk = unsafe { *next_ptr_slot(walk) };
        }
        HeapStats {
            total,
            used: self.used,
            largest_free,
            free_blocks,
        }
    }
}

/// Snapshot of heap accounting. Values are in bytes; `used` and
/// `largest_free` include the per-block header overhead.
#[derive(Clone, Copy, Debug)]
pub struct HeapStats {
    /// Arena size.
    pub total: usize,
    /// Bytes currently allocated.
    pub used: usize,
    /// Largest single free block (header included).
    pub largest_free: usize,
    /// Number of free blocks (fragmentation indicator).
    pub free_blocks: usize,
}

impl HeapStats {
    /// Free bytes across all free blocks.
    pub const fn free(&self) -> usize {
        self.total - self.used
    }
}

// ─────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────

const fn align_up(v: usize, a: usize) -> usize {
    (v + a - 1) & !(a - 1)
}

/// Pointer to the slot that holds a free block's `next` pointer
/// (immediately after the header).
unsafe fn next_ptr_slot(h: *mut Header) -> *mut *mut Header {
    unsafe { (h as *mut u8).add(core::mem::size_of::<Header>()) as *mut *mut Header }
}

// ─────────────────────────────────────────────────────────────────────
// Global state
// ─────────────────────────────────────────────────────────────────────

static HEAP: SpinMutex<Heap> = SpinMutex::new(Heap::new());

/// Initialise the kernel heap. Must run after [`frames::init`] and
/// before any allocation through the global allocator.
pub fn init() {
    let frame = frames::alloc_frames_contig(Pool::Ttram, HEAP_FRAMES)
        .expect("heap: failed to reserve initial arena from TT-RAM");
    let start = frame.pa() as usize;
    let size = HEAP_BYTES as usize;
    // SAFETY: we own this region exclusively (just allocated above);
    // it is identity-mapped writable Normal memory by the MMU
    // (Phase 03 D2/D3, DRAM mapping).
    unsafe { HEAP.lock().install(start, size) };
}

/// Snapshot of kernel-heap accounting.
pub fn stats() -> HeapStats {
    HEAP.lock().stats()
}

// ─────────────────────────────────────────────────────────────────────
// GlobalAlloc adapter
// ─────────────────────────────────────────────────────────────────────

/// Marker type wired up via `#[global_allocator]` in `lib.rs`.
pub struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            // Rust's GlobalAlloc contract allows any non-null,
            // well-aligned pointer for ZST allocations and forbids
            // dereferencing it. We deliberately return a sentinel
            // outside the arena so `dealloc` can recognise it.
            return layout.align() as *mut u8;
        }
        match HEAP.lock().alloc(layout) {
            Some(p) => p.as_ptr(),
            None => ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        unsafe { HEAP.lock().dealloc(ptr, layout) };
    }
}
