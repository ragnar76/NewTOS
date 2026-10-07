//! Block read cache (FS09) — FreeMiNT `block_IO` behaviour, below BIOS `Rwabs`.
//!
//! Spec: `docs/analysis/fs09-block-cache-2026-10-07.md` (FreeMiNT @ `f906416`,
//! `sys/block_IO.c`; FreeMiNT is GPL — the behaviour is reimplemented here,
//! no code is taken over). EmuTOS has no block cache; this is a FreeMiNT
//! extension beyond EmuTOS, decided by the user on 2026-10-07 (`TASK/FS09.md`).
//!
//! # Where it sits
//!
//! One cache for the (single) block device, **under** every access to it:
//! the GEMDOS direct transfers (`bdos/fs/fsio.rs`, EmuTOS `usrio`), the small
//! BDOS buffer lists (`bdos/fs/cache.rs`), BIOS `Rwabs` in logical **and**
//! physical mode, `Getbpb` and the MBR scan all reach the card through
//! [`device`]. The key is the **absolute device LBA** of a 512-byte sector, so
//! a logical sector of a partition and the physical sector it maps to are the
//! same cache line. FreeMiNT keeps its cache above `Rwabs`, per drive, keyed
//! by the drive's logical sector, and lets a program's `Rwabs` pass by
//! (`sys/bios.c:159-213`); placing it below every access is the named NewTOS
//! deviation that makes the cache coherent with every writer by construction.
//!
//! # Behaviour (spec §2)
//!
//! * Unit: the 512-byte sector — FreeMiNT's `CHUNK_SIZE` (`block_IO.c:202`).
//! * Read (`bio_l_read`, `block_IO.c:2071-2151`): hits are copied out of the
//!   cache; each contiguous run of misses is read with **one** device request
//!   straight into the caller's buffer and then inserted — but only if
//!   `run * 100 <= pct * capacity` (`bio_units_add`, `:2034`), so one long
//!   linear read does not flush the cache. No read-ahead (`bio_pre_read` is
//!   empty in FreeMiNT, `:2271-2276`). A failed run inserts nothing.
//! * Replacement: exact LRU over the sector slots (FreeMiNT approximates LRU
//!   with time stamps on 32-KiB blocks, `:1172-1258`, because its units vary in
//!   size; ours do not).
//! * Write: **write-through** (HW03, `TASK/FS09.md` decision 2) — the device
//!   is written first; only a write the device confirmed changes the cache.
//!   Up to 32 KiB (FreeMiNT `MIN_BLOCK`, `:201`) every written sector is
//!   inserted or overwritten (`bio_small_write`, `:2210-2246`); a larger write
//!   drops the range first (`bio_large_write`, `:2154-2207`). A failed write
//!   drops the range: what an aborted multi-block write left on the card is
//!   unknown, so the next read asks the card.
//! * Invalidate: [`invalidate_all`] — called by `bios::mount` (a new partition
//!   scan is NewTOS' "new medium / new mapping", FreeMiNT `bio_free_di` /
//!   `bio_set_lshift` → `bio_invalidate`, `:1857-1874`, `:1905-1925`,
//!   `:2411-2458`).
//!
//! Nothing here is observable through the API (same bytes, same codes) except
//! time — and the smaller TT-RAM the kernel's reserve leaves.
//!
//! # Memory
//!
//! The cache works on an arena the kernel hands over at boot ([`install`]): a
//! range of the OS reserve at the top of DRAM, outside ST-RAM and TT-RAM
//! (`kernel/src/bios/machine/layout.rs`). The slot bookkeeping lives at the
//! start of the arena, the 512-byte data slots behind it. Without an arena
//! ([`install`] never called, or a size of 0) [`device`] passes every request
//! straight to the host's device — that is also the state of every host test
//! that does not install one.
//!
//! Single block device, single-CPU, IRQ-off trap path — no locking (as the
//! BDOS buffers, `bdos/fs/cache.rs`).

use core::cell::UnsafeCell;
use core::mem::size_of;

use tos_types::error::EUNDEV;

use crate::bios::block::{BlockDev, BlockResult};
use crate::host;

/// Bytes per cache slot = per device sector.
pub const SECTOR: usize = 512;

/// Default cache size in KiB: 32 MiB. FreeMiNT's default 128 KB
/// (`block_IO.c:183`) is ~3 % of a 4-MB ST; the same share of the ~950 MiB
/// NewTOS manages on a Pi 4 is ~30 MiB (spec §2.4).
pub const DEFAULT_KIB: u32 = 32 * 1024;

/// Default share of the cache one linear read may fill, in percent —
/// FreeMiNT `DEFAULT_PERC` (`block_IO.c:184`).
pub const DEFAULT_PCT: u32 = 5;

/// Largest cache size the boot argument may ask for, in KiB (256 MiB).
pub const MAX_KIB: u32 = 256 * 1024;

/// Writes up to this size insert their sectors; larger ones drop the range —
/// FreeMiNT `MIN_BLOCK` (`block_IO.c:201`, `bio_l_write` `:2256`).
pub const SMALL_WRITE_BYTES: usize = 32 * 1024;

/// "No slot" in the index links.
const NIL: u32 = u32::MAX;
/// LBA of a slot that holds nothing.
const FREE_LBA: u64 = u64::MAX;

/// Bookkeeping of one slot: its LBA, the hash-chain link and the LRU links
/// (head = most recently used). A free slot is on the free list through
/// `next` and has `lba == FREE_LBA`.
#[repr(C)]
#[derive(Clone, Copy)]
struct Meta {
    lba: u64,
    hnext: u32,
    prev: u32,
    next: u32,
    _pad: u32,
}

/// Counters and geometry of a cache.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Number of 512-byte slots.
    pub slots: u32,
    /// Slots currently holding a sector.
    pub used: u32,
    /// Share of the cache one linear read may fill, in percent.
    pub pct: u32,
    /// Sectors answered from the cache.
    pub hit_sectors: u64,
    /// Sectors read from the device.
    pub miss_sectors: u64,
    /// Read requests sent to the device.
    pub dev_reads: u64,
    /// Write requests sent to the device.
    pub dev_writes: u64,
    /// Sectors put into a free or evicted slot.
    pub inserted: u64,
    /// Sectors pushed out by the LRU to make room.
    pub evicted: u64,
    /// Sectors removed by a large write, a failed write or an invalidation.
    pub dropped: u64,
    /// Miss runs not inserted because of the linear-read share.
    pub runs_not_inserted: u64,
    /// Whole-cache invalidations.
    pub invalidations: u64,
}

/// A sector cache over a caller-supplied arena. See the module docs.
pub struct BlockCache {
    meta: *mut Meta,
    heads: *mut u32,
    data: *mut u8,
    n: u32,
    mask: u32,
    lru_head: u32,
    lru_tail: u32,
    free: u32,
    stats: Stats,
}

impl BlockCache {
    /// Lay a cache out over `bytes` bytes at `arena`, filling at most `pct`
    /// percent with one linear read. `None` if the arena cannot hold a single
    /// slot, is not 8-byte aligned, or `pct > 100` (FreeMiNT
    /// `bio_set_percentage` refuses that too, `block_IO.c:1500-1501`).
    ///
    /// # Safety
    ///
    /// `arena..arena + bytes` must be writable memory owned by the cache for
    /// as long as the returned value (or anything it was moved into) is used.
    pub unsafe fn new(arena: *mut u8, bytes: usize, pct: u32) -> Option<BlockCache> {
        if arena.is_null() || !(arena as usize).is_multiple_of(8) || pct > 100 {
            return None;
        }
        // Per slot: the data, its bookkeeping, and at most two hash heads
        // (the head count is the next power of two, < 2n). One sector is
        // kept back for aligning the data area.
        let per = SECTOR + size_of::<Meta>() + 2 * size_of::<u32>();
        let mut n = (bytes.saturating_sub(SECTOR) / per).min(NIL as usize - 1);
        let (nb, data_off) = loop {
            if n == 0 {
                return None;
            }
            let nb = n.next_power_of_two();
            let book = n * size_of::<Meta>() + nb * size_of::<u32>();
            let data_off = book.div_ceil(SECTOR) * SECTOR;
            if data_off + n * SECTOR <= bytes {
                break (nb, data_off);
            }
            n -= 1;
        };
        let mut c = BlockCache {
            meta: arena.cast::<Meta>(),
            // SAFETY: inside the arena (checked above); u32 alignment holds
            // because `Meta` is 24 bytes and the arena is 8-aligned.
            heads: unsafe { arena.add(n * size_of::<Meta>()) }.cast::<u32>(),
            // SAFETY: `data_off + n * SECTOR <= bytes`.
            data: unsafe { arena.add(data_off) },
            n: n as u32,
            mask: (nb - 1) as u32,
            lru_head: NIL,
            lru_tail: NIL,
            free: NIL,
            stats: Stats { slots: n as u32, pct, ..Stats::default() },
        };
        c.reset();
        Some(c)
    }

    /// Empty every slot (free list in slot order, all hash chains empty).
    fn reset(&mut self) {
        for b in 0..=self.mask as usize {
            // SAFETY: `b <= mask < nb` heads were laid out in `new`.
            unsafe { *self.heads.add(b) = NIL };
        }
        for i in 0..self.n {
            let next = if i + 1 < self.n { i + 1 } else { NIL };
            *self.m(i) = Meta { lba: FREE_LBA, hnext: NIL, prev: NIL, next, _pad: 0 };
        }
        self.free = if self.n > 0 { 0 } else { NIL };
        self.lru_head = NIL;
        self.lru_tail = NIL;
        self.stats.used = 0;
    }

    /// Number of slots.
    pub fn slots(&self) -> u32 {
        self.n
    }

    /// Bytes of sector data the cache can hold.
    pub fn capacity_bytes(&self) -> usize {
        self.n as usize * SECTOR
    }

    /// A copy of the counters.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Is `lba` in the cache? (No LRU change.)
    pub fn contains(&self, lba: u64) -> bool {
        self.find(lba) != NIL
    }

    /// LBAs held, most recently used first.
    pub fn lru_order(&self, out: &mut [u64]) -> usize {
        let mut k = 0;
        let mut i = self.lru_head;
        while i != NIL && k < out.len() {
            out[k] = self.mc(i).lba;
            k += 1;
            i = self.mc(i).next;
        }
        k
    }

    fn m(&mut self, i: u32) -> &mut Meta {
        debug_assert!(i < self.n);
        // SAFETY: `i < n`; the metas were laid out in `new`.
        unsafe { &mut *self.meta.add(i as usize) }
    }

    fn mc(&self, i: u32) -> &Meta {
        debug_assert!(i < self.n);
        // SAFETY: as `m`.
        unsafe { &*self.meta.add(i as usize) }
    }

    fn slot(&self, i: u32) -> *mut u8 {
        // SAFETY: `i < n`; `n * SECTOR` bytes of data were laid out in `new`.
        unsafe { self.data.add(i as usize * SECTOR) }
    }

    fn bucket(&self, lba: u64) -> usize {
        ((lba.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as u32 & self.mask) as usize
    }

    fn head(&self, b: usize) -> u32 {
        // SAFETY: `b <= mask`.
        unsafe { *self.heads.add(b) }
    }

    fn set_head(&mut self, b: usize, v: u32) {
        // SAFETY: `b <= mask`.
        unsafe { *self.heads.add(b) = v };
    }

    fn find(&self, lba: u64) -> u32 {
        let mut i = self.head(self.bucket(lba));
        while i != NIL {
            if self.mc(i).lba == lba {
                return i;
            }
            i = self.mc(i).hnext;
        }
        NIL
    }

    fn hash_unlink(&mut self, i: u32) {
        let b = self.bucket(self.mc(i).lba);
        let mut p = self.head(b);
        if p == i {
            let nx = self.mc(i).hnext;
            self.set_head(b, nx);
            return;
        }
        while p != NIL {
            let nx = self.mc(p).hnext;
            if nx == i {
                let after = self.mc(i).hnext;
                self.m(p).hnext = after;
                return;
            }
            p = nx;
        }
        debug_assert!(false, "bcache: slot not on its hash chain");
    }

    fn lru_unlink(&mut self, i: u32) {
        let Meta { prev, next, .. } = *self.mc(i);
        if prev != NIL {
            self.m(prev).next = next;
        } else {
            self.lru_head = next;
        }
        if next != NIL {
            self.m(next).prev = prev;
        } else {
            self.lru_tail = prev;
        }
    }

    fn lru_push_front(&mut self, i: u32) {
        let old = self.lru_head;
        {
            let m = self.m(i);
            m.prev = NIL;
            m.next = old;
        }
        if old != NIL {
            self.m(old).prev = i;
        } else {
            self.lru_tail = i;
        }
        self.lru_head = i;
    }

    fn touch(&mut self, i: u32) {
        if self.lru_head != i {
            self.lru_unlink(i);
            self.lru_push_front(i);
        }
    }

    /// A slot for a new sector: a free one, else the least recently used.
    fn take_slot(&mut self) -> u32 {
        if self.free != NIL {
            let i = self.free;
            self.free = self.mc(i).next;
            self.stats.used += 1;
            return i;
        }
        let i = self.lru_tail;
        debug_assert!(i != NIL, "bcache: no slot at all");
        self.hash_unlink(i);
        self.lru_unlink(i);
        self.stats.evicted += 1;
        i
    }

    /// Put `src` (one sector) into the cache as `lba`, most recently used.
    fn put(&mut self, lba: u64, src: &[u8]) {
        debug_assert_eq!(src.len(), SECTOR);
        let mut i = self.find(lba);
        if i == NIL {
            i = self.take_slot();
            let b = self.bucket(lba);
            let h = self.head(b);
            {
                let m = self.m(i);
                m.lba = lba;
                m.hnext = h;
            }
            self.set_head(b, i);
            self.lru_push_front(i);
            self.stats.inserted += 1;
        } else {
            self.touch(i);
        }
        // SAFETY: the slot is SECTOR bytes inside the arena; `src` is a
        // caller buffer, never the arena itself.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), self.slot(i), SECTOR) };
    }

    /// Take slot `i` out of the cache.
    fn remove(&mut self, i: u32) {
        self.hash_unlink(i);
        self.lru_unlink(i);
        let f = self.free;
        {
            let m = self.m(i);
            m.lba = FREE_LBA;
            m.hnext = NIL;
            m.prev = NIL;
            m.next = f;
        }
        self.free = i;
        self.stats.used -= 1;
        self.stats.dropped += 1;
    }

    /// Drop every cached sector in `lba..lba + count`.
    pub fn invalidate_range(&mut self, lba: u64, count: u64) {
        if count == 0 || self.stats.used == 0 {
            return;
        }
        let end = lba.saturating_add(count);
        if count > u64::from(self.stats.used) {
            // Fewer cached sectors than the range is long: walk the cache.
            let mut i = self.lru_head;
            while i != NIL {
                let next = self.mc(i).next;
                let l = self.mc(i).lba;
                if l >= lba && l < end {
                    self.remove(i);
                }
                i = next;
            }
        } else {
            for l in lba..end {
                let i = self.find(l);
                if i != NIL {
                    self.remove(i);
                }
            }
        }
    }

    /// Drop everything (FreeMiNT `bio_invalidate`).
    pub fn invalidate_all(&mut self) {
        self.stats.dropped += u64::from(self.stats.used);
        self.stats.invalidations += 1;
        self.reset();
    }

    /// Read `buf.len() / 512` sectors from `lba` (spec §2.6).
    pub fn read(&mut self, dev: &dyn BlockDev, lba: u64, buf: &mut [u8]) -> BlockResult {
        let n = buf.len() / SECTOR;
        if n == 0 || !buf.len().is_multiple_of(SECTOR) {
            // Not a whole-sector request: nothing to cache, the device decides.
            self.stats.dev_reads += 1;
            return dev.read_blocks(lba, buf);
        }
        let mut k = 0;
        while k < n {
            let i = self.find(lba + k as u64);
            if i != NIL {
                // SAFETY: slot and caller sector are SECTOR bytes, distinct memory.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        self.slot(i),
                        buf[k * SECTOR..].as_mut_ptr(),
                        SECTOR,
                    )
                };
                self.touch(i);
                self.stats.hit_sectors += 1;
                k += 1;
                continue;
            }
            // A run of misses: one device request, straight into the caller's
            // buffer (FreeMiNT `tstart`/`tblocks`, `block_IO.c:2121-2134`).
            let start = k;
            k += 1;
            while k < n && self.find(lba + k as u64) == NIL {
                k += 1;
            }
            self.stats.dev_reads += 1;
            self.stats.miss_sectors += (k - start) as u64;
            dev.read_blocks(lba + start as u64, &mut buf[start * SECTOR..k * SECTOR])?;
            let run = (k - start) * SECTOR;
            if run * 100 <= self.capacity_bytes() * self.stats.pct as usize {
                for s in start..k {
                    self.put(lba + s as u64, &buf[s * SECTOR..(s + 1) * SECTOR]);
                }
            } else {
                self.stats.runs_not_inserted += 1;
            }
        }
        Ok(())
    }

    /// Write `buf.len() / 512` sectors to `lba`, write-through (spec §2.5).
    pub fn write(&mut self, dev: &dyn BlockDev, lba: u64, buf: &[u8]) -> BlockResult {
        let n = buf.len() / SECTOR;
        self.stats.dev_writes += 1;
        if n == 0 || !buf.len().is_multiple_of(SECTOR) {
            // Not a whole-sector request: whatever the device does with it,
            // no cached sector it may touch survives.
            let r = dev.write_blocks(lba, buf);
            self.invalidate_range(lba, buf.len().div_ceil(SECTOR) as u64);
            return r;
        }
        let small = buf.len() <= SMALL_WRITE_BYTES;
        if !small {
            // `bio_large_write`: drop the range, then the direct transfer.
            self.invalidate_range(lba, n as u64);
        }
        let r = dev.write_blocks(lba, buf);
        match r {
            Ok(()) => {
                if small {
                    for s in 0..n {
                        self.put(lba + s as u64, &buf[s * SECTOR..(s + 1) * SECTOR]);
                    }
                }
            }
            Err(_) => self.invalidate_range(lba, n as u64),
        }
        r
    }
}

// ── The system's cache ─────────────────────────────────────────────────

struct Global(UnsafeCell<Option<BlockCache>>);
// SAFETY: accessed only from the single-CPU, IRQ-off trap path and the boot
// CPU before it (host tests serialise their GEMDOS calls with a lock).
unsafe impl Sync for Global {}

static GLOBAL: Global = Global(UnsafeCell::new(None));

fn cache() -> Option<&'static mut BlockCache> {
    // SAFETY: single trap path; the reference does not outlive one request.
    unsafe { (*GLOBAL.0.get()).as_mut() }
}

fn raw() -> Option<&'static dyn BlockDev> {
    host::host().and_then(|h| h.block())
}

/// Install the system's cache over `bytes` bytes at `arena` (spec §2.4).
/// Replaces any cache installed before (its contents are forgotten). Returns
/// the new cache's counters (slot count), or `None` — and leaves no cache
/// installed — if the arena cannot hold one (see [`BlockCache::new`]).
///
/// # Safety
///
/// As [`BlockCache::new`]: the arena belongs to the cache from now on.
pub unsafe fn install(arena: *mut u8, bytes: usize, pct: u32) -> Option<Stats> {
    // SAFETY: forwarded contract.
    let c = unsafe { BlockCache::new(arena, bytes, pct) };
    let s = c.as_ref().map(|c| c.stats());
    // SAFETY: single trap path / boot CPU.
    unsafe { *GLOBAL.0.get() = c };
    s
}

/// Remove the system's cache; [`device`] passes everything straight through
/// again. The arena is no longer used afterwards.
pub fn uninstall() {
    // SAFETY: single trap path / boot CPU.
    unsafe { *GLOBAL.0.get() = None };
}

/// Drop every cached sector — a new partition scan (`bios::mount`).
pub fn invalidate_all() {
    if let Some(c) = cache() {
        c.invalidate_all();
    }
}

/// The system cache's counters, `None` without a cache.
pub fn stats() -> Option<Stats> {
    cache().map(|c| c.stats())
}

/// The block device as everything in `tos-api` must use it: the host's
/// device behind the cache. `None` if the host has no block device.
pub fn device() -> Option<&'static dyn BlockDev> {
    raw().map(|_| &CACHED as &'static dyn BlockDev)
}

struct CachedDev;
static CACHED: CachedDev = CachedDev;

impl BlockDev for CachedDev {
    fn block_count(&self) -> u64 {
        raw().map_or(0, |d| d.block_count())
    }

    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> BlockResult {
        let d = raw().ok_or(EUNDEV)?;
        match cache() {
            Some(c) => c.read(d, lba, buf),
            None => d.read_blocks(lba, buf),
        }
    }

    fn write_blocks(&self, lba: u64, buf: &[u8]) -> BlockResult {
        let d = raw().ok_or(EUNDEV)?;
        match cache() {
            Some(c) => c.write(d, lba, buf),
            None => d.write_blocks(lba, buf),
        }
    }
}
