//! ARCH03 stage 3c — E10 (a): a `G_USERDEF` routine of another process runs
//! in the current one with the owner's rights for the callout
//! (`aes/object.rs` `ob_user` → `memprot::grant_temp` / `revoke_temp`;
//! FreeMiNT `prot_temp`, `sys/arch/mprot030.c:775-805`; EmuTOS runs the
//! routine wherever the draw happens, `aes/gemoblib.c:411-426`).
//!
//! The case that needs the grant: `memprot=strict`, a `Pexec(0)` child
//! compositing its parent's `WF_NEWDESK` background — the child inherited
//! nothing (E5 strict), so without the grant the parent's `ub_code`, tree and
//! `ub_parm` would fault. The grant covers the owner's whole memory space
//! (a `Pexec(4)` thread's blocks too) and ends with the callout.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::{free_all_owned, traps};
use tos_api::bdos::process::mmu;
use tos_api::bdos::process::{self, memprot, PROCESS_TABLE};
use tos_types::{BasePage, Long, MX_TTRAM};

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

    /// `Pterm`'s memory sweep for this process.
    fn terminate(&self) {
        // SAFETY: under the lock.
        unsafe { free_all_owned::free_all_owned(self.pd()) };
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        self.terminate();
        process::free_slot(self.slot);
        // SAFETY: allocated by `Box::new` in `new`.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

fn access(slot: usize, va: u64) -> Option<u64> {
    mmu::translate(slot, va).map(|(_, d)| (d >> 6) & 0b11)
}

const RW: Option<u64> = Some(0b01);

#[test]
fn strict_child_runs_the_parents_userdef_only_for_the_callout() {
    let _g = lock();
    // SAFETY: under the lock; this binary runs strict.
    unsafe { memprot::configure(true, true) };
    let parent = Proc::new();
    let code = parent.alloc(0x2000, MX_TTRAM); // its text + tree + ub_parm
    let child = Proc::new();
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(child.slot, parent.pd().cast()) };
    assert_eq!(access(child.slot, code), None, "strict: the child inherited nothing");

    // ob_user: foreign owner -> grant, callout, revoke.
    // SAFETY: under the lock.
    unsafe { memprot::grant_temp(child.slot, parent.slot) };
    assert_eq!(access(child.slot, code), RW, "the owner's routine is reachable for the callout");
    assert_eq!(access(child.slot, code + 0x1000), RW);
    // SAFETY: under the lock.
    unsafe { memprot::revoke_temp(child.slot) };
    assert_eq!(access(child.slot, code), None, "and not after it");
    // The parent's own rights never changed.
    assert_eq!(access(parent.slot, code), RW);
}

#[test]
fn the_grant_covers_the_owners_whole_space() {
    let _g = lock();
    // SAFETY: under the lock; this binary runs strict.
    unsafe { memprot::configure(true, true) };
    let owner = Proc::new();
    let thread = Proc::new();
    // A `Pexec(4)` thread of the owner (E6): its blocks are the space's.
    PROCESS_TABLE[thread.slot].space_pd.store(owner.pd().cast(), Ordering::Release);
    // SAFETY: under the lock.
    unsafe { memprot::on_launch(thread.slot, owner.pd().cast()) };
    let by_thread = thread.alloc(0x1000, MX_TTRAM);
    let host = Proc::new();
    assert_eq!(access(host.slot, by_thread), None);
    // SAFETY: under the lock.
    unsafe { memprot::grant_temp(host.slot, owner.slot) };
    assert_eq!(access(host.slot, by_thread), RW);
    // SAFETY: under the lock.
    unsafe { memprot::revoke_temp(host.slot) };
    assert_eq!(access(host.slot, by_thread), None);
    PROCESS_TABLE[thread.slot].space_pd.store(std::ptr::null_mut(), Ordering::Release);
}
