//! ARCH03 stage 3c — the page-table arena runs out: no silent degradation.
//!
//! A process whose rights differ from everybody else's in a 2-MiB region
//! needs a private L3 table from the arena (`memprot` copy-on-differ). When
//! the arena is empty the process keeps the shared, non-owner rights there —
//! it can lose access, never gain any — and that is **counted** (since boot
//! and per process) and **reported once per process**
//! (`memprot::take_shortfall_report`, printed by the kernel at the end of
//! the trap). The next process in the same slot is reported on its own.
//!
//! The arena here is small on purpose (its own test binary).

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::traps;
use tos_api::bdos::process::mmu;
use tos_api::bdos::process::{self, memprot, PROCESS_TABLE};
use tos_types::{BasePage, Long, MX_TTRAM};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024;
/// The shared L3 tables of the two pools (5 + 32 regions) plus a few.
const ARENA_TABLES: usize = 44;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

#[repr(C, align(4096))]
struct Page([u8; 4096]);

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        let arena: Vec<Page> = (0..ARENA_TABLES).map(|_| Page([0; 4096])).collect();
        let arena = Box::leak(arena.into_boxed_slice());
        // SAFETY: once per binary, before the pools exist.
        unsafe {
            memprot::configure(true, false);
            // MEMPROT-FAST: every sync is checked against a full recomputation.
            memprot::set_verify(true, true);
            mmu::set_table_arena(arena.as_mut_ptr() as u64, ARENA_TABLES);
            tos_api::bdos::mem::init::init(ST_START, ST_LEN, TT_START, TT_LEN).expect("pools");
            mmu::map_pools().expect("the pools map");
        }
    });
    g
}

struct Proc {
    slot: usize,
    bp: *mut BasePage,
}

impl Proc {
    fn new() -> Proc {
        let slot = process::allocate_slot().expect("a slot");
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        let p = &PROCESS_TABLE[slot];
        p.basepage_kva.store(bp, Ordering::Release);
        // SAFETY: under the lock; the slot is not an installed root.
        unsafe { mmu::init_slot_tables(slot) }.unwrap();
        p.l0_pa.store(mmu::l0_pa_for(slot), Ordering::Release);
        p.set_state(process::State::Running);
        // SAFETY: under the lock.
        unsafe { memprot::on_root_built(slot) };
        Proc { slot, bp }
    }

    fn alloc(&self, n: Long, mode: i16) -> u64 {
        process::set_current(process::pd_of(self.slot));
        // SAFETY: under the lock.
        let a = unsafe { traps::mxalloc(n, mode) };
        process::set_current(std::ptr::null_mut());
        assert_ne!(a, 0, "allocation");
        a as u32 as u64
    }

    /// Allocate private 2-MiB blocks (each in regions of its own) until the
    /// arena has no table left for this process; the blocks that could not
    /// get one.
    fn exhaust(&self) -> Vec<u64> {
        let mut short = Vec::new();
        for _ in 0..30 {
            let before = memprot::table_shortfalls_of(self.slot);
            let b = self.alloc(2 * 1024 * 1024, MX_TTRAM);
            if memprot::table_shortfalls_of(self.slot) > before {
                short.push(b);
                if short.len() >= 2 {
                    break;
                }
            }
        }
        short
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        // SAFETY: under the lock.
        unsafe {
            tos_api::bdos::mem::free_all_owned::free_all_owned(process::pd_of(self.slot).cast())
        };
        process::free_slot(self.slot);
        // SAFETY: allocated by `Box::new` in `new`.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

#[test]
fn an_exhausted_arena_is_counted_and_reported_once_per_process() {
    let _g = lock();
    while memprot::take_shortfall_report().is_some() {}
    let total_before = memprot::table_shortfalls();

    let a = Proc::new();
    let short = a.exhaust();
    assert!(!short.is_empty(), "the small arena runs out");
    assert!(memprot::table_shortfalls_of(a.slot) >= short.len());
    assert!(memprot::table_shortfalls() > total_before, "counted since boot");

    // It can lose access, never gain any: its own private block in a region
    // without a private table is invalid for it (the non-owner pattern).
    let last = *short.last().unwrap();
    let pages = (0..2 * 1024 * 1024 / mmu::PAGE_SIZE).map(|i| last + i * mmu::PAGE_SIZE);
    let lost = pages.filter(|&va| mmu::translate(a.slot, va).is_none()).count();
    assert!(lost > 0, "a shortfall leaves the owner without access to part of its block");

    // Reported once for this process ...
    let r = memprot::take_shortfall_report().expect("a report");
    assert_eq!(r.slot, a.slot);
    assert_eq!(r.pd, process::pd_of(a.slot) as u64);
    assert!(r.of_process >= 1);
    assert_eq!(r.total, memprot::table_shortfalls());
    assert_eq!(memprot::take_shortfall_report(), None, "once");
    // ... also after further shortfalls of the same process.
    let more = a.alloc(2 * 1024 * 1024, MX_TTRAM);
    let _ = more;
    assert_eq!(memprot::take_shortfall_report(), None, "once per process");

    // The slot's next process is reported on its own.
    let slot = a.slot;
    drop(a);
    assert_eq!(memprot::table_shortfalls_of(slot), 0, "the per-process count starts afresh");
    let b = Proc::new();
    let short_b = b.exhaust();
    if !short_b.is_empty() {
        let r = memprot::take_shortfall_report().expect("the next process is reported");
        assert_eq!(r.slot, b.slot);
    }
}
