//! Sector cache (Phase 09 D2). Mirrors EmuTOS `bdos/fsbuf.c`.
//!
//! A small write-back cache of 512-byte sectors sitting between the FAT /
//! directory / file layers and the block device. Reads populate it;
//! repeated accesses to the same FAT or directory sector (the common case
//! in `fat_entry`, the directory scan and `Dfree`) hit the cache instead
//! of re-issuing an SD read. Writes mark slots dirty for write-back; the
//! write side flushes via [`flush`].
//!
//! Single block device, single-CPU, IRQ-off trap path — no locking.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::bios::block::BlockDev;
use crate::host;

const SLOTS: usize = 16;
const SECTOR: usize = 512;

struct Slot {
    lba: u64,
    valid: bool,
    dirty: bool,
    data: [u8; SECTOR],
}

struct Cache {
    slots: UnsafeCell<[Slot; SLOTS]>,
    /// Round-robin eviction pointer.
    victim: AtomicUsize,
}
// SAFETY: accessed only from the single-CPU, IRQ-off trap path.
unsafe impl Sync for Cache {}

static CACHE: Cache = Cache {
    slots: UnsafeCell::new(
        [const {
            Slot {
                lba: 0,
                valid: false,
                dirty: false,
                data: [0u8; SECTOR],
            }
        }; SLOTS],
    ),
    victim: AtomicUsize::new(0),
};

static HITS: AtomicU32 = AtomicU32::new(0);
static MISSES: AtomicU32 = AtomicU32::new(0);

fn block() -> Option<&'static dyn BlockDev> {
    host::host().and_then(|h| h.block())
}

fn slots() -> &'static mut [Slot; SLOTS] {
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut *CACHE.slots.get() }
}

/// Pick a victim slot (round robin), writing it back first if dirty.
fn evict(slots: &mut [Slot; SLOTS]) -> usize {
    let v = CACHE.victim.fetch_add(1, Ordering::Relaxed) % SLOTS;
    if slots[v].valid && slots[v].dirty {
        if let Some(b) = block() {
            b.write_blocks(slots[v].lba, &slots[v].data);
        }
        slots[v].dirty = false;
    }
    v
}

/// Read sector `lba` into `out`, through the cache.
pub fn read(lba: u64, out: &mut [u8; SECTOR]) -> bool {
    let slots = slots();
    for s in slots.iter() {
        if s.valid && s.lba == lba {
            out.copy_from_slice(&s.data);
            HITS.fetch_add(1, Ordering::Relaxed);
            return true;
        }
    }
    MISSES.fetch_add(1, Ordering::Relaxed);

    let v = evict(slots);
    let ok = match block() {
        Some(b) => b.read_blocks(lba, &mut slots[v].data),
        None => false,
    };
    if !ok {
        slots[v].valid = false;
        return false;
    }
    slots[v].lba = lba;
    slots[v].valid = true;
    slots[v].dirty = false;
    out.copy_from_slice(&slots[v].data);
    true
}

/// Write sector `lba` from `data` into the cache (write-back: the slot is
/// marked dirty and flushed later).
pub fn write(lba: u64, data: &[u8; SECTOR]) -> bool {
    let slots = slots();
    for s in slots.iter_mut() {
        if s.valid && s.lba == lba {
            s.data.copy_from_slice(data);
            s.dirty = true;
            return true;
        }
    }
    let v = evict(slots);
    slots[v].data.copy_from_slice(data);
    slots[v].lba = lba;
    slots[v].valid = true;
    slots[v].dirty = true;
    true
}

/// Write back all dirty sectors to the block device.
pub fn flush() -> bool {
    let slots = slots();
    let mut ok = true;
    for s in slots.iter_mut() {
        if s.valid && s.dirty {
            match block() {
                Some(b) if b.write_blocks(s.lba, &s.data) => s.dirty = false,
                _ => ok = false,
            }
        }
    }
    ok
}

/// Cache statistics: `(hits, misses)`.
pub fn stats() -> (u32, u32) {
    (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed))
}
