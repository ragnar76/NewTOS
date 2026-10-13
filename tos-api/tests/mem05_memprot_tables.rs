//! MEM05 — the tables that hold one entry per allocated block grow with the
//! MD supply (design `docs/analysis/mem05-design-2026-10-13.md` §2.4, test 7;
//! decision B in `TASK/MEM05.md`): the protection records (`RECS`), the
//! per-sync snapshot of all blocks (`SNAP`) and the `F_KEEP` flags (`KEPT`).
//!
//! * Far more live blocks than the old fixed tables (260 entries) held: every
//!   block keeps its owner's rights and nobody else's — the snapshot must not
//!   stop at a fixed size (it did silently), checked against the complete
//!   recomputation of the verification mode.
//! * The capacity is secured in `mxalloc` **before** `ffit`: when no table
//!   page can be had the allocation returns 0 and nothing changes; a record
//!   never fails after a successful `ffit`.
//! * Scans cost the live entries, not the capacity: `forget` (every `Malloc`
//!   and `Mfree`) looks at nothing while no block is `F_KEEP`.
//!
//! Pools, arena and processes as in `memprot_fast.rs`; the verification mode
//! runs at checkpoints (a complete recomputation after each of hundreds of
//! syncs would only be slow).

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::pages::{self, MD_PAGE_BUDGET};
use tos_api::bdos::mem::{free_all_owned, init, mdpool, owner_limit, region, traps};
use tos_api::bdos::process::mmu;
use tos_api::bdos::process::{self, memprot, PROCESS_TABLE};
use tos_types::{BasePage, Long, F_KEEP, MX_TTRAM};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024;
const ARENA_TABLES: usize = 512;
const PAGE: Long = 4096;
/// Live blocks well beyond the old fixed tables (86 MD blocks × 3 + 2 = 260).
const MANY: usize = 700;

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
            memprot::set_verify(false, true);
            mmu::set_table_arena(arena.as_mut_ptr() as u64, ARENA_TABLES);
            init::init(ST_START, ST_LEN, TT_START, TT_LEN).expect("pools");
            mmu::map_pools().expect("the pools map");
        }
    });
    pages::set_budget(MD_PAGE_BUDGET);
    memprot::set_verify(false, true);
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

    fn pd(&self) -> *mut core::ffi::c_void {
        process::pd_of(self.slot).cast()
    }

    fn try_alloc(&self, n: Long, mode: i16) -> Long {
        process::set_current(process::pd_of(self.slot));
        // SAFETY: under the lock.
        let a = unsafe { traps::mxalloc(n, mode) };
        process::set_current(std::ptr::null_mut());
        a
    }

    fn alloc(&self, n: Long, mode: i16) -> Long {
        let a = self.try_alloc(n, mode);
        assert_ne!(a, 0, "allocation");
        a
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        // SAFETY: under the lock.
        unsafe { free_all_owned::free_all_owned(self.pd()) };
        process::free_slot(self.slot);
        // SAFETY: allocated by `Box::new` in `new`.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

fn rights(slot: usize, a: Long) -> Option<u64> {
    mmu::translate(slot, a as u32 as u64).map(|(_, d)| (d >> 6) & 0b11)
}

const RW: Option<u64> = Some(0b01);

/// A complete recomputation agrees with the tables (panics otherwise).
fn verified() {
    // SAFETY: under the lock.
    assert_eq!(unsafe { memprot::verify_now() }, 0, "tables = complete recomputation");
}

#[test]
fn far_more_blocks_than_the_old_tables_keep_their_rights() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let t0 = memprot::tables();
    let blocks: Vec<Long> = (0..MANY).map(|_| a.alloc(PAGE, MX_TTRAM)).collect();
    let t1 = memprot::tables();
    assert!(t1.records >= t0.records + MANY, "one record per block");
    assert!(t1.record_capacity > t1.records, "room for the next one, reserved before ffit");
    assert!(t1.snapshot_capacity > t1.records, "the snapshot holds every block");
    assert_eq!(t1.snapshot_shortfalls, 0, "never a truncated snapshot");
    let ps = pages::stats();
    assert!(ps.recs >= 3 && ps.snap >= 13, "table pages from the budget: {ps:?}");
    assert!(ps.held <= ps.budget);
    verified();
    for &blk in &blocks {
        assert_eq!(rights(a.slot, blk), RW, "the owner reaches {blk:#x}");
        assert_eq!(rights(b.slot, blk), None, "nobody else reaches {blk:#x}");
    }
    // Free every other one, then the rest: records follow, rights go.
    for &blk in blocks.iter().step_by(2) {
        // SAFETY: under the lock.
        assert_eq!(unsafe { traps::mfree(blk) }, 0);
    }
    verified();
    for &blk in blocks.iter().skip(1).step_by(2) {
        assert_eq!(rights(a.slot, blk), RW);
    }
    for &blk in blocks.iter().skip(1).step_by(2) {
        // SAFETY: under the lock.
        assert_eq!(unsafe { traps::mfree(blk) }, 0);
    }
    verified();
    assert_eq!(memprot::tables().records, t0.records, "every record of a freed block forgotten");
    for &blk in &blocks {
        assert_eq!(rights(a.slot, blk), None, "{blk:#x} is free: invalid for everybody");
    }
}

#[test]
fn no_table_page_refuses_the_allocation_before_anything_changes() {
    let _g = lock();
    let a = Proc::new();
    // Fill the record table up to its last free entry under a frozen budget:
    // the first allocation that needs a new table page must fail cleanly.
    let mut blocks = Vec::new();
    let held = pages::stats().held;
    pages::set_budget(held);
    let refused0 = pages::stats().refused_budget;
    loop {
        let before = (
            memprot::tables(),
            mdpool::stats().in_use,
            // SAFETY: under the lock.
            unsafe { owner_limit::count_of(a.pd()) },
            // SAFETY: under the lock.
            unsafe { traps::mxalloc(-1, MX_TTRAM) },
        );
        let blk = a.try_alloc(PAGE, MX_TTRAM);
        if blk == 0 {
            let after = (
                memprot::tables(),
                mdpool::stats().in_use,
                // SAFETY: under the lock.
                unsafe { owner_limit::count_of(a.pd()) },
                // SAFETY: under the lock.
                unsafe { traps::mxalloc(-1, MX_TTRAM) },
            );
            assert_eq!(after.0.records, before.0.records, "no record");
            assert_eq!(after.1, before.1, "no MD taken");
            assert_eq!(after.2, before.2, "nothing booked");
            assert_eq!(after.3, before.3, "the pool is unchanged");
            break;
        }
        blocks.push(blk);
        assert!(blocks.len() < 20_000, "the frozen budget must stop the tables");
    }
    assert!(pages::stats().refused_budget > refused0);
    assert_eq!(pages::stats().held, held, "not one page beyond the budget");
    verified();
    for &blk in &blocks {
        assert_eq!(rights(a.slot, blk), RW);
    }
    pages::set_budget(MD_PAGE_BUDGET);
    blocks.push(a.alloc(PAGE, MX_TTRAM));
    verified();
    for blk in blocks {
        // SAFETY: under the lock.
        assert_eq!(unsafe { traps::mfree(blk) }, 0);
    }
}

#[test]
fn forget_scans_nothing_without_keep_blocks_and_keep_flags_grow() {
    let _g = lock();
    let a = Proc::new();
    assert_eq!(region::kept_len(), 0);
    let s0 = region::kept_scans();
    let blocks: Vec<Long> = (0..100).map(|_| a.alloc(PAGE, MX_TTRAM)).collect();
    for blk in blocks {
        // SAFETY: under the lock.
        assert_eq!(unsafe { traps::mfree(blk) }, 0);
    }
    assert_eq!(region::kept_scans(), s0, "forget looked at nothing: KEPT is empty");
    // More F_KEEP blocks than one page of flags (1024).
    let kept: Vec<Long> = (0..1100).map(|_| a.alloc(PAGE, MX_TTRAM | F_KEEP)).collect();
    assert_eq!(region::kept_len(), 1100);
    assert!(pages::stats().kept >= 2, "two pages of flags");
    for &blk in &kept {
        // SAFETY: under the lock.
        assert!(unsafe { region::is_kept(blk) });
    }
    verified();
    for &blk in &kept {
        // SAFETY: under the lock.
        assert_eq!(unsafe { traps::mfree(blk) }, 0);
    }
    assert_eq!(region::kept_len(), 0, "every flag forgotten with its block");
    for &blk in &kept {
        // SAFETY: under the lock.
        assert!(!unsafe { region::is_kept(blk) });
    }
}
