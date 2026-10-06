//! TASK/PPARENT — `p_parent` and the environment of kernel-launched programs
//! as on EmuTOS, through two stand-in PDs (user decision 2026-10-06, Way A).
//!
//! EmuTOS (`references/emutos`, pin `1a7484ab`): `p_parent` is written only by
//! `proc_go`, `p->p_parent = run` (`bdos/proc.c:541`); `run` is
//! `&initial_basepage` while the BIOS runs the AUTO folder
//! (`bdos/bdosmain.c:337-342`, `bios/bios.c:685-695`) and the AES PD from the
//! AES on (`bios/bios.c:855-866`, `aes/gemshlib.c:569-580,645`); an accessory
//! is never `proc_go`ed and keeps 0 (`util/gemdos.c:29-47`,
//! `bdos/proc.c:370`). `ad_envrn` is the AES PD's `p_env`
//! (`aes/gemstart.S:78-96`), a copy of `default_env` "PATH=\0X:\\\0"
//! (`bios/bios.c:626-636`). Spec:
//! `docs/api/gemdos/process/_parent-and-environment.md`.
//!
//! Basepages are host buffers (as in `arch03_pexec_567_ptermres.rs`). Host
//! addresses are wider than the 32-bit basepage fields, so a stored pointer is
//! compared in its low 32 bits — the bits the field holds on the target too.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::process::Abi;
use tos_api::bdos::process::{self, pexec, standin, PROCESS_TABLE};
use tos_types::{BasePage, PF_STANDARD};

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any program; boot drive C: (2).
        let published = unsafe { standin::init(2) };
        assert!(published.is_some(), "the page is published");
    });
    g
}

/// A loaded, not yet started slot with a host basepage.
struct Child {
    slot: usize,
    bp: *mut BasePage,
}
impl Child {
    fn new() -> Self {
        let slot = process::allocate_slot().expect("a slot");
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        PROCESS_TABLE[slot].basepage_kva.store(bp, Ordering::Release);
        Child { slot, bp }
    }
    fn p_parent(&self) -> u32 {
        // SAFETY: the box is alive until drop.
        unsafe { (*self.bp).p_parent as u32 }
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        PROCESS_TABLE[self.slot].basepage_kva.store(core::ptr::null_mut(), Ordering::Release);
        process::free_slot(self.slot);
        // SAFETY: the slot no longer names the box.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

fn low32(p: *mut BasePage) -> u32 {
    p as u64 as u32
}

#[test]
fn an_auto_program_has_initial_basepage_as_parent_and_an_empty_environment() {
    let _g = lock();
    standin::leave_aes();
    let c = Child::new();
    // SAFETY: under the lock; the slot holds a basepage.
    unsafe { pexec::set_kernel_parent(c.slot) };
    assert_eq!(c.p_parent(), low32(standin::initial_basepage()));
    assert_ne!(c.p_parent(), 0, "not an accessory to mintlib");
    // `alloc_env(flags, NULL)` with `run` = initial_basepage: `double_nul`.
    assert_eq!(standin::kernel_run_env().unwrap(), &[0u8, 0][..]);
}

#[test]
fn a_desktop_or_autorun_program_has_the_aes_pd_as_parent_and_its_environment() {
    let _g = lock();
    standin::enter_aes();
    let c = Child::new();
    // SAFETY: under the lock.
    unsafe { pexec::set_kernel_parent(c.slot) };
    assert_eq!(c.p_parent(), low32(standin::aes_pd()));
    // `ad_envrn` = the AES PD's environment, `default_env` for C:, envsize 11.
    assert_eq!(standin::kernel_run_env().unwrap(), b"PATH=\0C:\\\0\0");
    standin::leave_aes();
}

#[test]
fn the_chain_ends_at_initial_basepage() {
    let _g = lock();
    // The stand-ins are real basepages: follow p_parent from the AES PD
    // (kernel view = the user address on the host, VA = PA on the target).
    // SAFETY: the stand-in page is a live static.
    let aes = unsafe { &*standin::aes_pd() };
    assert_eq!(aes.p_parent as u32, low32(standin::initial_basepage()));
    assert_eq!(aes.p_flags, PF_STANDARD);
    assert_eq!(aes.p_lowtpa as u32, low32(standin::aes_pd()));
    // SAFETY: as above.
    let ib = unsafe { &*standin::initial_basepage() };
    assert_eq!(ib.p_parent, 0, "initial_basepage is the root");
    assert_eq!(ib.p_uft, [-1, -1, -2, -3, -1, -1], "default_handle (console.c:60-68)");
}

#[test]
fn a_pexec_from_a_process_keeps_the_callers_pd_and_a_kernel_caller_gets_the_stand_in() {
    let _g = lock();
    standin::enter_aes();
    let parent = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
    let c = Child::new();
    // SAFETY: under the lock; memory protection is off on the host.
    unsafe { pexec::set_parent(c.slot, parent) };
    assert_eq!(c.p_parent(), low32(parent), "p->p_parent = run (a process)");
    // An EL1 caller (the in-kernel EmuCON's `run`): no process is running.
    // SAFETY: as above.
    unsafe { pexec::set_parent(c.slot, core::ptr::null_mut()) };
    assert_eq!(c.p_parent(), low32(standin::aes_pd()), "run = the AES PD stand-in");
    standin::leave_aes();
    drop(c);
    // SAFETY: nothing names the box any more.
    drop(unsafe { Box::from_raw(parent as *mut [u8; 256]) });
}

/// `shel_envrn(&p, search)` through the real dispatcher (LP64, kernel
/// caller) → (`int_out[0]`, `p`).
fn envrn(search: &[u8]) -> (i16, u64) {
    let mut out: u64 = 1;
    let mut control = [125i16, 0, 1, 2, 0];
    let mut global = [0i16; 16];
    let mut intin = [0i16; 4];
    let mut intout = [0i16; 8];
    let mut addrin = [&mut out as *mut u64 as u64, search.as_ptr() as u64];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    // SAFETY: a well-formed LP64 parameter block, alive for the call.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    (intout[0], out)
}

#[test]
fn shel_envrn_answers_from_the_aes_pd_environment() {
    let _g = lock();
    let (env, _) = standin::aes_env().unwrap();
    // `*p` points just past "PATH=" — at the NUL before "C:" (the TOS quirk
    // `sh_find` skips, gemshlib.c:448-455).
    let (rc, p) = envrn(b"PATH=\0");
    assert_eq!(rc, 1);
    assert_eq!(p, env.as_ptr() as u64 + 5, "the value starts after \"PATH=\"");
    // SAFETY: `p` points into the live stand-in page.
    let v = unsafe { std::slice::from_raw_parts(p as *const u8, 5) };
    assert_eq!(v, b"\0C:\\\0");
    // An unknown variable: NULL, still 1 (gemshlib.c:333,347).
    assert_eq!(envrn(b"NOSUCHVAR=\0"), (1, 0));
}
