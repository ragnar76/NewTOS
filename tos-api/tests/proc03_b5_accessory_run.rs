//! TASK/PROC03 B5, option 2 (user decision 2026-10-18) — what a desk
//! accessory sees as `run`.
//!
//! EmuTOS (`references/emutos`, pin `1a7484ab`): an accessory is never
//! `proc_go`ed and its GEMDOS calls execute with `run` unchanged
//! (`aes/gemasm.S:83-110`), i.e. the innermost started process — the program
//! in front, its running `Pexec` child, or the accdesk PD between programs
//! (`_run-ownership-curdir.md` §1.2, §1.3). Two things follow from it and are
//! built:
//!
//! * `*os_run` (OSHEADER +0x28, `bios/startup.S:110-111`) read by the
//!   accessory names that PD — NewTOS's per-process page-0 cell (D-7) is
//!   rewritten whenever the accessory becomes the running slot;
//! * a process the accessory `Pexec`s gets `p_parent = run`
//!   (`bdos/proc.c:541`), and its `Pterm` still resumes the accessory (in
//!   EmuTOS the accessory's trap saved its context into `run`'s PD,
//!   `bdos/rwa.S:188-206`; in NewTOS the parent record of the child slot).
//!
//! Everything else stays with the accessory (named deviation,
//! `docs/COMPATIBILITY.md`): after the child's `Pterm` the running process —
//! the owner of what is allocated next — is the accessory again.
//!
//! Basepages are host buffers; a stored pointer is compared in its low 32
//! bits (the bits the field holds on the target too).

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::bdos::process::{self, loader, mmu, pexec, pterm, standin, PROCESS_TABLE};
use tos_types::BasePage;

static LOCK: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

fn lock() -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any program; boot drive C: (2).
        let published = unsafe { standin::init(2) };
        assert!(published.is_some(), "the page is published");
    });
    standin::enter_aes();
    standin::enter_accdesk();
    process::set_current(core::ptr::null_mut());
    g
}

fn low32(p: *mut BasePage) -> u32 {
    p as u64 as u32
}

/// The `os_run` cell of `slot`'s page 0 (`loader::OS_RUN_CELL_VA`), as a
/// program reads it with a 4-byte load.
fn os_run_cell(slot: usize) -> u32 {
    let p0 = mmu::page0_kva(slot).expect("page 0");
    // SAFETY: the slot's static page 0; the cell is inside it.
    unsafe { (p0.add(loader::OS_RUN_CELL_VA as usize) as *const u32).read_unaligned() }
}

fn set_os_run_cell(slot: usize, v: u32) {
    let p0 = mmu::page0_kva(slot).expect("page 0");
    // SAFETY: as above.
    unsafe { (p0.add(loader::OS_RUN_CELL_VA as usize) as *mut u64).write_unaligned(u64::from(v)) };
}

/// A process in a slot with a host basepage.
struct Proc {
    slot: usize,
    bp: *mut BasePage,
    acc: bool,
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
        set_os_run_cell(slot, 0);
        Proc { slot, bp, acc: false }
    }
    /// Started by the kernel launcher (`launch.rs` → `set_kernel_parent`).
    fn kernel_started() -> Self {
        let c = Self::new();
        // SAFETY: under the lock; the slot holds a basepage.
        unsafe { pexec::set_kernel_parent(c.slot) };
        c
    }
    /// A desk accessory: loaded (`p_parent` 0, never started through
    /// `set_parent`), registered with the AES like `kernel/src/accs.rs`
    /// `load_one_acc` does, whole-frame resume record (ACC03).
    fn accessory() -> Self {
        let mut a = Self::new();
        PROCESS_TABLE[a.slot].parent_frame_full.store(true, Ordering::Release);
        tos_api::aes::acc::register_process(a.slot);
        a.acc = true;
        a
    }
    /// `Pexec`ed by `parent` (`handle_pexec`: `set_parent` with the caller's
    /// PD + `parent_slot`).
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
    /// This process becomes the running one (a scheduler switch, a `Pterm`
    /// return, a launch — every path ends in `set_current`).
    fn run(&self) {
        process::set_current(self.pd());
    }
    /// `Pterm` of this process (the real `terminate`), the slot released as
    /// the handler does after its frame copy.
    fn pterm(self) -> pterm::PtermResume {
        process::set_current(self.pd());
        // SAFETY: under the lock; `RUN` is this process's basepage.
        let r = unsafe { pterm::terminate(0) }.expect("Pterm of the running process");
        assert_eq!(r.parent_frame_slot, self.slot);
        // SAFETY: under the lock; the frame is not used on the host.
        unsafe { pterm::release_slot(self.slot) };
        r
    }
}
impl Drop for Proc {
    fn drop(&mut self) {
        if self.acc {
            tos_api::aes::acc::forget_process(self.slot);
        }
        if process::pd_of(self.slot) == self.bp {
            PROCESS_TABLE[self.slot].basepage_kva.store(core::ptr::null_mut(), Ordering::Release);
            process::free_slot(self.slot);
        }
        // SAFETY: the slot no longer names the box.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

#[test]
fn an_accessory_between_programs_reads_the_accdesk_pd_as_os_run() {
    let _g = lock();
    let a = Proc::accessory();
    a.run();
    assert_eq!(os_run_cell(a.slot), low32(standin::accdesk_pd()), "run = accdesk (aes/geminit.c:894..739)");
}

#[test]
fn an_accessory_reads_the_program_in_front_as_os_run() {
    let _g = lock();
    let a = Proc::accessory();
    let z = Proc::kernel_started();
    z.run();
    a.run();
    assert_eq!(os_run_cell(a.slot), low32(z.pd()), "run = the program in front");
    // The program's own cell is the loader's and names itself; it is not
    // rewritten (only an accessory sees another PD).
    set_os_run_cell(z.slot, low32(z.pd()));
    z.run();
    assert_eq!(os_run_cell(z.slot), low32(z.pd()));
}

#[test]
fn an_accessory_reads_the_innermost_started_process_and_follows_its_end() {
    let _g = lock();
    let a = Proc::accessory();
    let z = Proc::kernel_started();
    let k = Proc::child_of(&z);
    a.run();
    assert_eq!(os_run_cell(a.slot), low32(k.pd()), "run = Z's running Pexec child");
    let r = k.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, None));
    a.run();
    assert_eq!(os_run_cell(a.slot), low32(z.pd()), "after the child's Pterm: run = Z");
    let r = z.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, None));
    a.run();
    assert_eq!(os_run_cell(a.slot), low32(standin::accdesk_pd()), "after Z: run = accdesk");
}

#[test]
fn an_accessorys_pexec_child_has_run_as_parent_and_returns_to_the_accessory() {
    let _g = lock();
    let a = Proc::accessory();
    let z = Proc::kernel_started();
    a.run();
    let c = Proc::child_of(&a);
    assert_eq!(c.p_parent(), low32(z.pd()), "p_parent = run (bdos/proc.c:541), not the accessory");
    // The child's own child: its caller is a process, the caller's PD.
    let g = Proc::child_of(&c);
    assert_eq!(g.p_parent(), low32(c.pd()));
    let r = g.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, None));
    assert_eq!(process::current(), c.pd());
    // The child's Pterm: unchanged p_parent, back to the accessory — the
    // running process (owner of what is allocated next) is the accessory.
    let r = c.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, None), "the recorded parent, no fallback");
    assert_eq!(process::current(), a.pd(), "the accessory resumes");
    assert_eq!(os_run_cell(a.slot), low32(z.pd()), "and reads run = Z again");
}

#[test]
fn an_accessorys_pexec_child_between_programs_has_accdesk_as_parent() {
    let _g = lock();
    let a = Proc::accessory();
    a.run();
    let c = Proc::child_of(&a);
    assert_eq!(c.p_parent(), low32(standin::accdesk_pd()), "run = accdesk");
    a.run();
    assert_eq!(os_run_cell(a.slot), low32(c.pd()), "while the child lives it is run");
    let r = c.pterm();
    assert_eq!((r.unwind, r.ignored_parent), (0, None));
    assert_eq!(process::current(), a.pd(), "the accessory resumes");
    assert_eq!(os_run_cell(a.slot), low32(standin::accdesk_pd()));
}

#[test]
fn a_program_that_is_not_an_accessory_keeps_its_own_pd_as_the_childs_parent() {
    let _g = lock();
    let _a = Proc::accessory();
    let z = Proc::kernel_started();
    let k = Proc::child_of(&z);
    assert_eq!(k.p_parent(), low32(z.pd()), "caller = run for a program");
    // An unregistered loaded process (not an accessory) is its children's parent.
    let x = Proc::new();
    let c = Proc::child_of(&x);
    assert_eq!(c.p_parent(), low32(x.pd()));
}
