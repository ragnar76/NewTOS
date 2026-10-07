//! FS09 block read cache (`tos_api::bios::bcache::BlockCache`) on its own:
//! hits and misses, one device request per run of misses, LRU eviction, the
//! linear-read share, write-through with partial overlap, write/read failures,
//! invalidation, and a long random sequence checked against the card.
//!
//! Spec: `docs/analysis/fs09-block-cache-2026-10-07.md` §2. Every test owns
//! its cache and its disk (no statics), so they run in parallel.

use std::sync::Mutex;

use tos_api::bios::bcache::{BlockCache, SECTOR, SMALL_WRITE_BYTES};
use tos_api::bios::block::BlockDev;

const EWRITF: i32 = -10;
const EREADF: i32 = -11;
const ESECNF: i32 = -8;

/// One device request: `(write, lba, sectors)`.
type Req = (bool, u64, u32);

/// A RAM disk with a request log and fault injection.
struct Disk {
    img: Mutex<Vec<u8>>,
    log: Mutex<Vec<Req>>,
    /// Reads covering this LBA fail with `EREADF`.
    bad_read: Mutex<Option<u64>>,
    /// Writes covering this LBA store the sectors **before** it, then fail
    /// with `EWRITF` — an aborted multi-block write.
    bad_write: Mutex<Option<u64>>,
}

impl Disk {
    fn new(sectors: usize) -> Disk {
        let mut img = vec![0u8; sectors * SECTOR];
        for (i, b) in img.iter_mut().enumerate() {
            *b = (i as u32).wrapping_mul(2_654_435_761).rotate_left(7) as u8;
        }
        Disk {
            img: Mutex::new(img),
            log: Mutex::new(Vec::new()),
            bad_read: Mutex::new(None),
            bad_write: Mutex::new(None),
        }
    }
    fn take_log(&self) -> Vec<Req> {
        std::mem::take(&mut *self.log.lock().unwrap())
    }
    fn sectors(&self, lba: u64, n: usize) -> Vec<u8> {
        let img = self.img.lock().unwrap();
        img[lba as usize * SECTOR..(lba as usize + n) * SECTOR].to_vec()
    }
}

impl BlockDev for Disk {
    fn block_count(&self) -> u64 {
        (self.img.lock().unwrap().len() / SECTOR) as u64
    }
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<(), i32> {
        let n = buf.len() / SECTOR;
        self.log.lock().unwrap().push((false, lba, n as u32));
        if let Some(b) = *self.bad_read.lock().unwrap() {
            if (lba..lba + n as u64).contains(&b) {
                return Err(EREADF);
            }
        }
        let img = self.img.lock().unwrap();
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return Err(ESECNF);
        }
        buf[..n * SECTOR].copy_from_slice(&img[off..off + n * SECTOR]);
        Ok(())
    }
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<(), i32> {
        let n = buf.len() / SECTOR;
        self.log.lock().unwrap().push((true, lba, n as u32));
        let mut img = self.img.lock().unwrap();
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return Err(ESECNF);
        }
        let stop = match *self.bad_write.lock().unwrap() {
            Some(b) if (lba..lba + n as u64).contains(&b) => Some((b - lba) as usize),
            _ => None,
        };
        let good = stop.unwrap_or(n);
        img[off..off + good * SECTOR].copy_from_slice(&buf[..good * SECTOR]);
        match stop {
            Some(_) => Err(EWRITF),
            None => Ok(()),
        }
    }
}

/// A cache with room for exactly `slots` sectors (the arena is sized from the
/// layout `BlockCache::new` uses, then checked).
fn cache(slots: u32, pct: u32) -> (Vec<u64>, BlockCache) {
    // Per slot: 512 data + 24 bookkeeping + up to 8 hash heads; one sector
    // of alignment slack. Grow until the cache reports `slots` slots.
    let mut bytes = slots as usize * (SECTOR + 32) + SECTOR;
    loop {
        let mut arena = vec![0u64; bytes.div_ceil(8)];
        // SAFETY: the arena outlives the cache (returned together; the Vec's
        // heap block does not move when the Vec does).
        let c = unsafe { BlockCache::new(arena.as_mut_ptr().cast(), bytes, pct) }.unwrap();
        if c.slots() == slots {
            return (arena, c);
        }
        assert!(c.slots() < slots, "arena layout gave more slots than expected");
        bytes += SECTOR;
    }
}

fn read(c: &mut BlockCache, d: &Disk, lba: u64, n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n * SECTOR];
    c.read(d, lba, &mut b).expect("read");
    b
}

fn pattern(seed: u8, n: usize) -> Vec<u8> {
    (0..n * SECTOR).map(|i| (i as u8).wrapping_mul(13) ^ seed).collect()
}

#[test]
fn new_refuses_what_cannot_be_a_cache() {
    let mut a = vec![0u64; 16];
    // Too small for one slot.
    assert!(unsafe { BlockCache::new(a.as_mut_ptr().cast(), 128, 5) }.is_none());
    let mut big = vec![0u64; 1024];
    // pct > 100 (FreeMiNT bio_set_percentage: EBADARG).
    assert!(unsafe { BlockCache::new(big.as_mut_ptr().cast(), 8192, 101) }.is_none());
    // Misaligned.
    assert!(unsafe { BlockCache::new(big.as_mut_ptr().cast::<u8>().add(1), 8000, 5) }.is_none());
    let c = unsafe { BlockCache::new(big.as_mut_ptr().cast(), 8192, 5) }.unwrap();
    assert!(c.slots() >= 13 && c.slots() <= 15, "8 KiB arena -> {} slots", c.slots());
    assert_eq!(c.capacity_bytes(), c.slots() as usize * SECTOR);
}

#[test]
fn second_read_is_served_from_the_cache() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    let first = read(&mut c, &d, 10, 4);
    assert_eq!(d.take_log(), vec![(false, 10, 4)], "one request for the miss run");
    assert_eq!(first, d.sectors(10, 4));
    let second = read(&mut c, &d, 10, 4);
    assert!(d.take_log().is_empty(), "no request for a hit");
    assert_eq!(second, first);
    let s = c.stats();
    assert_eq!((s.hit_sectors, s.miss_sectors, s.dev_reads, s.inserted), (4, 4, 1, 4));
}

#[test]
fn a_large_request_reads_only_the_gaps_one_request_per_run() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    read(&mut c, &d, 10, 2); // 10, 11
    read(&mut c, &d, 15, 1); // 15
    d.take_log();
    let got = read(&mut c, &d, 8, 10); // 8..18
    assert_eq!(d.take_log(), vec![(false, 8, 2), (false, 12, 3), (false, 16, 2)]);
    assert_eq!(got, d.sectors(8, 10), "byte-exact across hits and misses");
    // Everything is cached now.
    assert_eq!(read(&mut c, &d, 8, 10), got);
    assert!(d.take_log().is_empty());
}

#[test]
fn lru_evicts_the_least_recently_used_sector() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(4, 100);
    for l in 0..4 {
        read(&mut c, &d, l, 1);
    }
    read(&mut c, &d, 0, 1); // 0 is now most recent; 1 is the LRU
    let mut order = [0u64; 8];
    let k = c.lru_order(&mut order);
    assert_eq!(&order[..k], &[0, 3, 2, 1]);
    read(&mut c, &d, 4, 1);
    assert!(!c.contains(1), "LRU victim");
    assert!(c.contains(0) && c.contains(2) && c.contains(3) && c.contains(4));
    let k = c.lru_order(&mut order);
    assert_eq!(&order[..k], &[4, 0, 3, 2]);
    assert_eq!(c.stats().evicted, 1);
    d.take_log();
    // The evicted sector comes from the card again, byte-exact.
    assert_eq!(read(&mut c, &d, 1, 1), d.sectors(1, 1));
    assert_eq!(d.take_log(), vec![(false, 1, 1)]);
}

#[test]
fn a_run_larger_than_the_linear_share_is_not_inserted() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(8, 50); // 50 % of 8 slots = 4 sectors per run
    read(&mut c, &d, 0, 4);
    assert_eq!(c.stats().used, 4, "4 sectors = exactly the share: inserted");
    let got = read(&mut c, &d, 20, 5);
    assert_eq!(got, d.sectors(20, 5), "the caller still gets the data");
    let s = c.stats();
    assert_eq!((s.used, s.runs_not_inserted), (4, 1));
    // Hits inside a big request do not count against the share: 0..4 cached,
    // 4..8 a run of 4 -> inserted.
    d.take_log();
    read(&mut c, &d, 0, 8);
    assert_eq!(d.take_log(), vec![(false, 4, 4)]);
    assert_eq!(c.stats().used, 8);
}

#[test]
fn small_write_is_written_through_and_updates_partial_overlap() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    let before = read(&mut c, &d, 0, 8);
    d.take_log();
    let new = pattern(0x5A, 2);
    c.write(&d, 3, &new).unwrap();
    assert_eq!(d.take_log(), vec![(true, 3, 2)], "write goes to the card at once");
    assert_eq!(d.sectors(3, 2), new, "card holds the new data");
    let after = read(&mut c, &d, 0, 8);
    assert!(d.take_log().is_empty(), "all eight still cached");
    assert_eq!(&after[..3 * SECTOR], &before[..3 * SECTOR]);
    assert_eq!(&after[3 * SECTOR..5 * SECTOR], &new[..]);
    assert_eq!(&after[5 * SECTOR..], &before[5 * SECTOR..]);
    assert_eq!(after, d.sectors(0, 8));
}

#[test]
fn small_write_allocates_like_bio_small_write() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    let new = pattern(0x11, 3);
    c.write(&d, 40, &new).unwrap();
    d.take_log();
    assert_eq!(read(&mut c, &d, 40, 3), new);
    assert!(d.take_log().is_empty());
}

#[test]
fn large_write_drops_the_range_and_the_next_read_asks_the_card() {
    let n = SMALL_WRITE_BYTES / SECTOR + 1; // 65 sectors
    let d = Disk::new(256);
    let (_a, mut c) = cache(200, 100);
    read(&mut c, &d, 0, 100);
    let new = pattern(0x77, n);
    d.take_log();
    c.write(&d, 10, &new).unwrap();
    assert_eq!(d.take_log(), vec![(true, 10, n as u32)]);
    for l in 10..10 + n as u64 {
        assert!(!c.contains(l), "sector {l} must be dropped");
    }
    assert!(c.contains(9) && c.contains(10 + n as u64), "neighbours stay");
    let got = read(&mut c, &d, 0, 100);
    assert_eq!(d.take_log(), vec![(false, 10, n as u32)]);
    assert_eq!(got, d.sectors(0, 100));
    assert_eq!(&got[10 * SECTOR..(10 + n) * SECTOR], &new[..]);
}

#[test]
fn failed_write_drops_the_range_and_the_card_is_the_truth() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    read(&mut c, &d, 0, 16);
    *d.bad_write.lock().unwrap() = Some(6); // 4, 5 land; 6, 7 do not
    let new = pattern(0x33, 4);
    assert_eq!(c.write(&d, 4, &new), Err(EWRITF), "code passed through");
    for l in 4..8 {
        assert!(!c.contains(l), "sector {l} must be dropped after a failed write");
    }
    *d.bad_write.lock().unwrap() = None;
    d.take_log();
    let got = read(&mut c, &d, 0, 16);
    assert_eq!(d.take_log(), vec![(false, 4, 4)]);
    assert_eq!(got, d.sectors(0, 16), "what the card holds, not what was asked");
    assert_eq!(&got[4 * SECTOR..6 * SECTOR], &new[..2 * SECTOR]);
}

#[test]
fn failed_read_inserts_nothing_and_returns_the_code() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    read(&mut c, &d, 0, 2);
    *d.bad_read.lock().unwrap() = Some(5);
    let mut b = vec![0u8; 8 * SECTOR];
    assert_eq!(c.read(&d, 0, &mut b), Err(EREADF));
    assert_eq!(&b[..2 * SECTOR], &d.sectors(0, 2)[..], "hits before the run were copied");
    for l in 2..8 {
        assert!(!c.contains(l), "nothing from the failed run");
    }
    *d.bad_read.lock().unwrap() = None;
    d.take_log();
    assert_eq!(read(&mut c, &d, 0, 8), d.sectors(0, 8));
    assert_eq!(d.take_log(), vec![(false, 2, 6)]);
}

#[test]
fn invalidate_all_forgets_everything() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    read(&mut c, &d, 0, 10);
    c.invalidate_all();
    assert_eq!(c.stats().used, 0);
    assert_eq!(c.stats().invalidations, 1);
    d.take_log();
    read(&mut c, &d, 0, 10);
    assert_eq!(d.take_log(), vec![(false, 0, 10)]);
}

#[test]
fn invalidate_range_both_walks() {
    let d = Disk::new(256);
    let (_a, mut c) = cache(64, 100);
    read(&mut c, &d, 0, 20);
    // Short range: looked up sector by sector.
    c.invalidate_range(3, 2);
    assert!(!c.contains(3) && !c.contains(4) && c.contains(2) && c.contains(5));
    // Range longer than the cache holds: walked over the slots.
    c.invalidate_range(10, 200);
    for l in 10..20 {
        assert!(!c.contains(l));
    }
    assert!(c.contains(9));
    assert_eq!(c.stats().used, 8);
}

#[test]
fn requests_that_are_not_whole_sectors_pass_through() {
    let d = Disk::new(64);
    let (_a, mut c) = cache(32, 100);
    read(&mut c, &d, 2, 1);
    d.take_log();
    let mut b = vec![0u8; 100];
    c.read(&d, 2, &mut b).unwrap();
    assert_eq!(d.take_log().len(), 1, "partial-sector read goes to the device");
    // A partial-sector write drops the sector it touches.
    c.write(&d, 2, &[0xEE; 100]).unwrap();
    assert!(!c.contains(2));
}

/// xorshift64*, deterministic.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Random reads, small and large writes, failing writes (with partial
/// stores) and failing reads, over a cache much smaller than the disk: every
/// read must return exactly what the card holds at that moment.
fn random_against_the_card(seed: u64, slots: u32, pct: u32) {
    const DISK: u64 = 300;
    let d = Disk::new(DISK as usize);
    let (_a, mut c) = cache(slots, pct);
    let mut rng = Rng(seed);
    let mut writes_failed = 0;
    for step in 0..6000 {
        let n = 1 + match rng.below(10) {
            0 => rng.below(90), // sometimes large (> 64 sectors = large write)
            _ => rng.below(12),
        };
        let lba = rng.below(DISK - n);
        match rng.below(10) {
            0..=5 => {
                let fail = rng.below(40) == 0;
                if fail {
                    *d.bad_read.lock().unwrap() = Some(lba + rng.below(n));
                }
                let mut b = vec![0u8; n as usize * SECTOR];
                let r = c.read(&d, lba, &mut b);
                *d.bad_read.lock().unwrap() = None;
                if !fail {
                    r.unwrap();
                    assert_eq!(b, d.sectors(lba, n as usize), "step {step}: read {lba}+{n}");
                }
            }
            6..=8 => {
                let fail = rng.below(15) == 0;
                if fail {
                    *d.bad_write.lock().unwrap() = Some(lba + rng.below(n));
                }
                let data = pattern(rng.next() as u8, n as usize);
                let r = c.write(&d, lba, &data);
                *d.bad_write.lock().unwrap() = None;
                if fail {
                    assert_eq!(r, Err(EWRITF));
                    writes_failed += 1;
                } else {
                    r.unwrap();
                    assert_eq!(d.sectors(lba, n as usize), data);
                }
            }
            _ => c.invalidate_range(lba, n),
        }
    }
    assert!(writes_failed > 10, "the sequence exercised failing writes");
    // Final sweep: every sector, through the cache, equals the card.
    for l in 0..DISK {
        assert_eq!(read(&mut c, &d, l, 1), d.sectors(l, 1), "final sector {l}");
    }
    let s = c.stats();
    assert!(s.hit_sectors > 0 && s.evicted > 0, "{s:?}");
}

#[test]
fn random_sequence_matches_the_card_full_share() {
    random_against_the_card(0x9E37_79B9_7F4A_7C15, 37, 100);
}

#[test]
fn random_sequence_matches_the_card_freemint_share() {
    random_against_the_card(0xD1B5_4A32_D192_ED03, 120, 5);
}

#[test]
fn random_sequence_matches_the_card_tiny_cache() {
    random_against_the_card(0x1234_5678_9ABC_DEF1, 1, 100);
}
