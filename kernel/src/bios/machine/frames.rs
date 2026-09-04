//! Physical frame allocator (Phase 03 D4).
//!
//! # Design
//!
//! Two independent bitmap allocators, one per logical pool:
//! [`Pool::Stram`] (≤ 14 MiB) and [`Pool::Ttram`] (rest of DRAM).
//! Each bitmap has 1 bit per 4-KiB frame; a `1` bit means **used**.
//!
//! Both bitmaps live in BSS. The static storage is sized for the
//! worst case we will ever see on either board (1 GiB per pool — the
//! current `MAX_FRAMES_PER_POOL` cap). The runtime length of each
//! bitmap is set in [`init`] from the actual pool extents.
//!
//! # Concurrency
//!
//! Each pool sits behind a [`SpinMutex`][crate::sync::SpinMutex] so
//! [`alloc_frame`] / [`free_frame`] are safe to call from any
//! context that does not already hold the lock. The kernel is
//! single-CPU and IRQs do not currently allocate, so contention is
//! zero in practice; the lock is there to keep the API stable when
//! SMP and IRQ-side allocations land.
//!
//! # Reservations
//!
//! No explicit reservation step is required: the pool extents
//! (`map.stram`, `map.ttram` from
//! [`layout::discover`][crate::bios::machine::layout::discover]) are
//! constructed so that nothing kernel-owned falls inside them.
//!
//! * The ST-RAM pool starts at the fixed `membot`
//!   ([`layout::STRAM_OS_RESERVE`][crate::bios::machine::layout::STRAM_OS_RESERVE]),
//!   above the boot stack, the kernel image, the page tables (in BSS) and
//!   the kernel stack — a linker `ASSERT` keeps them below it.
//! * `.dma` sits in the 2 MiB the ST-RAM slot excludes anyway
//!   ([`STRAM_IO_RESERVE`][crate::bios::machine::layout::STRAM_IO_RESERVE],
//!   the Atari `$E00000` window), between the two pools.
//! * The kernel heap and (on `board-qemu-virt`) the framebuffer come out
//!   of [`MemoryMap::os_reserve`][crate::bios::machine::layout::MemoryMap],
//!   carved off the top of DRAM before the pools are formed.

#![allow(dead_code)] // consumed by heap (D5) and Mxalloc (D6).

use crate::bios::machine::layout::{MemoryMap, PhysRange};
use crate::sync::SpinMutex;

/// Physical frame size. 4 KiB matches the AArch64 4-KiB granule used
/// by [`crate::arch::aarch64::mmu`].
pub const FRAME_SIZE: u64 = 4096;

/// Hard cap for the number of frames a single pool can manage, derived
/// from [`crate::bios::machine::layout::POOL_MAX_BYTES`] — the single
/// place that defines how much memory one pool may span.
/// 1 GiB / 4 KiB = 262_144 frames → 4096 × `u64` = 32 KiB bitmap per
/// pool, two pools = 64 KiB BSS for the allocator.
///
/// [`crate::bios::machine::layout::discover`] already caps `map.ttram` at
/// the same bound (a machine that reports more RAM keeps booting, with
/// the remainder reported as unmanaged), so the `assert!` in
/// [`Bitmap::init`] is a belt-and-braces invariant check, not the
/// mechanism that handles large machines.
const MAX_FRAMES_PER_POOL: usize =
    (crate::bios::machine::layout::POOL_MAX_BYTES / FRAME_SIZE) as usize;
const BITMAP_WORDS: usize = MAX_FRAMES_PER_POOL / 64;

/// Logical pool identifier. Maps to the `Mxalloc` mode flag (Phase 06).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    /// ST-RAM pool, capped at `STRAM_POOL_MAX` (14 MiB).
    Stram,
    /// TT-RAM pool, the rest of DRAM.
    Ttram,
}

/// 4-KiB-aligned physical frame address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysFrame(pub u64);

impl PhysFrame {
    /// Construct from a 4-KiB-aligned physical address.
    pub const fn new(pa: u64) -> Self {
        debug_assert!(pa & (FRAME_SIZE - 1) == 0);
        Self(pa)
    }
    /// Underlying physical address.
    pub const fn pa(self) -> u64 {
        self.0
    }
}

/// Snapshot of one pool's accounting.
#[derive(Clone, Copy, Debug)]
pub struct PoolStats {
    /// First physical address managed by the pool.
    pub start: u64,
    /// Total frames in the pool.
    pub total: usize,
    /// Currently allocated frames.
    pub used: usize,
}

impl PoolStats {
    /// Free frames.
    pub const fn free(&self) -> usize {
        self.total - self.used
    }
    /// Total bytes.
    pub const fn total_bytes(&self) -> u64 {
        (self.total as u64) * FRAME_SIZE
    }
    /// Free bytes.
    pub const fn free_bytes(&self) -> u64 {
        (self.free() as u64) * FRAME_SIZE
    }
}

// ─────────────────────────────────────────────────────────────────────
// Bitmap
// ─────────────────────────────────────────────────────────────────────

struct Bitmap {
    /// Physical address of frame index 0.
    start_pa: u64,
    /// Number of valid bits in [`bits`] (≤ `MAX_FRAMES_PER_POOL`).
    frames: usize,
    /// Allocated frame count (cached for `O(1)` stats).
    used: usize,
    /// One bit per frame; `1` = used. Bits beyond `frames` stay 1
    /// (set in [`init`]) so allocation cannot return an out-of-pool
    /// address even if the search overshoots.
    bits: [u64; BITMAP_WORDS],
}

impl Bitmap {
    const fn new() -> Self {
        Self {
            start_pa: 0,
            frames: 0,
            used: 0,
            // All bits set so an uninitialised pool reports "full".
            bits: [u64::MAX; BITMAP_WORDS],
        }
    }

    fn init(&mut self, range: &PhysRange) {
        // Round the pool down to whole frames at the lower edge and
        // up at the upper edge — the latter just means the unusable
        // tail bits stay marked as used. The lower edge is already
        // 4-KiB-aligned because the kernel image is 4-KiB-aligned
        // (linker script).
        debug_assert!(range.start & (FRAME_SIZE - 1) == 0);
        let frames = (range.len() / FRAME_SIZE) as usize;
        assert!(
            frames <= MAX_FRAMES_PER_POOL,
            "frames: pool too large for static bitmap"
        );

        self.start_pa = range.start;
        self.frames = frames;
        self.used = 0;

        // Clear bits [0, frames), leave the tail set so we never hand
        // out an address outside the pool.
        let full_words = frames / 64;
        let tail_bits = frames % 64;
        for w in 0..full_words {
            self.bits[w] = 0;
        }
        if tail_bits != 0 {
            let mask = (1u64 << tail_bits) - 1;
            self.bits[full_words] = !mask;
        }
        for w in (full_words + (tail_bits != 0) as usize)..BITMAP_WORDS {
            self.bits[w] = u64::MAX;
        }
    }

    fn frame_addr(&self, idx: usize) -> u64 {
        self.start_pa + (idx as u64) * FRAME_SIZE
    }

    fn frame_index(&self, pa: u64) -> Option<usize> {
        if pa < self.start_pa {
            return None;
        }
        let off = pa - self.start_pa;
        if off & (FRAME_SIZE - 1) != 0 {
            return None;
        }
        let idx = (off / FRAME_SIZE) as usize;
        if idx >= self.frames {
            return None;
        }
        Some(idx)
    }

    /// Allocate one frame. Returns `None` if the pool is exhausted.
    fn alloc_one(&mut self) -> Option<PhysFrame> {
        for (w_idx, word) in self.bits.iter_mut().enumerate() {
            if *word != u64::MAX {
                let bit = (!*word).trailing_zeros() as usize;
                let idx = w_idx * 64 + bit;
                if idx >= self.frames {
                    return None;
                }
                *word |= 1u64 << bit;
                self.used += 1;
                return Some(PhysFrame::new(self.frame_addr(idx)));
            }
        }
        None
    }

    /// Allocate `n` physically contiguous frames.
    fn alloc_contig(&mut self, n: usize) -> Option<PhysFrame> {
        if n == 0 {
            return None;
        }
        // Linear scan — fine for now; replaced by buddy/slab if the
        // working sets ever justify it (ADR-trigger documented in
        // `STATUS.md`).
        let mut run_start: Option<usize> = None;
        let mut run_len = 0usize;
        for idx in 0..self.frames {
            let w = idx / 64;
            let b = idx % 64;
            let used = (self.bits[w] >> b) & 1 == 1;
            if used {
                run_start = None;
                run_len = 0;
                continue;
            }
            if run_start.is_none() {
                run_start = Some(idx);
            }
            run_len += 1;
            if run_len == n {
                let start = run_start.unwrap();
                for k in 0..n {
                    let idx = start + k;
                    self.bits[idx / 64] |= 1u64 << (idx % 64);
                }
                self.used += n;
                return Some(PhysFrame::new(self.frame_addr(start)));
            }
        }
        None
    }

    fn free_one(&mut self, frame: PhysFrame) {
        let idx = self
            .frame_index(frame.pa())
            .expect("frames: free of address outside pool");
        let w = idx / 64;
        let b = idx % 64;
        let mask = 1u64 << b;
        debug_assert!(self.bits[w] & mask != 0, "frames: double free");
        self.bits[w] &= !mask;
        self.used -= 1;
    }

    fn stats(&self) -> PoolStats {
        PoolStats {
            start: self.start_pa,
            total: self.frames,
            used: self.used,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Allocator state
// ─────────────────────────────────────────────────────────────────────

static STRAM: SpinMutex<Bitmap> = SpinMutex::new(Bitmap::new());
static TTRAM: SpinMutex<Bitmap> = SpinMutex::new(Bitmap::new());

/// Initialise both pools from the discovered memory map.
///
/// Must be called exactly once, before any [`alloc_frame`] call.
pub fn init(map: &MemoryMap) {
    STRAM.lock().init(&map.stram);
    TTRAM.lock().init(&map.ttram);
}

// ─────────────────────────────────────────────────────────────────────
// Public allocation API
// ─────────────────────────────────────────────────────────────────────

/// Allocate one physical frame from the requested pool.
pub fn alloc_frame(pool: Pool) -> Option<PhysFrame> {
    pool_mutex(pool).lock().alloc_one()
}

/// Allocate `count` physically contiguous frames from the requested pool.
pub fn alloc_frames_contig(pool: Pool, count: usize) -> Option<PhysFrame> {
    pool_mutex(pool).lock().alloc_contig(count)
}

/// Free one frame. The pool is inferred from the frame address.
pub fn free_frame(frame: PhysFrame) {
    let pool = pool_for(frame).expect("frames: free of address in no pool");
    pool_mutex(pool).lock().free_one(frame);
}

/// Free `count` frames starting at `frame`.
pub fn free_frames(frame: PhysFrame, count: usize) {
    let pool = pool_for(frame).expect("frames: free of address in no pool");
    let mut g = pool_mutex(pool).lock();
    for i in 0..count {
        g.free_one(PhysFrame::new(frame.pa() + (i as u64) * FRAME_SIZE));
    }
}

/// Snapshot of pool accounting, useful for `Malloc(-1)` and diagnostics.
pub fn stats(pool: Pool) -> PoolStats {
    pool_mutex(pool).lock().stats()
}

fn pool_mutex(pool: Pool) -> &'static SpinMutex<Bitmap> {
    match pool {
        Pool::Stram => &STRAM,
        Pool::Ttram => &TTRAM,
    }
}

fn pool_for(frame: PhysFrame) -> Option<Pool> {
    if STRAM.lock().frame_index(frame.pa()).is_some() {
        return Some(Pool::Stram);
    }
    if TTRAM.lock().frame_index(frame.pa()).is_some() {
        return Some(Pool::Ttram);
    }
    None
}

// ─────────────────────────────────────────────────────────────────────
// Tests (host-runnable bitmap unit tests).
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bitmap(start: u64, len: u64) -> Bitmap {
        let mut b = Bitmap::new();
        b.init(&PhysRange::new(start, start + len));
        b
    }

    #[test]
    fn alloc_then_free_returns_same_frame() {
        let mut b = make_bitmap(0x4000_0000, 16 * FRAME_SIZE);
        let f = b.alloc_one().unwrap();
        assert_eq!(f.pa(), 0x4000_0000);
        b.free_one(f);
        assert_eq!(b.alloc_one().unwrap().pa(), 0x4000_0000);
    }

    #[test]
    fn alloc_exhausts_pool() {
        let n = 8;
        let mut b = make_bitmap(0, (n as u64) * FRAME_SIZE);
        for _ in 0..n {
            assert!(b.alloc_one().is_some());
        }
        assert!(b.alloc_one().is_none());
    }

    #[test]
    fn contig_finds_run_after_fragmentation() {
        let n = 16;
        let mut b = make_bitmap(0, (n as u64) * FRAME_SIZE);
        let f0 = b.alloc_one().unwrap();
        let f1 = b.alloc_one().unwrap();
        let _f2 = b.alloc_one().unwrap();
        b.free_one(f0);
        b.free_one(f1);
        // Two-frame run starts at index 0.
        let run = b.alloc_contig(2).unwrap();
        assert_eq!(run.pa(), 0);
    }

    #[test]
    fn tail_bits_are_marked_used() {
        // 5 frames in a bitmap that is sized for ≥ 64.
        let mut b = make_bitmap(0, 5 * FRAME_SIZE);
        for _ in 0..5 {
            assert!(b.alloc_one().is_some());
        }
        // No further frame must come back, even though the bitmap
        // word still has 59 zero bits beyond the pool.
        assert!(b.alloc_one().is_none());
    }
}
