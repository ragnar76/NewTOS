//! TASK/PROC03 stage 1 + B4 — the `p_parent` chain through the accdesk and
//! EmuCON stand-ins, and `Pterm` returning through `p_parent`.
//!
//! EmuTOS (`references/emutos`, pin `1a7484ab`):
//!
//! * `p->p_parent = run` (`proc_go`, `bdos/proc.c:541`); `run` is the AES PD
//!   while the AES initialises, the **accdesk PD** for the whole of
//!   `run_accs_and_desktop` (`aes/geminit.c:654-750,894`,
//!   `aes/gemshlib.c:570-581`) — the parent of the desktop, the `#Z` program
//!   and the programs the desktop asks for (`aes/gemshlib.c:600-611,645`) —
//!   and the **EmuCON PD** while EmuCON runs (`aes/gemshlib.c:614-620`,
//!   `cli/cmdexec.c:52`). Spec: `docs/api/gemdos/process/
//!   _parent-and-environment.md` §1.1, §2.3, §2.4.
//! * `xterm`: `run = run->p_parent` (`bdos/proc.c:609`) — unchecked; the
//!   decision for a changed value is D-5 (`_run-ownership-curdir.md` §6.4):
//!   an ancestor on the launch chain is resumed (the processes between end),
//!   anything else returns to the recorded parent.
//!
//! Basepages are host buffers; a stored pointer is compared in its low 32
//! bits (the bits the field holds on the target too).

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::process::{self, pexec, pterm, standin, NO_PARENT, PROCESS_TABLE};
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

fn low32(p: *mut BasePage) -> u32 {
    p as u64 as u32
}

/// A started process in a slot with a host basepage.
struct Proc {
    slot: usize,
    bp: *mut BasePage,
}
impl Proc {
    fn new() -> Self {
        let slot = process::allocate_slot().expect("a slot");
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        let p = &PROCESS_TABLE[slot];
        p.basepage_kva.store(bp, Ordering::Release);
        p.set_state(process::State::Running);
        // A resume record, as `handle_pexec` / the launcher leave one.
        p.parent_frame.elr().store(1, Ordering::Release);
        Proc { slot, bp }
    }
    /// Started by the kernel launcher (`launch.rs` → `set_kernel_parent`).
    fn kernel_started() -> Self {
        let c = Self::new();
        // SAFETY: under the lock; the slot holds a basepage.
        unsafe { pexec::set_kernel_parent(c.slot) };
        c
    }
    /// `Pexec`ed by `parent` (`handle_pexec`: `set_parent` + `parent_slot`).
    fn child_of(parent: &Proc) -> Self {
        let c = Self::new();
        // SAFETY: under the lock; memory protection is off on the host.
        unsafe { pexec::set_parent(c.slot, parent.pd()) };
        PROCESS_TABLE[c.slot].parent_slot.store(parent.slot as u32, Ordering::Release);
        c
    }
    fn pd(&self) -> *mut BasePage {
        process::pd_of(self.slot)
    }
    fn p_parent(&self) -> u32 {
        // SAFETY: the box is alive until drop.
        unsafe { (*self.bp).p_parent as u32 }
    }
    fn set_p_parent(&self, v: u32) {
        // SAFETY: as above — the program writing its own basepage.
        unsafe { (*self.bp).p_parent = v as i32 };
    }
    /// `Pterm` of this process (the real `terminate`), the slot kept.
    fn pterm(&self) -> pterm::PtermResume {
        process::set_current(self.pd());
        // SAFETY: under the lock; `RUN` is this process's basepage.
        let r = unsafe { pterm::terminate(0) }.expect("Pterm of the running process");
        assert_eq!(r.parent_frame_slot, self.slot);
        r
    }
}
impl Drop for Proc {
    fn drop(&mut self) {
        if process::pd_of(self.slot) == self.bp {
            PROCESS_TABLE[self.slot].basepage_kva.store(core::ptr::null_mut(), Ordering::Release);
            process::free_slot(self.slot);
        }
        // SAFETY: the slot no longer names the box.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

#[test]
fn desktop_autorun_and_shel_write_programs_are_children_of_accdesk() {
    let _g = lock();
    standin::enter_aes();
    standin::enter_accdesk();
    let z = Proc::kernel_started();
    assert_eq!(z.p_parent(), low32(standin::accdesk_pd()), "X -> accdesk");
    // SAFETY: the stand-in page is a live static (host: user VA = kernel VA).
    let acc = unsafe { &*standin::accdesk_pd() };
    assert_eq!(acc.p_parent as u32, low32(standin::aes_pd()), "accdesk -> AES");
    assert_eq!(acc.p_flags, PF_STANDARD);
    assert_eq!(acc.p_lowtpa as u32, low32(standin::accdesk_pd()));
    // SAFETY: as above.
    let aes = unsafe { &*standin::aes_pd() };
    assert_eq!(aes.p_parent as u32, low32(standin::initial_basepage()), "AES -> initial");
    // SAFETY: as above.
    assert_eq!(unsafe { (*standin::initial_basepage()).p_parent }, 0, "initial -> 0");
    // `alloc_env(flags, NULL)` with `run` = accdesk: a copy of its block,
    // which is a copy of `ad_envrn` — not the same block.
    assert_eq!(standin::kernel_run_env().unwrap(), b"PATH=\0C:\\\0\0");
    assert_ne!(acc.p_env, aes.p_env, "accdesk owns a copy");
    standin::leave_accdesk();
    assert_eq!(standin::kernel_run(), standin::aes_pd(), "Pterm0 of accdesk: run = AES PD");
    standin::leave_aes();
}

#[test]
fn a_program_started_from_emucon_is_a_child_of_the_emucon_pd() {
    let _g = lock();
    standin::enter_aes();
    standin::enter_accdesk();
    standin::enter_emucon();
    // EmuCON's `run`: an EL1 `Pexec`, no running process.
    let e = Proc::new();
    // SAFETY: under the lock.
    unsafe { pexec::set_parent(e.slot, core::ptr::null_mut()) };
    assert_eq!(e.p_parent(), low32(standin::emucon_pd()), "X -> EmuCON");
    // SAFETY: the stand-in page is a live static.
    let emu = unsafe { &*standin::emucon_pd() };
    assert_eq!(emu.p_parent as u32, low32(standin::accdesk_pd()), "EmuCON -> accdesk");
    assert_eq!(emu.p_flags, PF_STANDARD);
    assert_eq!(emu.p_hitpa as u32, low32(standin::emucon_pd()) + 0x100, "D-2");
    assert_eq!(emu.p_tbase, 0, "D-1");
    standin::leave_emucon();
    assert_eq!(standin::kernel_run(), standin::accdesk_pd(), "EmuCON's exit: run = accdesk");
    standin::leave_accdesk();
    standin::leave_aes();
}

#[test]
fn pterm_with_an_unchanged_p_parent_returns_to_the_recorded_parent() {
    let _g = lock();
    standin::enter_aes();
    standin::enter_accdesk();
    let z = Proc::kernel_started();
    let k = Proc::child_of(&z);
    let r = k.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, None));
    assert_eq!(process::current(), z.pd(), "run = run->p_parent");
    let r = z.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, None), "the root: back to the launcher");
    assert!(process::current().is_null());
    standin::leave_accdesk();
    standin::leave_aes();
}

#[test]
fn pterm_through_a_changed_p_parent_unwinds_to_that_ancestor() {
    let _g = lock();
    standin::enter_aes();
    standin::enter_accdesk();
    let z = Proc::kernel_started();
    let k = Proc::child_of(&z);
    let g = Proc::child_of(&k);

    // G names Z: Z resumes, K ends without being resumed.
    g.set_p_parent(low32(z.pd()));
    assert_eq!(g.pterm().unwind, 1);
    // G names accdesk, the root's starter: Z ends too, the launcher resumes.
    g.set_p_parent(low32(standin::accdesk_pd()));
    assert_eq!(g.pterm().unwind, 2);
    // G names its own parent again: the normal return.
    g.set_p_parent(low32(k.pd()));
    assert_eq!(g.pterm().unwind, 0);

    // The handler's sequence for unwind = 1: release G's slot, terminate the
    // next process (`RUN` = K after G's teardown), resume K's parent record.
    g.set_p_parent(low32(z.pd()));
    let r = g.pterm();
    assert_eq!(r.unwind, 1);
    assert_eq!(process::current(), k.pd(), "RUN is the next process to end");
    // SAFETY: under the lock; G's frame is not used.
    unsafe { pterm::release_slot(g.slot) };
    // SAFETY: under the lock; `RUN` = K.
    let r = unsafe { pterm::terminate_unwound() }.expect("K ends");
    assert_eq!(r.parent_frame_slot, k.slot, "Z resumes from K's parent record");
    assert_eq!((r.unwind, r.ignored_parent), (0, None), "K's own p_parent is not consulted");
    assert_eq!(process::current(), z.pd(), "run = Z");
    standin::leave_accdesk();
    standin::leave_aes();
}

#[test]
fn pterm_through_an_invalid_p_parent_returns_to_the_recorded_parent() {
    let _g = lock();
    standin::enter_aes();
    standin::enter_accdesk();
    let z = Proc::kernel_started();
    let k = Proc::child_of(&z);
    let g = Proc::child_of(&k);
    let other = Proc::kernel_started(); // a process not on G's chain
    for v in [
        0u32,
        0x1234_5000,
        low32(standin::aes_pd()),         // a stand-in with no resume point here
        low32(standin::initial_basepage()),
        low32(other.pd()),
        low32(g.pd()),                    // itself
    ] {
        g.set_p_parent(v);
        let r = g.pterm();
        assert_eq!((r.unwind, r.ignored_parent), (0, Some(v)), "p_parent {v:#x}");
        assert_eq!(process::current(), k.pd(), "the recorded parent resumes");
    }
    // A root that changed its own p_parent: back to its launcher.
    z.set_p_parent(low32(standin::aes_pd()));
    let r = z.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, Some(low32(standin::aes_pd()))));
    standin::leave_accdesk();
    standin::leave_aes();
}

#[test]
fn an_accessory_root_is_resumable_but_its_zero_starter_is_not() {
    let _g = lock();
    standin::enter_aes();
    standin::enter_accdesk();
    // An accessory: loaded, never `proc_go`ed — `p_parent` 0, no
    // `launch_parent`, a whole-frame resume record (ACC03).
    let a = Proc::new();
    PROCESS_TABLE[a.slot].parent_frame_full.store(true, Ordering::Release);
    assert_eq!(PROCESS_TABLE[a.slot].parent_slot.load(Ordering::Acquire), NO_PARENT);
    let c = Proc::child_of(&a);
    let g = Proc::child_of(&c);
    g.set_p_parent(low32(a.pd()));
    assert_eq!(g.pterm().unwind, 1, "the accessory's pending Pexec resumes");
    g.set_p_parent(0);
    assert_eq!(g.pterm().ignored_parent, Some(0), "0 is never a resume point");
    PROCESS_TABLE[a.slot].parent_frame_full.store(false, Ordering::Release);
    standin::leave_accdesk();
    standin::leave_aes();
}
