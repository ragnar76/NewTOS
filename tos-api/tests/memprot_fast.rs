//! MEMPROT-FAST — the protection resync recomputes only where rights
//! change (FreeMiNT `mark_region` shortcut,
//! `references/freemint/sys/arch/mprot040.c:1102-1130`) and a compound
//! operation enters its net change once (`memprot::batch`).
//!
//! Every sync in this binary is followed by the verification mode: a
//! complete, independent recomputation of the shared tables and every live
//! slot's view, compared entry by entry (`memprot::set_verify(true, true)`
//! panics at the first difference). The tests below add what the shortcut
//! must *not* do: the descriptor work (`memprot::stats().pages`) stays within
//! the slots and pages whose rights change. The bounds are what the stage-3b
//! algorithm exceeds (it recomputed the non-owner pattern and every live
//! slot over the whole block, and every pool page of every slot on a
//! rebuild).
//!
//! Pool and arena as in `arch03_3b_memprot.rs`; one binary, one pool, one
//! lock.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::{free_all_owned, region, traps};
use tos_api::bdos::process::mmu;
use tos_api::bdos::process::{self, memprot, PROCESS_TABLE};
use tos_types::{BasePage, Long, ProtMode, MX_TTRAM};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024;
const ARENA_TABLES: usize = 512;

/// A "TPA-sized" block for the tests: 8 MiB spans four or five 2-MiB
/// regions.
const BIG: Long = 8 * 1024 * 1024;
const REGION: u64 = 2 * 1024 * 1024;

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

    fn pd(&self) -> *mut core::ffi::c_void {
        process::pd_of(self.slot).cast()
    }

    fn alloc(&self, n: Long, mode: i16) -> u64 {
        process::set_current(process::pd_of(self.slot));
        // SAFETY: under the lock.
        let a = unsafe { traps::mxalloc(n, mode) };
        process::set_current(std::ptr::null_mut());
        assert_ne!(a, 0, "allocation");
        a as u32 as u64
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

fn access(slot: usize, va: u64) -> Option<u64> {
    mmu::translate(slot, va).map(|(pa, d)| {
        assert_eq!(pa, va, "VA = PA");
        (d >> 6) & 0b11
    })
}

const RW: Option<u64> = Some(0b01);
const RO: Option<u64> = Some(0b11);

/// Descriptor work since `before`.
fn work_since(before: memprot::Stats) -> u64 {
    memprot::stats().pages - before.pages
}

/// The 2-MiB regions `[a, a + len)` touches.
fn regions(a: u64, len: u64) -> u64 {
    ((a + len).div_ceil(REGION)) - a / REGION
}

/// Pages of `[a, a + len)` (the stage-3b work for one table).
fn pages(len: u64) -> u64 {
    len / 4096
}

#[test]
fn the_same_mode_or_an_equivalent_one_costs_nothing() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let blk = a.alloc(BIG, MX_TTRAM);
    let w = memprot::stats();
    // SAFETY: under the lock.
    unsafe { memprot::set_mode(blk, ProtMode::P) };
    assert_eq!(work_since(w), 0, "PROT_P → PROT_P: nothing to recompute");
    // PROT_S: owners read/write, others nothing — the same descriptors.
    // SAFETY: under the lock.
    unsafe { memprot::set_mode(blk, ProtMode::S) };
    assert_eq!(work_since(w), 0, "PROT_P → PROT_S: identical descriptors");
    assert_eq!(access(a.slot, blk), RW);
    assert_eq!(access(b.slot, blk), None);
    // PROT_G changes what everybody else sees.
    // SAFETY: under the lock.
    unsafe { memprot::set_mode(blk, ProtMode::G) };
    assert!(work_since(w) > 0);
    assert_eq!(access(b.slot, blk + BIG as u64 - 4096), RW);
    // SAFETY: under the lock.
    unsafe { memprot::set_mode(blk, ProtMode::P) };
    assert_eq!(access(b.slot, blk), None);
    assert_eq!(access(a.slot, blk), RW);
}

#[test]
fn a_private_allocation_touches_only_its_owners_tables() {
    let _g = lock();
    let a = Proc::new();
    let _b = Proc::new();
    let _c = Proc::new();
    let w = memprot::stats();
    let blk = a.alloc(BIG, MX_TTRAM);
    let r = regions(blk, BIG as u64);
    // The owner's new private copies (whole regions); the non-owner pattern
    // stays invalid, the other slots' views stay the same.
    assert!(work_since(w) <= r * 512, "work {} > {} (owner only)", work_since(w), r * 512);
    assert_eq!(access(a.slot, blk), RW);
    assert_eq!(access(_b.slot, blk), None);
}

#[test]
fn mshrink_recomputes_only_the_tail() {
    let _g = lock();
    let a = Proc::new();
    let blk = a.alloc(BIG, MX_TTRAM);
    let w = memprot::stats();
    process::set_current(process::pd_of(a.slot));
    // SAFETY: under the lock.
    assert_eq!(unsafe { traps::mshrink(0, blk as Long, 4096) }, 0);
    process::set_current(std::ptr::null_mut());
    // Whole tail regions lose the private copy without a descriptor
    // written; only the region the block still reaches into is rewritten,
    // and only its tail part.
    assert!(work_since(w) <= 512, "work {} for a shrink (stage 3b: {})", work_since(w), 2 * pages(BIG as u64));
    assert_eq!(access(a.slot, blk), RW, "the kept page");
    assert_eq!(access(a.slot, blk + 4096), None, "the tail is nobody's");
    assert_eq!(access(a.slot, blk + BIG as u64 - 4096), None);
}

#[test]
fn an_owner_change_touches_only_the_slots_it_concerns() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let _bystander = Proc::new();
    let blk = a.alloc(BIG, MX_TTRAM);
    let r = regions(blk, BIG as u64);
    let w = memprot::stats();
    // SAFETY: under the lock.
    unsafe { region::attach(blk as Long, b.pd()) }.unwrap();
    assert!(work_since(w) <= r * 512, "attach: only b's new copies ({} > {})", work_since(w), r * 512);
    assert_eq!(access(b.slot, blk), RW);
    assert_eq!(access(a.slot, blk), RW);
    let w = memprot::stats();
    // SAFETY: under the lock.
    assert_eq!(unsafe { region::detach(blk as Long, b.pd()) }, Ok(region::Detached::Kept));
    assert!(work_since(w) <= 2 * 512, "detach: b's copies go, a stays ({})", work_since(w));
    assert_eq!(access(b.slot, blk), None);
    assert_eq!(access(a.slot, blk), RW);
    // `set_owner` (EmuTOS `Pexec` 3 → 4 hand-over): old and new owner only.
    let w = memprot::stats();
    // SAFETY: under the lock.
    assert!(unsafe { traps::set_owner(blk as Long, b.pd()) });
    assert!(work_since(w) <= r * 512 + 512, "set_owner ({})", work_since(w));
    assert_eq!(access(b.slot, blk), RW);
    assert_eq!(access(a.slot, blk), None);
}

#[test]
fn a_batch_enters_the_net_change_once() {
    let _g = lock();
    let parent = Proc::new();
    let child = Proc::new();
    let used = mmu::private_l3_used();
    let w = memprot::stats();
    let blk;
    {
        // `Pexec(0)`: the TPA is allocated as the caller, then handed to the
        // child — the caller never needs copies for it.
        // SAFETY: under the lock.
        let _batch = unsafe { memprot::batch() };
        blk = parent.alloc(BIG, MX_TTRAM);
        // SAFETY: under the lock.
        unsafe { memprot::set_mode(blk, ProtMode::P) };
        assert_eq!(access(parent.slot, blk), None, "nothing entered inside the batch");
        // SAFETY: under the lock.
        assert!(unsafe { traps::set_owner(blk as Long, child.pd()) });
        assert_eq!(memprot::stats().syncs, w.syncs, "no sync inside the batch");
    }
    let r = regions(blk, BIG as u64);
    assert_eq!(memprot::stats().syncs, w.syncs + 1, "one sync at its end");
    assert!(work_since(w) <= r * 512, "only the child's copies ({})", work_since(w));
    assert_eq!(mmu::private_l3_used(), used + r as usize, "no copy for the caller");
    assert_eq!(access(child.slot, blk), RW);
    assert_eq!(access(parent.slot, blk), None);
}

#[test]
fn a_thread_joining_a_space_costs_only_the_new_holders() {
    let _g = lock();
    let parent = Proc::new();
    let thread = Proc::new();
    let _bystander = Proc::new();
    let big = parent.alloc(BIG, MX_TTRAM);
    let small = thread.alloc(0x1000, MX_TTRAM);
    let fallbacks = memprot::stats().fallbacks;
    let w = memprot::stats();
    // SAFETY: under the lock.
    unsafe { memprot::set_space(thread.slot, parent.pd().cast()) };
    let r = regions(big, BIG as u64);
    // The thread gets copies for the parent's block, the parent one region
    // for the thread's block.
    assert!(work_since(w) <= (r + 1) * 512, "join ({})", work_since(w));
    assert_eq!(access(thread.slot, big), RW, "the thread sees its caller's memory");
    assert_eq!(access(parent.slot, small), RW, "and the caller what the thread owns");
    assert_eq!(memprot::stats().fallbacks, fallbacks, "announced: no fallback");
    // SAFETY: under the lock.
    unsafe { memprot::set_space(thread.slot, std::ptr::null_mut()) };
    assert_eq!(access(thread.slot, big), None);
    assert_eq!(access(parent.slot, small), None);
}

#[test]
fn an_unannounced_space_change_falls_back_and_stays_correct() {
    let _g = lock();
    let parent = Proc::new();
    let child = Proc::new();
    let mine = parent.alloc(0x1000, MX_TTRAM);
    let fallbacks = memprot::stats().fallbacks;
    PROCESS_TABLE[child.slot].space_pd.store(parent.pd().cast(), Ordering::Release);
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(child.slot, parent.pd().cast()) };
    assert_eq!(memprot::stats().fallbacks, fallbacks + 1, "caught at the sync");
    assert_eq!(access(child.slot, mine), RW);
    PROCESS_TABLE[child.slot].space_pd.store(std::ptr::null_mut(), Ordering::Release);
    // SAFETY: under the lock.
    unsafe { memprot::sync_now() };
    assert_eq!(access(child.slot, mine), None);
}

#[test]
fn an_e10_grant_touches_only_the_owners_blocks() {
    let _g = lock();
    let owner = Proc::new();
    let host = Proc::new();
    let _big = host.alloc(BIG, MX_TTRAM);
    let tree = owner.alloc(0x1000, MX_TTRAM);
    let w = memprot::stats();
    // SAFETY: under the lock.
    unsafe { memprot::grant_temp(host.slot, owner.slot) };
    assert!(work_since(w) <= 512, "grant: the owner's one region ({})", work_since(w));
    assert_eq!(access(host.slot, tree), RW);
    // SAFETY: under the lock.
    unsafe { memprot::revoke_temp(host.slot) };
    assert!(work_since(w) <= 2 * 512, "revoke ({})", work_since(w));
    assert_eq!(access(host.slot, tree), None);
    assert_eq!(access(host.slot, _big), RW, "the host's own block untouched");
}

#[test]
fn children_inherit_cheaply_unless_strict() {
    let _g = lock();
    let parent = Proc::new();
    let blk = parent.alloc(BIG, MX_TTRAM);
    let child = Proc::new();
    let w = memprot::stats();
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(child.slot, parent.pd().cast()) };
    let r = regions(blk, BIG as u64);
    assert!(work_since(w) <= (r + 1) * 512, "E5: the child's copies only ({})", work_since(w));
    assert_eq!(access(child.slot, blk), RW);
    // SAFETY: under the lock; restored below.
    unsafe { memprot::configure(true, true) };
    let strict_child = Proc::new();
    let w = memprot::stats();
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(strict_child.slot, parent.pd().cast()) };
    // SAFETY: as above.
    unsafe { memprot::configure(true, false) };
    assert_eq!(work_since(w), 0, "strict: nothing inherited, nothing to do");
    assert_eq!(access(strict_child.slot, blk), None);
}

#[test]
fn readable_and_global_blocks_reach_everybody() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let r = a.alloc(0x3000, MX_TTRAM | 0x40);
    let g = a.alloc(0x3000, MX_TTRAM | 0x20);
    assert_eq!(access(b.slot, r), RO);
    assert_eq!(access(a.slot, r), RW);
    assert_eq!(access(b.slot, g + 0x2000), RW);
    // SAFETY: under the lock.
    unsafe {
        assert_eq!(traps::mfree(r as Long), 0);
        assert_eq!(traps::mfree(g as Long), 0);
    }
    assert_eq!(access(b.slot, r), None);
    assert_eq!(access(a.slot, r), None);
    assert_eq!(access(b.slot, g), None);
}

#[test]
fn freeing_a_large_block_writes_no_descriptor_of_its_whole_regions() {
    let _g = lock();
    let a = Proc::new();
    let blk = a.alloc(BIG, MX_TTRAM);
    let w = memprot::stats();
    // SAFETY: under the lock.
    assert_eq!(unsafe { traps::mfree(blk as Long) }, 0);
    // Regions the block filled alone drop the copy; only the edge regions
    // (shared with other blocks of `a` at most) are rewritten.
    assert!(work_since(w) <= 2 * 512, "free ({})", work_since(w));
    assert_eq!(access(a.slot, blk), None);
    assert_eq!(access(a.slot, blk + BIG as u64 - 4096), None);
}

#[test]
fn page_zero_is_untouched_by_rights_changes() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let p0a = mmu::translate(a.slot, 0);
    let p0b = mmu::translate(b.slot, 0);
    let blk = a.alloc(BIG, MX_TTRAM);
    // SAFETY: under the lock.
    unsafe {
        region::attach(blk as Long, b.pd()).unwrap();
        memprot::grant_temp(b.slot, a.slot);
        memprot::revoke_temp(b.slot);
        assert_eq!(traps::mfree(blk as Long), 0);
    }
    assert_eq!(mmu::translate(a.slot, 0), p0a);
    assert_eq!(mmu::translate(b.slot, 0), p0b);
}

#[test]
fn the_verifier_finds_a_wrong_descriptor() {
    let _g = lock();
    let a = Proc::new();
    let blk = a.alloc(0x1000, MX_TTRAM);
    let region = blk & !(mmu::REGION_SPAN - 1);
    let idx = ((blk - region) / 4096) as usize;
    // SAFETY: under the lock.
    let p = unsafe { mmu::slot_private_l3(a.slot, region) }.expect("a's copy");
    let verified = memprot::stats().verified;
    assert!(verified > 0, "the verification runs in this binary");
    memprot::set_verify(true, false);
    // SAFETY: under the lock; restored below.
    let good = unsafe { mmu::leaf(p, idx) };
    // SAFETY: as above.
    unsafe { mmu::set_leaf(p, idx, 0, blk, Some(a.slot)) };
    // SAFETY: under the lock.
    let found = unsafe { memprot::verify_now() };
    // SAFETY: as above.
    unsafe { mmu::set_leaf(p, idx, good, blk, Some(a.slot)) };
    // SAFETY: under the lock.
    let after = unsafe { memprot::verify_now() };
    let mut reported = None;
    while let Some(r) = memprot::take_verify_report() {
        if let memprot::VerifyReport::Mismatch(m) = r {
            reported.get_or_insert(m);
        }
    }
    memprot::set_verify(true, true);
    assert_eq!(found, 1, "the corrupted entry");
    assert_eq!(after, 0, "and nothing once it is restored");
    let m = reported.expect("reported");
    assert_eq!(m.kind, memprot::MismatchKind::Descriptor);
    assert_eq!(m.slot, Some(a.slot));
    assert_eq!(m.va, blk);
    assert_eq!(m.actual, 0);
    assert_eq!(m.expected, good);
}
