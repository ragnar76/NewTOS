//! ARCH03 stage 2b/2c on the host: the parts of `Pexec` 4/5/6/7 and
//! `Ptermres` that are bookkeeping, not memory copies.
//!
//! Sources: EmuTOS `bdos/proc.c` — `xexec` modes 5/6/7 (`:210-248`), the
//! unknown-mode default (`:252-253`), `proc_go` starting at `p_tbase` on a
//! stack at `p_hitpa` (`:544,548`), `set_owner(p, p); set_owner(p->p_env, p)`
//! for mode 6 (`:238-242`); `xtermres` (`:624-634`) — the blocks survive. The
//! TSR holder owning them on the allocated list is FreeMiNT's `TSR_Q`
//! (`sys/k_exit.c:307-336`, plan §3.8). Spec: `docs/api/gemdos/process/
//! 4b-pexec.md`, `31-ptermres.md` (NewTOS notes).
//!
//! The pools are bookkeeping only (their addresses are never dereferenced);
//! a basepage the code under test reads is a host buffer, as in
//! `arch03_shared_space.rs`.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::mem::free_all_owned::reserve_all_owned;
use tos_api::bdos::mem::{init, traps};
use tos_api::bdos::process::loader::EnvSpec;
use tos_api::bdos::process::pexec::{self, PexecError};
use tos_api::bdos::process::{self, Disposition, State, PROCESS_TABLE};
use tos_types::error::{EIMBA, EINVFN};
use tos_types::{BasePage, MD, MX_STRAM, MX_TTRAM};


const ST_START: u64 = 0x0040_0000;
const ST_LEN: u64 = 10 * 1024 * 1024;
const TT_START: u64 = 0x0100_0000;
const TT_LEN: u64 = 64 * 1024 * 1024;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any allocation.
        unsafe { init::init(ST_START, ST_LEN, TT_START, TT_LEN) }.expect("the test pools");
    });
    g
}

/// The MD of the allocated block at `addr`, if any.
fn md_of(addr: i32) -> Option<*mut MD> {
    for mp in [init::pmd(), init::pmdalt()] {
        // SAFETY: under the lock; read-only walk.
        let mut m = unsafe { (*mp).mp_mal };
        while !m.is_null() {
            // SAFETY: list invariant.
            unsafe {
                if (*m).m_start == addr {
                    return Some(m);
                }
                m = (*m).m_link;
            }
        }
    }
    None
}

fn owner_of(addr: i32) -> *mut core::ffi::c_void {
    // SAFETY: under the lock.
    unsafe { (*md_of(addr).expect("allocated")).m_own }
}

/// A slot in the "loaded, not started" state with a host basepage, as
/// `Pexec` 3 or 5 leaves it.
struct Unstarted {
    slot: usize,
    bp: *mut BasePage,
}
impl Unstarted {
    fn new(entry_va: u64, fill: impl FnOnce(&mut BasePage)) -> Self {
        let slot = process::allocate_slot().expect("a slot");
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        // SAFETY: a fresh, exclusively owned 256-byte, 4-aligned buffer.
        fill(unsafe { &mut *bp });
        let p = &PROCESS_TABLE[slot];
        p.basepage_kva.store(bp, Ordering::Release);
        p.entry_va.store(entry_va, Ordering::Release);
        p.sp_el0.store(if entry_va != 0 { 0x0123_0000 } else { 0 }, Ordering::Release);
        Unstarted { slot, bp }
    }
    fn pd(&self) -> u64 {
        self.bp as u64
    }
}
impl Drop for Unstarted {
    fn drop(&mut self) {
        process::free_slot(self.slot);
        // SAFETY: the slot no longer names the box.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

#[test]
fn an_unknown_mode_is_einvfn_and_5_6_7_are_known() {
    let _g = lock();
    // SAFETY: under the lock.
    let e = unsafe { pexec::dispatch(8, &[], 0, 0, &[], EnvSpec::None) }.unwrap_err();
    assert_eq!(e, PexecError::UnknownMode);
    assert_eq!(e.errno(), EINVFN);
    for m in [1u16, 2, 50, 100, 104, 106, 200] {
        // SAFETY: under the lock.
        let e = unsafe { pexec::dispatch(m, &[], 0, 0, &[], EnvSpec::None) }.unwrap_err();
        assert_eq!(e, PexecError::UnknownMode, "mode {m}");
    }
    assert_eq!((pexec::PE_BASEPAGE, pexec::PE_GOTHENFREE, pexec::PE_BASEPAGEFLAGS), (5, 6, 7));
}

#[test]
fn go_takes_only_an_unstarted_pd() {
    let _g = lock();
    let u = Unstarted::new(0, |bp| bp.p_tbase = 0x0123_4000);
    for then_free in [false, true] {
        // SAFETY: under the lock.
        let e = unsafe { pexec::go(u.pd() + 0x100, then_free) }.unwrap_err();
        assert_eq!(e.errno(), EIMBA, "not a PD");
        // SAFETY: under the lock.
        let e = unsafe { pexec::go(0, then_free) }.unwrap_err();
        assert_eq!(e.errno(), EIMBA, "null");
    }
    // A slot that is running (or in the run-set) is not "unstarted".
    PROCESS_TABLE[u.slot].set_state(State::Running);
    // SAFETY: under the lock.
    assert_eq!(unsafe { pexec::go(u.pd(), false) }.unwrap_err(), PexecError::BadHandle);
    PROCESS_TABLE[u.slot].set_state(State::Loaded);
}

#[test]
fn a_basepage_without_an_image_starts_at_p_tbase_on_p_hitpa() {
    let mut b = [0u8; 256];
    // SAFETY: a 256-byte, 4-aligned-enough local buffer read as a BasePage.
    let bp = unsafe { &mut *(b.as_mut_ptr() as *mut BasePage) };
    bp.p_tbase = 0x0123_4560;
    bp.p_hitpa = 0x0040_1107;
    assert_eq!(
        pexec::start_of(0, bp),
        (0x0123_4560, 0x0040_1100),
        "proc_go: retaddr = p_tbase, stack at p_hitpa (16-aligned)"
    );
}

#[test]
fn a_loaded_program_keeps_its_elf_entry_and_takes_the_stack_at_p_hitpa() {
    let mut b = [0u8; 256];
    // SAFETY: as above.
    let bp = unsafe { &mut *(b.as_mut_ptr() as *mut BasePage) };
    bp.p_tbase = 0x0200_0fc0;
    bp.p_hitpa = 0x0240_0000;
    // The ELF entry, but the stack where p_hitpa is now: the shell idiom
    // lowers it after Mshrink, and proc_go builds the stack there (:544).
    assert_eq!(pexec::start_of(0x0200_1000, bp), (0x0200_1000, 0x0240_0000));
    bp.p_hitpa = 0x0204_3008;
    assert_eq!(pexec::start_of(0x0200_1000, bp), (0x0200_1000, 0x0204_3000));
}

#[test]
fn go_refuses_a_pd_its_caller_does_not_own() {
    let _g = lock();
    // The host basepage is no pool block, so nobody owns it (E15): refused
    // although the slot is waiting to be started.
    let u = Unstarted::new(0, |bp| bp.p_tbase = 0x0123_4000);
    for then_free in [false, true] {
        // SAFETY: under the lock.
        let e = unsafe { pexec::go(u.pd(), then_free) }.unwrap_err();
        assert_eq!(e, PexecError::BadHandle);
        assert_eq!(e.errno(), EIMBA);
    }
}

#[test]
fn ptermres_keeps_every_owned_block_allocated_under_the_tsr_holder() {
    let _g = lock();
    let tsr = 0x7ff0_0000usize as *mut core::ffi::c_void; // a PD cookie
    let other = 0x7fe0_0000usize as *mut core::ffi::c_void;
    // SAFETY: under the lock.
    let (a, b, c) = unsafe {
        (traps::mxalloc(256, MX_STRAM), traps::mxalloc(4096, MX_TTRAM), traps::mxalloc(32, MX_STRAM))
    };
    // SAFETY: under the lock.
    unsafe {
        traps::set_owner(a, tsr);
        traps::set_owner(b, tsr);
        traps::set_owner(c, other);
    }
    // SAFETY: under the lock.
    assert_eq!(unsafe { reserve_all_owned(tsr) }, 2);
    for blk in [a, b] {
        assert!(traps::is_resident(owner_of(blk)), "0x{blk:x} kept by the TSR holder");
        // SAFETY: under the lock.
        assert!(unsafe { traps::is_allocated(blk as u64) }, "still allocated");
    }
    assert_eq!(owner_of(c), other, "someone else's block is untouched");
    // A later Pterm of the same PD finds nothing to free.
    // SAFETY: under the lock.
    assert_eq!(unsafe { tos_api::bdos::mem::free_all_owned::free_all_owned(tsr) }, 0);
    assert!(!traps::is_resident(traps::current_owner()));
    // SAFETY: under the lock.
    unsafe {
        traps::mfree(a);
        traps::mfree(b);
        traps::mfree(c);
    }
}

#[test]
fn a_terminate_disposition_carries_ptermres_blkln() {
    for (rc, reserve) in [
        (0, None),
        (0xFFFF, None),
        (0x31, Some(0)),
        (0x31, Some(0x2A40)),
        (7, Some(-1)),
        (0xFFFF, Some(i32::MAX)),
        (1, Some(i32::MIN)),
    ] {
        let d = Disposition::Terminate { rc, reserve };
        assert_eq!(Disposition::decode(d.encode()), d, "rc {rc} reserve {reserve:?}");
    }
    assert_eq!(Disposition::decode(0), Disposition::Resume);
    assert_eq!(Disposition::decode(Disposition::UserdefReturn.encode()), Disposition::UserdefReturn);
}
