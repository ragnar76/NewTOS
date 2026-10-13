//! MEM05 — allocated blocks per owner cookie and the per-process limit
//! (design `docs/analysis/mem05-design-2026-10-13.md` §3, tests 5 and 6;
//! user decisions E3/E5, `TASK/MEM05.md`).
//!
//! * Every change of an allocated block's EmuTOS tag `m_own` is booked
//!   (`owner_limit::rebook`); a recount over the allocated lists
//!   (`owner_limit::audit`) agrees after every step — `Mxalloc`, `Mshrink`,
//!   `Mfree`, `set_owner` (Pexec), `detach` (tag passed on), `F_KEEP`
//!   inheritance, `Ptermres`, `Pterm`.
//! * With the limit on, a process that holds `limit` blocks gets `0` from
//!   `Malloc`/`Mxalloc` (EmuTOS' answer when its MD pool is exhausted,
//!   `bdos/umem.c:324-325`); `Malloc(-1)` still reports the largest free
//!   block (E5); another process and the kernel are not affected; `Mfree`
//!   makes room again; `Mshrink` never fails at the limit.
//!
//! The pools are bookkeeping only (addresses never dereferenced); memory
//! protection is off (16-byte grain). A process that calls GEMDOS is a host
//! basepage in a claimed slot, as in `arch03_region_owners.rs`.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::free_all_owned::{free_all_owned, reserve_all_owned};
use tos_api::bdos::mem::owner_limit::{self, DEFAULT_OWNER_MD_LIMIT};
use tos_api::bdos::mem::{init, region, traps};
use tos_api::bdos::process::{self, PROCESS_TABLE};
use tos_api::trap::TrapArgs;
use tos_types::error::E_OK;
use tos_types::{BasePage, Long, MD, F_KEEP, MX_PREFTTRAM, MX_STRAM, MX_TTRAM};

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any allocation.
        unsafe { init::init(0x0040_0000, 10 << 20, 0x0100_0000, 64 << 20) }.expect("pools");
    });
    // Every test starts with the default limit.
    owner_limit::set_limit(DEFAULT_OWNER_MD_LIMIT);
    g
}

type Who = *mut core::ffi::c_void;

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

fn kernel() {
    process::set_current(std::ptr::null_mut());
}

fn gemdos(func: u64, a: [u64; 3]) -> Long {
    let args = TrapArgs { regs: [a[0], a[1], a[2], 0, 0, 0, 0, 0], func, svc_imm: 1, space: tos_api::uaccess::Space::Kernel };
    match func {
        0x44 => tos_api::gemdos::memory::mxalloc(&args),
        0x48 => tos_api::gemdos::memory::malloc(&args),
        0x49 => tos_api::gemdos::memory::mfree(&args),
        0x4A => tos_api::gemdos::memory::mshrink(&args),
        _ => unreachable!(),
    }
}

fn malloc(n: i64) -> Long {
    gemdos(0x48, [n as u64, 0, 0])
}

fn mxalloc(n: i64, mode: i16) -> Long {
    gemdos(0x44, [n as u64, mode as u16 as u64, 0])
}

fn mfree(a: Long) -> Long {
    gemdos(0x49, [a as u32 as u64, 0, 0])
}

fn count(who: Who) -> u32 {
    // SAFETY: under the lock.
    unsafe { owner_limit::count_of(who) }
}

#[track_caller]
fn audit() -> usize {
    // SAFETY: under the lock.
    match unsafe { owner_limit::audit() } {
        Ok(n) => n,
        Err((who, booked, actual)) => panic!("owner count of {who:?}: booked {booked}, list has {actual}"),
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

// ── test 6: bookings ─────────────────────────────────────────────────────

#[test]
fn every_tag_change_is_booked_and_the_recount_agrees() {
    let _g = lock();
    let p = Proc::new();
    let q = Proc::new();
    p.run();
    let a = malloc(1000);
    let b = mxalloc(5000, MX_STRAM);
    let c = mxalloc(64, MX_TTRAM);
    assert!(a != 0 && b != 0 && c != 0);
    assert_eq!(count(p.pd()), 3);
    audit();
    // Mshrink keeps the count (the tail goes to the free list).
    assert_eq!(gemdos(0x4A, [0, b as u64, 100]), E_OK);
    assert_eq!(count(p.pd()), 3);
    audit();
    // Mfree.
    assert_eq!(mfree(a), E_OK);
    assert_eq!(count(p.pd()), 2);
    audit();
    // set_owner (Pexec hands a block to the child): re-booked.
    // SAFETY: under the lock.
    assert!(unsafe { traps::set_owner(b, q.pd()) });
    assert_eq!((count(p.pd()), count(q.pd())), (1, 1));
    audit();
    // detach of the tag holder with a further owner: the tag moves on.
    // SAFETY: under the lock.
    unsafe {
        region::attach(c, q.pd()).unwrap();
        assert_eq!(region::detach(c, p.pd()), Ok(region::Detached::Kept));
    }
    assert_eq!(m_own(c), Some(q.pd()));
    assert_eq!((count(p.pd()), count(q.pd())), (0, 2));
    // SAFETY: under the lock.
    assert_eq!(unsafe { owner_limit::tracked_owners() } >= 1, true);
    audit();
    // Pterm of q frees both.
    // SAFETY: under the lock.
    assert_eq!(unsafe { free_all_owned(q.pd()) }, 2);
    assert_eq!(count(q.pd()), 0);
    audit();
    kernel();
}

#[test]
fn f_keep_and_ptermres_hand_the_count_to_unlimited_holders() {
    let _g = lock();
    let p = Proc::new();
    p.run();
    let kept = mxalloc(256, F_KEEP | MX_TTRAM);
    let tsr = mxalloc(256, MX_TTRAM);
    assert!(kept != 0 && tsr != 0);
    assert_eq!(count(p.pd()), 2);
    // SAFETY: under the lock.
    unsafe {
        // Ptermres: every block stays resident, F_KEEP or not.
        assert_eq!(reserve_all_owned(p.pd()), 2);
    }
    assert_eq!(m_own(tsr), Some(traps::tsr_owner()));
    assert_eq!(count(p.pd()), 0, "Ptermres transfers the count to the TSR holder");
    assert_eq!(count(traps::tsr_owner()), 0, "the TSR holder is not counted");
    audit();
    kernel();
    // SAFETY: under the lock; clean up.
    unsafe {
        traps::mfree(kept);
        traps::mfree(tsr);
    }
    audit();

    let p2 = Proc::new();
    p2.run();
    let kept = mxalloc(256, F_KEEP | MX_TTRAM);
    assert_eq!(count(p2.pd()), 1);
    // SAFETY: under the lock.
    assert_eq!(unsafe { free_all_owned(p2.pd()) }, 0, "inherited, not freed");
    assert_eq!(m_own(kept), Some(traps::kernel_owner()));
    assert_eq!(count(p2.pd()), 0);
    audit();
    kernel();
    // SAFETY: under the lock.
    assert!(unsafe { region::release_kept(kept) });
    audit();
}

#[test]
fn aligned_and_largest_allocations_count_one_block() {
    let _g = lock();
    let p = Proc::new();
    p.run();
    // SAFETY: under the lock.
    let a = unsafe { traps::mxalloc_aligned(3000, MX_STRAM, 4096) };
    assert_ne!(a, 0);
    assert_eq!(count(p.pd()), 1);
    audit();
    // SAFETY: under the lock.
    let (l, _) = unsafe { traps::mxalloc_largest(MX_TTRAM, 4096) }.expect("largest");
    assert_eq!(count(p.pd()), 2);
    audit();
    assert_eq!(mfree(l), E_OK);
    assert_eq!(mfree(a), E_OK);
    assert_eq!(count(p.pd()), 0);
    audit();
    kernel();
}

#[test]
fn kernel_and_null_cookies_are_never_counted() {
    let _g = lock();
    kernel();
    let before = audit();
    let a = malloc(128);
    assert_ne!(a, 0);
    assert_eq!(count(traps::kernel_owner()), 0);
    assert_eq!(audit(), before);
    assert!(!owner_limit::is_limited(std::ptr::null_mut()));
    assert!(!owner_limit::is_limited(traps::kernel_owner()));
    assert!(!owner_limit::is_limited(traps::tsr_owner()));
    assert_eq!(mfree(a), E_OK);
}

// ── test 5: the limit ────────────────────────────────────────────────────

#[test]
fn a_process_at_its_limit_gets_zero_and_nobody_else_is_affected() {
    let _g = lock();
    owner_limit::set_limit(5);
    let p = Proc::new();
    let q = Proc::new();
    p.run();
    let mut held = Vec::new();
    loop {
        let a = malloc(32);
        if a == 0 {
            break;
        }
        held.push(a);
        assert!(held.len() <= 5, "the limit holds");
    }
    assert_eq!(held.len(), 5, "exactly `limit` blocks");
    assert_eq!(count(p.pd()), 5);
    assert_eq!(mxalloc(32, MX_STRAM), 0, "Mxalloc too");
    // SAFETY: under the lock.
    assert_eq!(unsafe { traps::mxalloc_aligned(64, MX_TTRAM, 4096) }, 0, "and the Pexec TPA path");
    // E5: Malloc(-1) still reports the largest free block.
    assert!(malloc(-1) > 0);
    assert!(mxalloc(-1, MX_PREFTTRAM) > 0);
    // Mshrink of a held block never fails at the limit.
    let big = held[0];
    // SAFETY: under the lock.
    assert_eq!(gemdos(0x4A, [0, big as u64, 16]), E_OK);
    // Another process and the kernel allocate unaffected.
    q.run();
    let qa = malloc(32);
    assert_ne!(qa, 0);
    kernel();
    let ka = malloc(32);
    assert_ne!(ka, 0);
    // A re-booking onto p is never refused: p sits above its limit for a while.
    // SAFETY: under the lock.
    assert!(unsafe { traps::set_owner(qa, p.pd()) });
    assert_eq!(count(p.pd()), 6);
    audit();
    // Mfree makes room again only below the limit.
    p.run();
    assert_eq!(mfree(qa), E_OK);
    assert_eq!(malloc(32), 0, "5 held = limit");
    assert_eq!(mfree(held.pop().unwrap()), E_OK);
    let again = malloc(32);
    assert_ne!(again, 0, "one free place again");
    held.push(again);
    audit();
    // Pterm releases everything; the cookie's entry is gone.
    // SAFETY: under the lock.
    assert_eq!(unsafe { free_all_owned(p.pd()) }, 5);
    assert_eq!(count(p.pd()), 0);
    kernel();
    assert_eq!(mfree(ka), E_OK);
    audit();
}

#[test]
fn limit_zero_is_off() {
    let _g = lock();
    owner_limit::set_limit(0);
    let p = Proc::new();
    p.run();
    let blocks: Vec<Long> = (0..64).map(|_| malloc(16)).collect();
    assert!(blocks.iter().all(|&a| a != 0));
    assert_eq!(count(p.pd()), 64, "still counted, just not limited");
    audit();
    // SAFETY: under the lock.
    assert_eq!(unsafe { free_all_owned(p.pd()) }, 64);
    kernel();
    audit();
}

#[test]
fn the_default_limit_is_8192() {
    assert_eq!(DEFAULT_OWNER_MD_LIMIT, 8192);
}
