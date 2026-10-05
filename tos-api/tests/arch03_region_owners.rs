//! ARCH03 stage 2d (plan Frage 6 / §3.4, decision E15): a block can have
//! several owners, and only an owner frees it.
//!
//! Sources: FreeMiNT `sys/memory.c:512-660` (`attach_region` increments a
//! region's `links`, `detach_region` decrements it and frees the region at
//! 0), `sys/dosmem.c:330-455` (`Mfree`/`Mshrink` find only the caller's
//! regions, `EFAULT` = −40 otherwise), `sys/k_exec.c:518-520,586-590` (the
//! started child attaches to basepage and environment; mode 6 detaches the
//! parent), `sys/k_exit.c:307-336` (a TSR keeps its regions). EmuTOS
//! `bdos/proc.c:92-101` (`free_all_owned` by `m_own`) is the one-owner case.
//! Spec: `docs/api/gemdos/memory/49-mfree.md` § "several owners".
//!
//! The pools are bookkeeping only (addresses never dereferenced); a process
//! that calls GEMDOS is a host basepage in a claimed slot, as in
//! `arch03_shared_space.rs`.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::free_all_owned::{free_all_owned, reserve_all_owned};
use tos_api::bdos::mem::region::{self, Detached, RegionError, MAX_FURTHER_OWNERS};
use tos_api::bdos::mem::{init, traps};
use tos_api::bdos::process::{self, PROCESS_TABLE};
use tos_api::trap::TrapArgs;
use tos_types::error::{EIMBA, E_OK};
use tos_types::{BasePage, Long, MD, MX_STRAM, MX_TTRAM};

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any allocation.
        unsafe { init::init(0x0040_0000, 10 << 20, 0x0100_0000, 64 << 20) }.expect("pools");
    });
    g
}

type Who = *mut core::ffi::c_void;
const A: Who = 0x7a00_0000usize as Who;
const B: Who = 0x7b00_0000usize as Who;
const C: Who = 0x7c00_0000usize as Who;

fn alloc(owner: Who) -> Long {
    // SAFETY: under the lock.
    unsafe {
        let a = traps::mxalloc(256, MX_STRAM);
        assert_ne!(a, 0);
        traps::set_owner(a, owner);
        a
    }
}

fn m_own(addr: Long) -> Option<Who> {
    for mp in [init::pmd(), init::pmdalt()] {
        // SAFETY: under the lock; read-only walk.
        let mut m: *mut MD = unsafe { (*mp).mp_mal };
        while !m.is_null() {
            // SAFETY: list invariant.
            unsafe {
                if (*m).m_start == addr {
                    return Some((*m).m_own);
                }
                m = (*m).m_link;
            }
        }
    }
    None
}

#[test]
fn one_owner_frees_like_emutos_and_a_stranger_cannot() {
    let _g = lock();
    let x = alloc(A);
    // SAFETY: under the lock (all `region` calls below).
    unsafe {
        assert_eq!(region::links(x), 1);
        assert!(region::is_owner(x, A) && !region::is_owner(x, B));
        assert_eq!(region::detach(x, B), Err(RegionError::NotOwner));
        assert_eq!(m_own(x), Some(A), "untouched");
        assert_eq!(region::detach(x, A), Ok(Detached::Freed));
        assert_eq!(m_own(x), None, "back in the pool");
        assert_eq!(region::detach(x, A), Err(RegionError::NoBlock));
    }
}

#[test]
fn a_second_owner_keeps_the_block_alive_and_takes_the_tag() {
    let _g = lock();
    let x = alloc(A);
    // SAFETY: under the lock.
    unsafe {
        assert_eq!(region::attach(x, B), Ok(()));
        assert_eq!(region::attach(x, B), Ok(()), "attaching twice is one link");
        assert_eq!(region::attach(x, A), Ok(()), "the tag holder is an owner already");
        assert_eq!(region::links(x), 2);
        // The tag holder leaves: B takes the EmuTOS tag, nothing is freed.
        assert_eq!(region::detach(x, A), Ok(Detached::Kept));
        assert_eq!(m_own(x), Some(B));
        assert_eq!(region::links(x), 1);
        assert!(!region::is_owner(x, A));
        assert_eq!(region::detach(x, B), Ok(Detached::Freed));
    }
}

#[test]
fn a_further_owner_leaving_frees_nothing() {
    let _g = lock();
    let x = alloc(A);
    // SAFETY: under the lock.
    unsafe {
        region::attach(x, B).unwrap();
        assert_eq!(region::detach(x, B), Ok(Detached::Kept));
        assert_eq!((m_own(x), region::links(x)), (Some(A), 1));
        assert_eq!(region::detach(x, A), Ok(Detached::Freed));
    }
}

#[test]
fn hand_over_is_pexec_6() {
    let _g = lock();
    let x = alloc(A);
    // SAFETY: under the lock.
    unsafe {
        region::hand_over(x, A, B).unwrap();
        assert_eq!((m_own(x), region::links(x)), (Some(B), 1), "set_owner(p, p)");
        // From someone who did not own it: the child only gains it.
        region::hand_over(x, C, A).unwrap();
        assert_eq!(region::links(x), 2);
        region::detach(x, A).unwrap();
        assert_eq!(region::detach(x, B), Ok(Detached::Freed));
    }
}

#[test]
fn pterm_drops_the_process_everywhere_and_frees_only_its_own() {
    let _g = lock();
    let parents = alloc(A); // the child's TPA, say: the parent owns it
    let childs = alloc(B); // the child's Malloc
    // SAFETY: under the lock.
    unsafe {
        region::attach(parents, B).unwrap(); // Pexec 4 attached the child
        assert_eq!(free_all_owned(B), 1, "only its own block is freed");
        assert_eq!(m_own(childs), None);
        assert_eq!((m_own(parents), region::links(parents)), (Some(A), 1), "the parent keeps its");
        // And when the child held the tag of a shared block, the tag moves.
        let shared = alloc(B);
        region::attach(shared, A).unwrap();
        assert_eq!(free_all_owned(B), 0);
        assert_eq!((m_own(shared), region::links(shared)), (Some(A), 1));
        assert_eq!(free_all_owned(A), 2);
        assert_eq!(m_own(parents), None);
    }
}

#[test]
fn ptermres_hands_every_link_to_the_tsr_holder() {
    let _g = lock();
    let own = alloc(A);
    let shared = alloc(C);
    // SAFETY: under the lock.
    unsafe {
        region::attach(shared, A).unwrap();
        assert_eq!(reserve_all_owned(A), 1);
        let tsr = traps::tsr_owner();
        assert_eq!(m_own(own), Some(tsr));
        assert!(region::is_owner(shared, tsr), "the TSR keeps its link to a shared block");
        assert_eq!(region::links(shared), 2);
        assert!(!region::is_owner(own, A) && !region::is_owner(shared, A));
        // Nothing of A is left to free.
        assert_eq!(free_all_owned(A), 0);
        // C lets go: the TSR holder keeps the shared block alive.
        assert_eq!(region::detach(shared, C), Ok(Detached::Kept));
        assert_eq!(m_own(shared), Some(tsr));
        traps::mfree(own);
        traps::mfree(shared);
    }
}

#[test]
fn a_freed_block_forgets_its_owners_and_a_new_one_starts_alone() {
    let _g = lock();
    let x = alloc(A);
    // SAFETY: under the lock.
    unsafe {
        region::attach(x, B).unwrap();
        // A kernel-internal free (no owner check) forgets the further owner.
        assert_eq!(traps::mfree(x), E_OK);
        let y = traps::mxalloc(256, MX_STRAM);
        assert_eq!(y, x, "first fit hands out the same address again");
        assert_eq!(region::links(y), 1);
        assert!(!region::is_owner(y, B));
        traps::mfree(y);
    }
}

#[test]
fn owners_per_block_are_bounded() {
    let _g = lock();
    let x = alloc(A);
    // SAFETY: under the lock.
    unsafe {
        for i in 0..MAX_FURTHER_OWNERS {
            region::attach(x, (0x7100_0000 + i * 0x1000) as Who).unwrap();
        }
        assert_eq!(region::attach(x, C), Err(RegionError::Full));
        assert_eq!(region::links(x), 1 + MAX_FURTHER_OWNERS);
        traps::mfree(x);
    }
}

// ── at the GEMDOS interface ────────────────────────────────────────────

struct Proc {
    slot: usize,
    bp: *mut BasePage,
}
impl Proc {
    fn new() -> Proc {
        let slot = process::allocate_slot().expect("a slot");
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        PROCESS_TABLE[slot].basepage_kva.store(bp, Ordering::Release);
        Proc { slot, bp }
    }
    fn run(&self) {
        process::set_current(process::pd_of(self.slot));
    }
    fn pd(&self) -> Who {
        process::pd_of(self.slot).cast()
    }
}
impl Drop for Proc {
    fn drop(&mut self) {
        process::set_current(std::ptr::null_mut());
        process::free_slot(self.slot);
        // SAFETY: the slot no longer names the box.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

fn gemdos(func: u64, a: [u64; 3]) -> Long {
    let args = TrapArgs { regs: [a[0], a[1], a[2], 0, 0, 0, 0, 0], func, svc_imm: 1, space: tos_api::uaccess::Space::Kernel };
    match func {
        0x44 => tos_api::gemdos::memory::mxalloc(&args),
        0x49 => tos_api::gemdos::memory::mfree(&args),
        0x4A => tos_api::gemdos::memory::mshrink(&args),
        _ => unreachable!(),
    }
}

#[test]
fn mfree_and_mshrink_only_for_an_owner() {
    let _g = lock();
    let p = Proc::new();
    let q = Proc::new();
    p.run();
    let blk = gemdos(0x44, [4096, MX_TTRAM as u64, 0]);
    assert_eq!(m_own(blk), Some(p.pd()));
    q.run();
    assert_eq!(gemdos(0x49, [blk as u64, 0, 0]), EIMBA, "Mfree of another's block (E15)");
    assert_eq!(gemdos(0x4A, [0, blk as u64, 64]), EIMBA, "Mshrink of another's block");
    assert_eq!(gemdos(0x4A, [0, blk as u64, 0]), EIMBA, "Mshrink(0) follows Mfree");
    assert_eq!(m_own(blk), Some(p.pd()), "untouched");
    // A shared block: q attached (a Pexec 4 child) — q's Mfree drops q only.
    // SAFETY: under the lock.
    unsafe { region::attach(blk, q.pd()).unwrap() };
    assert_eq!(gemdos(0x4A, [0, blk as u64, 2048]), E_OK, "an owner shrinks");
    assert_eq!(gemdos(0x49, [blk as u64, 0, 0]), E_OK);
    assert_eq!(m_own(blk), Some(p.pd()), "still p's");
    assert_eq!(gemdos(0x49, [blk as u64, 0, 0]), EIMBA, "q is no owner any more");
    p.run();
    assert_eq!(gemdos(0x49, [blk as u64, 0, 0]), E_OK);
    assert_eq!(m_own(blk), None, "freed with its last owner");
    assert_eq!(gemdos(0x49, [blk as u64, 0, 0]), EIMBA, "double free");
}

#[test]
fn a_pexec_4_child_owns_what_its_caller_owns() {
    let _g = lock();
    let p = Proc::new();
    let t = Proc::new();
    p.run();
    let blk = gemdos(0x44, [512, MX_STRAM as u64, 0]);
    // t runs in p's memory space (plan E6, FreeMiNT FORK_SHAREVM).
    PROCESS_TABLE[t.slot].space_pd.store(p.pd().cast(), Ordering::Release);
    t.run();
    assert_eq!(gemdos(0x4A, [0, blk as u64, 256]), E_OK, "the thread may shrink it");
    assert_eq!(gemdos(0x49, [blk as u64, 0, 0]), E_OK, "and free it");
    assert_eq!(m_own(blk), None);
    // Without the shared space it may not.
    p.run();
    let blk2 = gemdos(0x44, [512, MX_STRAM as u64, 0]);
    PROCESS_TABLE[t.slot].space_pd.store(std::ptr::null_mut(), Ordering::Release);
    t.run();
    assert_eq!(gemdos(0x49, [blk2 as u64, 0, 0]), EIMBA);
    p.run();
    assert_eq!(gemdos(0x49, [blk2 as u64, 0, 0]), E_OK);
}

#[test]
fn resident_memory_is_nobodys_to_free() {
    let _g = lock();
    let p = Proc::new();
    p.run();
    let blk = gemdos(0x44, [128, MX_STRAM as u64, 0]);
    // SAFETY: under the lock.
    unsafe { reserve_all_owned(p.pd()) };
    assert_eq!(gemdos(0x49, [blk as u64, 0, 0]), EIMBA, "a TSR's memory (EmuTOS: on no list)");
    // SAFETY: under the lock; clean up for the other tests.
    unsafe { traps::mfree(blk) };
}
