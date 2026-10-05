//! ARCH03 stage 3b — **rights per address space** (FreeMiNT `PROT_P/G/S/PR/I`,
//! plan `docs/analysis/arch03-implementation-plan.md` §3.2–§3.4, invariants
//! I7–I10, decisions E5, E6, E10, E15, E16).
//!
//! Checked on the host, against the real pool, ownership table and page
//! tables, with memory protection switched on for this test binary:
//!
//! * the rights matrix (table §3.3): owner / non-owner descriptor bits for
//!   every mode, free memory invalid for everybody (I8 — MEM04);
//! * table sharing: a process whose rights equal everybody else's in a
//!   2-MiB region uses the shared L3; one whose rights differ gets its own
//!   copy, which goes away when the difference does (copy-on-differ);
//! * ownership transitions: `Pexec(4)` thread shares its caller's space
//!   (E6), a `Pexec(0)` child inherits its parent's blocks unless strict
//!   (E5), attach/detach of a further owner, `Ptermres` turns sole-owner
//!   private memory global, temporary rights (E10), slot release;
//! * the page grain (E16) and the crash-report classification.
//!
//! The pool is bookkeeping only (its addresses are never dereferenced); the
//! basepages are host memory; the table arena is a leaked, page-aligned host
//! buffer. One binary, one pool, one lock.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::{region, traps};
use tos_api::bdos::process::mmu::{self, PageFlags};
use tos_api::bdos::process::{self, memprot, PROCESS_TABLE};
use tos_api::bios::crash::RegionKind;
use tos_types::{BasePage, Long, ProtMode, MX_STRAM, MX_TTRAM};

const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024;
const ARENA_TABLES: usize = 512;

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
            mmu::set_table_arena(arena.as_mut_ptr() as u64, ARENA_TABLES);
            tos_api::bdos::mem::init::init(ST_START, ST_LEN, TT_START, TT_LEN).expect("pools");
            mmu::map_pools().expect("the pools map");
        }
    });
    g
}

/// A process the host can "run": a basepage of its own (host memory) in a
/// claimed slot with a root.
struct Proc {
    slot: usize,
    bp: *mut BasePage,
}

impl Proc {
    fn new() -> Proc {
        let slot = process::allocate_slot().expect("a slot");
        let b = Box::new([0u8; 256]);
        let bp = Box::into_raw(b) as *mut BasePage;
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

    /// Allocate as this process (it is `RUN` for the call).
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
        unsafe { tos_api::bdos::mem::free_all_owned::free_all_owned(self.pd()) };
        process::free_slot(self.slot);
        // SAFETY: allocated by `Box::new` in `new`.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

/// What `slot` may do at `va`: `None` = invalid, else the leaf's AP bits
/// (`0b01` RW, `0b11` RO).
fn access(slot: usize, va: u64) -> Option<u64> {
    mmu::translate(slot, va).map(|(pa, d)| {
        assert_eq!(pa, va, "VA = PA");
        (d >> 6) & 0b11
    })
}

const RW: Option<u64> = Some(0b01);
const RO: Option<u64> = Some(0b11);

#[test]
fn rights_matrix_owner_and_others_for_every_mode() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    // Mxalloc mode nibble = PROT value + 1 (FreeMiNT `sys/dosmem.c:209-221`).
    for (nibble, mode, owner, other) in [
        (0x10, ProtMode::P, RW, None),
        (0x20, ProtMode::G, RW, RW),
        (0x30, ProtMode::S, RW, None),
        (0x40, ProtMode::Pr, RW, RO),
        (0x50, ProtMode::I, None, None),
    ] {
        let blk = a.alloc(0x2000, MX_TTRAM | nibble);
        // SAFETY: under the lock.
        assert_eq!(unsafe { memprot::mode_of(blk) }, mode);
        for va in [blk, blk + 0x1FFF] {
            assert_eq!(access(a.slot, va), owner, "owner, {mode:?}");
            assert_eq!(access(b.slot, va), other, "other, {mode:?}");
        }
        // PXN on everything; owners may execute (UXN clear) as in stage 1.
        if let Some((_, d)) = mmu::translate(a.slot, blk) {
            assert_ne!(d & (1 << 53), 0, "PXN");
            assert_eq!(d & (1 << 54), 0, "UXN clear");
        }
        // SAFETY: under the lock.
        assert_eq!(unsafe { traps::mfree(blk as Long) }, 0);
    }
}

#[test]
fn malloc_takes_the_prgflags_mode_and_free_memory_is_nobodys() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    // PRGFLAGS nibble 0 = PROT_P (PF_STANDARD).
    let blk = a.alloc(100, MX_STRAM);
    assert_eq!(access(a.slot, blk), RW);
    assert_eq!(access(b.slot, blk), None);
    // MEM04 / I8: after Mfree the former owner can no longer touch it.
    // SAFETY: under the lock.
    assert_eq!(unsafe { traps::mfree(blk as Long) }, 0);
    assert_eq!(access(a.slot, blk), None, "freed memory is invalid for its former owner");
    assert_eq!(access(b.slot, blk), None);
    // The same pages handed to b: b only.
    let again = b.alloc(100, MX_STRAM);
    assert_eq!(again, blk, "first fit reuses the pages");
    assert_eq!(access(b.slot, again), RW);
    assert_eq!(access(a.slot, again), None);
}

#[test]
fn page_grain_with_protection_on() {
    let _g = lock();
    let a = Proc::new();
    let x = a.alloc(10, MX_TTRAM);
    let y = a.alloc(10, MX_TTRAM);
    assert_eq!(x % 4096, 0);
    assert_eq!(y % 4096, 0);
    assert!(y.abs_diff(x) >= 4096, "no page has two blocks (I10)");
    // Mshrink keeps a page grain too: shrinking 3 pages to 1 byte keeps one
    // page and frees the rest, which becomes invalid again.
    let z = a.alloc(3 * 4096, MX_TTRAM);
    process::set_current(process::pd_of(a.slot));
    // SAFETY: under the lock.
    assert_eq!(unsafe { traps::mshrink(0, z as Long, 1) }, 0);
    process::set_current(std::ptr::null_mut());
    assert_eq!(access(a.slot, z), RW);
    assert_eq!(access(a.slot, z + 4096), None, "the shrunk tail is nobody's");
}

#[test]
fn equal_rights_share_the_table_and_different_rights_copy_it() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let region = |va: u64| va & !(mmu::REGION_SPAN - 1);
    // A global block: everybody sees the same → nobody needs a copy.
    let g = a.alloc(0x1000, MX_TTRAM | 0x20);
    // SAFETY: under the lock.
    unsafe {
        assert!(mmu::slot_private_l3(a.slot, region(g)).is_none());
        assert!(mmu::slot_private_l3(b.slot, region(g)).is_none());
    }
    // A private block in the same region: its owner gets a copy, b does not.
    let p = a.alloc(0x1000, MX_TTRAM);
    assert_eq!(region(p), region(g));
    // SAFETY: under the lock.
    unsafe {
        assert!(mmu::slot_private_l3(a.slot, region(p)).is_some());
        assert!(mmu::slot_private_l3(b.slot, region(p)).is_none());
    }
    let used = mmu::private_l3_used();
    // Freed: the difference is gone, so is the copy.
    // SAFETY: under the lock.
    unsafe {
        assert_eq!(traps::mfree(p as Long), 0);
        assert!(mmu::slot_private_l3(a.slot, region(p)).is_none());
    }
    assert_eq!(mmu::private_l3_used(), used - 1);
    // SAFETY: under the lock.
    unsafe { assert_eq!(traps::mfree(g as Long), 0) };
}

#[test]
fn a_pexec4_thread_shares_its_callers_space() {
    let _g = lock();
    let parent = Proc::new();
    let child = Proc::new();
    let mine = parent.alloc(0x1000, MX_TTRAM);
    assert_eq!(access(child.slot, mine), None);
    // `Pexec(4)`: the child runs in its caller's memory space (E6).
    PROCESS_TABLE[child.slot].space_pd.store(parent.pd().cast(), Ordering::Release);
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(child.slot, parent.pd().cast()) };
    assert_eq!(access(child.slot, mine), RW, "the thread sees its caller's memory");
    // And the caller sees what the thread allocates (LDG).
    let theirs = child.alloc(0x1000, MX_TTRAM);
    assert_eq!(access(parent.slot, theirs), RW);
    PROCESS_TABLE[child.slot].space_pd.store(std::ptr::null_mut(), Ordering::Release);
}

#[test]
fn a_child_inherits_its_parents_blocks_unless_strict() {
    let _g = lock();
    let parent = Proc::new();
    let before = parent.alloc(0x1000, MX_TTRAM);
    let child = Proc::new();
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(child.slot, parent.pd().cast()) };
    assert_eq!(access(child.slot, before), RW, "E5: what the parent had at the start");
    let after = parent.alloc(0x1000, MX_TTRAM);
    assert_eq!(access(child.slot, after), None, "not what it allocates later");
    // Strict mode: nothing is inherited.
    // SAFETY: under the lock; restored below.
    unsafe { memprot::configure(true, true) };
    let strict_child = Proc::new();
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(strict_child.slot, parent.pd().cast()) };
    assert_eq!(access(strict_child.slot, before), None, "E5 strict");
    // SAFETY: as above.
    unsafe { memprot::configure(true, false) };
    // The inherited right ends with the block.
    // SAFETY: under the lock.
    unsafe { assert_eq!(traps::mfree(before as Long), 0) };
    assert_eq!(access(child.slot, before), None);
}

#[test]
fn further_owners_get_and_lose_rights() {
    let _g = lock();
    let a = Proc::new();
    let b = Proc::new();
    let blk = a.alloc(0x1000, MX_TTRAM);
    // SAFETY: under the lock.
    unsafe { region::attach(blk as Long, b.pd()) }.unwrap();
    assert_eq!(access(b.slot, blk), RW, "attached: an owner");
    // SAFETY: under the lock.
    assert_eq!(unsafe { region::detach(blk as Long, b.pd()) }, Ok(region::Detached::Kept));
    assert_eq!(access(b.slot, blk), None, "detached: not any more");
    assert_eq!(access(a.slot, blk), RW);
    // `Pexec(6)` hand-over: b becomes the owner, a stops being one.
    // SAFETY: under the lock.
    unsafe { region::hand_over(blk as Long, a.pd(), b.pd()) }.unwrap();
    assert_eq!(access(b.slot, blk), RW);
    assert_eq!(access(a.slot, blk), None);
}

#[test]
fn resident_private_memory_becomes_global() {
    let _g = lock();
    let tsr = Proc::new();
    let other = Proc::new();
    let code = tsr.alloc(0x1000, MX_TTRAM);
    let ro = tsr.alloc(0x1000, MX_TTRAM | 0x40);
    // SAFETY: under the lock.
    unsafe { tos_api::bdos::mem::free_all_owned::reserve_all_owned(tsr.pd()) };
    // FreeMiNT `sys_ptermres`: sole-owner PROT_P → PROT_G; PROT_PR stays.
    // SAFETY: under the lock.
    unsafe {
        assert_eq!(memprot::mode_of(code), ProtMode::G);
        assert_eq!(memprot::mode_of(ro), ProtMode::Pr);
    }
    assert_eq!(access(other.slot, code), RW, "resident code is callable by everybody");
    assert_eq!(access(other.slot, ro), RO);
    // SAFETY: under the lock.
    unsafe {
        assert_eq!(traps::mfree(code as Long), 0);
        assert_eq!(traps::mfree(ro as Long), 0);
    }
}

#[test]
fn temporary_rights_for_a_foreign_callout() {
    let _g = lock();
    let owner = Proc::new();
    let host = Proc::new();
    let tree = owner.alloc(0x1000, MX_TTRAM);
    assert_eq!(access(host.slot, tree), None);
    // SAFETY: under the lock.
    unsafe { memprot::grant_temp(host.slot, owner.slot) };
    assert_eq!(access(host.slot, tree), RW, "E10: during the callout");
    // SAFETY: under the lock.
    unsafe { memprot::revoke_temp(host.slot) };
    assert_eq!(access(host.slot, tree), None, "and not after it");
}

#[test]
fn a_released_slot_gives_its_tables_back() {
    let _g = lock();
    let free_before = mmu::arena_free_count();
    let a = Proc::new();
    let _x = a.alloc(0x1000, MX_TTRAM);
    let _y = a.alloc(0x1000, MX_STRAM);
    assert!(mmu::arena_free_count() < free_before);
    drop(a);
    assert_eq!(mmu::arena_free_count(), free_before, "every private table back");
}

#[test]
fn the_kernel_allocates_supervisor_memory() {
    let _g = lock();
    let a = Proc::new();
    process::set_current(std::ptr::null_mut());
    // SAFETY: under the lock; no running process = a kernel caller.
    let k = unsafe { traps::mxalloc(0x1000, MX_TTRAM) } as u32 as u64;
    assert_ne!(k, 0);
    // SAFETY: under the lock.
    assert_eq!(unsafe { memprot::mode_of(k) }, ProtMode::S);
    assert_eq!(access(a.slot, k), None, "PROT_S: invalid for programs");
    // SAFETY: under the lock.
    unsafe { assert_eq!(traps::mfree(k as Long), 0) };
}

#[test]
fn crash_classification_names_what_was_hit() {
    let _g = lock();
    let a = Proc::new();
    let p = a.alloc(0x1000, MX_TTRAM);
    let s = a.alloc(0x1000, MX_TTRAM | 0x30);
    let r = a.alloc(0x1000, MX_TTRAM | 0x40);
    let g = a.alloc(0x1000, MX_TTRAM | 0x20);
    assert_eq!(memprot::region_kind(p + 8), Some(RegionKind::Private));
    assert_eq!(memprot::region_kind(s), Some(RegionKind::Super));
    assert_eq!(memprot::region_kind(r), Some(RegionKind::Readable));
    assert_eq!(memprot::region_kind(g), Some(RegionKind::Hardware), "global: not a protection fault");
    assert_eq!(memprot::region_kind(0xDEAD_0000), Some(RegionKind::Hardware), "outside the pools");
    // SAFETY: under the lock.
    unsafe { assert_eq!(traps::mfree(p as Long), 0) };
    assert_eq!(memprot::region_kind(p), Some(RegionKind::Free));
}

#[test]
fn publications_reach_private_copies() {
    let _g = lock();
    let a = Proc::new();
    // A private block at the very end of the TT pool's last region forces a
    // private copy of a region that also holds non-pool pages above it.
    let blk = a.alloc(0x1000, MX_TTRAM);
    let region = blk & !(mmu::REGION_SPAN - 1);
    // SAFETY: under the lock.
    assert!(unsafe { mmu::slot_private_l3(a.slot, region) }.is_some());
    // Publish a kernel page below the pools into a region nobody has a copy
    // of, and check the owner still sees its block and the publication.
    let low = 0x0009_3000u64;
    // SAFETY: under the lock; nothing is dereferenced.
    unsafe { mmu::map_shared(low, 0x1000, PageFlags::UserRoNx, false) }.unwrap();
    assert_eq!(access(a.slot, low), RO);
    assert_eq!(access(a.slot, blk), RW);
}
