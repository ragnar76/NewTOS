//! TASK/PROC04 point 2 (with TASK/PROC03, user decision 2026-10-18 "alles
//! EmuTOS-treu") — the current drive and the per-drive current directory
//! belong to a PD, are inherited when a PD is created, and are changed only
//! in `run`'s PD.
//!
//! EmuTOS (`references/emutos`, pin `1a7484ab`;
//! `docs/api/gemdos/process/_run-ownership-curdir.md` §4):
//!
//! * `init_pd_files` (`bdos/proc.c:387-410`) copies `run->p_curdir[]` (with a
//!   `dirtbl` use each) and `run->p_curdrv` into every PD `xexec` creates.
//! * `Dgetdrv`/`Dsetdrv` (`bdos/fsmain.c:420-441`), `Dsetpath`/`Dgetpath`
//!   (`bdos/fsdir.c:1113-1251`) and relative path resolution (`dcrack`,
//!   `:1695-1731`) read and write `run`'s PD only.
//! * `ixterm` drops the dying PD's directory uses (`bdos/proc.c:126-132`).
//! * In the AES shell, `run` is the accdesk PD: `sh_chdef` → `set_drvdir`
//!   moves *its* directory (`aes/gemshlib.c:526-554`); EmuCON's `cd` moves the
//!   EmuCON PD's; `dos_sdrv(bootdev)` on a restart the AES PD's
//!   (`aes/geminit.c:796-801`).
//!
//! The PD (`p_curdrv` at +0x37) is a host buffer here; the volume is the
//! in-memory FAT16 image of `fatvol` (`C:\SUB\INNER.TXT`, `C:\HELLO.TXT`).

mod fatvol;

use std::sync::atomic::Ordering;
use std::sync::Once;

use fatvol::*;
use tos_api::aes::shell;
use tos_api::bdos::process::{self, loader, pexec, pterm, standin, PROCESS_TABLE};
use tos_types::BasePage;

const DSETPATH: u16 = 0x3B;
const DGETDRV: u16 = 0x19;

static SETUP: Once = Once::new();

/// The file-system lock of `fatvol`, plus the stand-in page built once with
/// boot drive `C:` and the kernel's `run` = the accdesk PD (between programs).
fn setup() -> (std::sync::MutexGuard<'static, ()>, &'static DiskHost) {
    let (g, h) = fs();
    SETUP.call_once(|| {
        // SAFETY: once per binary, before any program; boot drive C: (2).
        assert!(unsafe { standin::init(2) }.is_some());
    });
    process::set_current(core::ptr::null_mut());
    standin::leave_aes();
    standin::enter_aes();
    standin::enter_accdesk();
    (g, h)
}

/// Back to the state the next test expects: no process, accdesk left.
fn teardown() {
    process::set_current(core::ptr::null_mut());
    standin::leave_emucon();
    standin::leave_accdesk();
    standin::leave_aes();
}

/// A process in a slot with a host basepage, created the way `xexec` creates
/// one: `init_pd_fields` zeroes it, `init_pd_files` inherits from `run`.
struct Proc {
    slot: usize,
    bp: *mut BasePage,
}
impl Proc {
    /// Created while `run` is whatever [`process::current`] / the kernel's
    /// stand-in is now.
    fn create() -> Self {
        let slot = process::allocate_slot().expect("a slot");
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        let p = &PROCESS_TABLE[slot];
        p.basepage_kva.store(bp, Ordering::Release);
        p.set_state(process::State::Running);
        p.parent_frame.elr().store(1, Ordering::Release);
        // SAFETY: under the lock; `bp` is a live, zeroed basepage that is not
        // the running one.
        unsafe { loader::init_pd_files(bp) };
        Proc { slot, bp }
    }
    /// Created by the kernel (`run` = the stand-in) and started by it.
    fn kernel_started() -> Self {
        process::set_current(core::ptr::null_mut());
        let c = Self::create();
        // SAFETY: under the lock.
        unsafe { pexec::set_kernel_parent(c.slot) };
        c
    }
    /// `Pexec`ed by `parent` (created while `parent` is `run`).
    fn child_of(parent: &Proc) -> Self {
        process::set_current(parent.pd());
        let c = Self::create();
        // SAFETY: under the lock.
        unsafe { pexec::set_parent(c.slot, parent.pd()) };
        PROCESS_TABLE[c.slot].parent_slot.store(parent.slot as u32, Ordering::Release);
        c
    }
    fn pd(&self) -> *mut BasePage {
        process::pd_of(self.slot)
    }
    /// Make this process `run`.
    fn run(&self) {
        process::set_current(self.pd());
    }
    /// The byte at PD+0x37.
    fn p_curdrv(&self) -> u8 {
        // SAFETY: the box is alive until drop.
        unsafe { *(self.bp as *const u8).add(0x37) }
    }
    fn pterm(&self) {
        self.run();
        // SAFETY: under the lock; `RUN` is this process's basepage.
        unsafe { pterm::terminate(0) }.expect("Pterm of the running process");
        // SAFETY: as above; the frame is not used on the host.
        unsafe { pterm::release_slot(self.slot) };
    }
}
impl Drop for Proc {
    fn drop(&mut self) {
        if process::pd_of(self.slot) == self.bp {
            // A process the test did not `Pterm` gives its directory uses back
            // as its `ixterm` would, so the next test starts from clean counts.
            // SAFETY: the box is alive.
            unsafe { tos_api::bdos::fs::curdir::release(self.bp) };
            PROCESS_TABLE[self.slot].basepage_kva.store(core::ptr::null_mut(), Ordering::Release);
            process::free_slot(self.slot);
        }
        // SAFETY: the slot no longer names the box.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

fn dsetpath(p: &str) -> i32 {
    let c = cpath(p);
    gemdos1(DSETPATH, c.as_ptr() as u64)
}

/// `Dgetpath(buf, drv)` as a string; panics on an error code.
fn dgetpath(drv: u16) -> String {
    let mut buf = [0xAAu8; 128];
    assert_eq!(gemdos2(DGETPATH, buf.as_mut_ptr() as u64, drv as u64), E_OK);
    let n = buf.iter().position(|&b| b == 0).expect("NUL-terminated");
    String::from_utf8(buf[..n].to_vec()).unwrap()
}

fn dgetdrv() -> i32 {
    gemdos1(DGETDRV, 0)
}

/// `Fopen(name, 0)` + `Fclose` → the open's result (a handle > 0, or the code).
fn fopen_result(name: &str) -> i32 {
    let c = cpath(name);
    let h = gemdos2(FOPEN, c.as_ptr() as u64, 0);
    if h > 0 {
        gemdos1(FCLOSE, h as u64);
    }
    h
}

/// The stand-in PD the kernel's `run` names, as a reference.
fn kernel_run_pd() -> &'static BasePage {
    // SAFETY: the stand-in page is a live static (host: user VA = kernel VA).
    unsafe { &*standin::kernel_run() }
}

#[test]
fn a_new_pd_inherits_run_s_current_drive_at_creation() {
    let (_g, _h) = setup();
    // `initial_basepage.p_curdrv = bootdev` (`bios/bios.c:827`), the AES PD
    // and accdesk inherit it.
    // SAFETY: stand-in page, live static.
    assert_eq!(unsafe { (*standin::initial_basepage()).p_curdrv }, 2);
    assert_eq!(kernel_run_pd().p_curdrv, 2, "accdesk inherits C:");

    // A program the kernel starts: PD+0x37 = accdesk's drive
    // (`bdos/proc.c:409`).
    let z = Proc::kernel_started();
    assert_eq!(z.p_curdrv(), 2, "PD+0x37 of a desktop program = C:");
    z.run();
    assert_eq!(dgetdrv(), 2);

    // Z moves to drive F: (no existence check, `bdos/fsmain.c:422-423`) —
    // its own byte, nobody else's.
    assert_ne!(gemdos1(DSETDRV, 5), 0, "Dsetdrv answers the drive map");
    assert_eq!(z.p_curdrv(), 5, "Dsetdrv writes run->p_curdrv");
    assert_eq!(dgetdrv(), 5);

    // Its child is created while Z is `run`: it starts on F:.
    let k = Proc::child_of(&z);
    assert_eq!(k.p_curdrv(), 5, "init_pd_files: p->p_curdrv = run->p_curdrv");
    k.run();
    assert_eq!(gemdos1(DSETDRV, 2) & 0b100, 0b100);
    assert_eq!(dgetdrv(), 2);
    z.run();
    assert_eq!(dgetdrv(), 5, "the child's Dsetdrv left the parent's drive alone");

    // The kernel's `run` (accdesk) never moved.
    process::set_current(core::ptr::null_mut());
    assert_eq!(dgetdrv(), 2, "accdesk still on C:");
    assert_eq!(kernel_run_pd().p_curdrv, 2);
    drop(k);
    drop(z);
    teardown();
}

#[test]
fn a_childs_dsetpath_does_not_move_the_parents_directory() {
    let (_g, _h) = setup();
    let p = Proc::kernel_started();
    p.run();
    assert_eq!(dgetpath(0), "", "a new desktop program starts at accdesk's root");
    assert_eq!(dsetpath("\\SUB"), E_OK);
    assert_eq!(dgetpath(0), "\\SUB");
    assert!(fopen_result("INNER.TXT") > 0, "relative name in P's directory");

    // The child inherits P's directory (`bdos/proc.c:401-406`)...
    let c = Proc::child_of(&p);
    c.run();
    assert_eq!(dgetpath(0), "\\SUB", "inherited at creation");
    assert!(fopen_result("INNER.TXT") > 0);
    // ...and moves only its own (`run->p_curdir[dlog] = newdir`, `fsdir.c:1145`).
    assert_eq!(dsetpath("\\"), E_OK);
    assert_eq!(dgetpath(0), "");
    assert_eq!(fopen_result("INNER.TXT"), EFILNF, "the child is at the root now");
    assert!(fopen_result("HELLO.TXT") > 0);

    p.run();
    assert_eq!(dgetpath(0), "\\SUB", "the parent's directory is unchanged");
    assert_eq!(dgetpath(3), "\\SUB", "Dgetpath(3) = C:");
    assert!(fopen_result("INNER.TXT") > 0, "relative names still resolve in \\SUB");
    assert_eq!(fopen_result("HELLO.TXT"), EFILNF);

    // The child's end keeps it that way.
    c.pterm();
    assert_eq!(process::current(), p.pd(), "run = run->p_parent");
    assert_eq!(dgetpath(0), "\\SUB");

    // accdesk (the kernel's `run`) was never touched.
    process::set_current(core::ptr::null_mut());
    assert_eq!(dgetpath(0), "", "accdesk still at the root");
    drop(c);
    drop(p);
    teardown();
}

#[test]
fn a_drive_prefix_moves_that_drives_directory_of_run_only() {
    let (_g, _h) = setup();
    let p = Proc::kernel_started();
    p.run();
    assert_eq!(gemdos1(DSETDRV, 0) & 0b100, 0b100, "current drive A:");
    // `Dsetpath("C:\SUB")` sets p_curdir[2], not p_curdrv (`fsdir.c:1122-1145`).
    assert_eq!(dsetpath("C:\\SUB"), E_OK);
    assert_eq!(dgetdrv(), 0);
    assert_eq!(dgetpath(3), "\\SUB");
    assert!(fopen_result("C:INNER.TXT") > 0, "C: relative = C:'s directory");
    process::set_current(core::ptr::null_mut());
    assert_eq!(dgetdrv(), 2);
    assert_eq!(dgetpath(3), "", "accdesk's C: directory unchanged");
    drop(p);
    teardown();
}

#[test]
fn kernel_context_calls_act_on_the_stand_in_that_is_run() {
    let (_g, _h) = setup();
    // The AES shell (`run` = accdesk) moves accdesk's directory
    // (`set_drvdir`, `aes/gemshlib.c:526-554`); a program it starts next
    // inherits it.
    process::set_current(core::ptr::null_mut());
    assert_eq!(dsetpath("C:\\SUB"), E_OK);
    assert_eq!(dgetpath(0), "\\SUB");
    let z = Proc::kernel_started();
    z.run();
    assert_eq!(dgetpath(0), "\\SUB", "inherited from accdesk at creation");
    assert_eq!(dsetpath("\\"), E_OK);
    process::set_current(core::ptr::null_mut());
    assert_eq!(dgetpath(0), "\\SUB", "the program's Dsetpath left accdesk alone");

    // EmuCON: its PD copies accdesk's at its start; its `cd` moves its own
    // (`cli/cmdint.c:250-255`); after its end accdesk is unchanged.
    standin::enter_emucon();
    assert_eq!(dgetpath(0), "\\SUB", "EmuCON inherits accdesk's directory");
    assert_eq!(dsetpath("\\"), E_OK);
    assert_eq!(dgetpath(0), "");
    let e = Proc::kernel_started();
    e.run();
    assert_eq!(dgetpath(0), "", "a program EmuCON starts inherits EmuCON's directory");
    process::set_current(core::ptr::null_mut());
    standin::leave_emucon();
    assert_eq!(dgetpath(0), "\\SUB", "accdesk unchanged after EmuCON");

    // Back to the root for the next session.
    assert_eq!(dsetpath("\\"), E_OK);
    drop(e);
    drop(z);
    teardown();
}

#[test]
fn sh_chdef_sets_the_directory_of_accdesk_not_of_the_running_program() {
    let (_g, _h) = setup();
    // A program P runs at the root; the shell (kernel context, `run` =
    // accdesk) prepares the next start in C:\SUB (`sh_chdef` → `set_drvdir`).
    let p = Proc::kernel_started();
    process::set_current(core::ptr::null_mut());
    shell::sh_wdef(b"C:\\SUB\\INNER.TXT\0", b"C:\\SUB\0");
    shell::set_sh_nextapp(shell::DESKTOP_APP);
    shell::sh_chdef();
    assert_eq!(dgetpath(0), "\\SUB", "accdesk is in C:\\SUB");
    p.run();
    assert_eq!(dgetpath(0), "", "the program P is not moved by the shell");
    let q = Proc::kernel_started();
    q.run();
    assert_eq!(dgetpath(0), "\\SUB", "the program started next inherits it");
    process::set_current(core::ptr::null_mut());
    assert_eq!(dsetpath("\\"), E_OK);
    drop(q);
    drop(p);
    teardown();
}

#[test]
fn the_restart_drive_reset_lands_in_the_aes_pd_and_the_next_accdesk() {
    let (_g, _h) = setup();
    process::set_current(core::ptr::null_mut());
    assert_eq!(gemdos1(DSETDRV, 5) & 0b100, 0b100);
    assert_eq!(kernel_run_pd().p_curdrv, 5, "accdesk moved to F:");
    // accdesk ends (`Pterm0`): `run` = AES PD, still on C:.
    standin::leave_accdesk();
    assert_eq!(dgetdrv(), 2, "the AES PD was never moved");
    // `dos_sdrv(bootdev)` (`aes/geminit.c:796-801`) with `run` = AES PD.
    tos_api::bdos::fs::drive::set_boot_drive();
    assert_eq!(kernel_run_pd().p_curdrv, 2);
    standin::enter_accdesk();
    assert_eq!(kernel_run_pd().p_curdrv, 2, "the next accdesk inherits the AES PD's drive");
    teardown();
}

// ── dirtbl use counts (reference [R], `_run-ownership-curdir.md` §4.1/§4.5) ──
// Added after the red run: they read `curdir::uses_of`/`slots_in_use`, which
// did not exist before the change.

use tos_api::bdos::fs::curdir;

#[test]
fn uses_follow_inheritance_dsetpath_and_pterm() {
    let (_g, _h) = setup();
    let base = curdir::slots_in_use();
    let p = Proc::kernel_started();
    p.run();
    assert_eq!(dsetpath("\\SUB"), E_OK);
    assert_eq!(curdir::uses_of(2, "\\SUB"), 1, "incr_curdir_usage: one PD names it");
    // `init_pd_files` takes one use per inherited byte (`bdos/proc.c:401-406`).
    let c = Proc::child_of(&p);
    assert_eq!(curdir::uses_of(2, "\\SUB"), 2);
    // The child moves to the root: its use is dropped (`fsdir.c:1150-1151`).
    c.run();
    assert_eq!(dsetpath("\\"), E_OK);
    assert_eq!(curdir::uses_of(2, "\\SUB"), 1);
    assert_eq!(dsetpath("\\SUB"), E_OK, "the same directory shares the slot");
    assert_eq!(curdir::uses_of(2, "\\SUB"), 2);
    assert_eq!(curdir::slots_in_use(), base + 1);
    // `ixterm` drops the dying PD's uses (`bdos/proc.c:126-132`).
    c.pterm();
    assert_eq!(curdir::uses_of(2, "\\SUB"), 1);
    p.pterm();
    assert_eq!(curdir::uses_of(2, "\\SUB"), 0, "the slot is free again");
    assert_eq!(curdir::slots_in_use(), base);
    drop(c);
    drop(p);
    teardown();
}

#[test]
fn stand_ins_give_their_uses_back_when_they_end() {
    let (_g, _h) = setup();
    let base = curdir::slots_in_use();
    process::set_current(core::ptr::null_mut());
    assert_eq!(dsetpath("\\SUB"), E_OK, "accdesk in \\SUB");
    standin::enter_emucon();
    assert_eq!(curdir::uses_of(2, "\\SUB"), 2, "EmuCON copied accdesk's");
    standin::leave_emucon();
    assert_eq!(curdir::uses_of(2, "\\SUB"), 1, "EmuCON's Pterm0");
    standin::leave_accdesk();
    assert_eq!(curdir::uses_of(2, "\\SUB"), 0, "accdesk's Pterm0");
    assert_eq!(curdir::slots_in_use(), base);
    standin::enter_accdesk();
    assert_eq!(dgetpath(0), "", "a new accdesk starts from the AES PD (root)");
    teardown();
}

#[test]
fn a_byte_naming_no_live_slot_reads_as_the_root_and_is_not_inherited() {
    let (_g, _h) = setup();
    let base = curdir::slots_in_use();
    let p = Proc::kernel_started();
    p.run();
    // A program scribbles its own p_curdir[2] (PD+0x42) — EmuTOS would follow
    // whatever slot the byte names; NewTOS reads a byte that names no live
    // slot of that drive as the root.
    // SAFETY: the box is alive.
    unsafe { *(p.bp as *mut u8).add(0x40 + 2) = 33 };
    assert_eq!(dgetpath(0), "");
    let c = Proc::child_of(&p);
    // SAFETY: as above.
    assert_eq!(unsafe { *(c.bp as *const u8).add(0x42) }, 0, "copied as the root");
    c.pterm();
    p.pterm();
    assert_eq!(curdir::slots_in_use(), base, "no use was taken or dropped for it");
    drop(c);
    drop(p);
    teardown();
}

#[test]
fn a_program_writing_its_own_pd_0x37_changes_its_dgetdrv() {
    let (_g, _h) = setup();
    let p = Proc::kernel_started();
    p.run();
    // `Dgetdrv` is `return run->p_curdrv` (`bdos/fsmain.c:438-441`) — the
    // byte, not a copy of it.
    // SAFETY: the box is alive.
    unsafe { *(p.bp as *mut u8).add(0x37) = 4 };
    assert_eq!(dgetdrv(), 4);
    drop(p);
    teardown();
}

#[test]
fn a_full_table_is_recounted_before_dsetpath_gives_up() {
    let (_g, _h) = setup();
    // Directories D00..D44 on C: (fixture of this test only).
    let names: Vec<String> = (0..45).map(|i| format!("C:\\D{i:02}")).collect();
    for n in &names {
        let c = cpath(n);
        let rc = gemdos1(DCREATE, c.as_ptr() as u64);
        assert!(rc == E_OK || rc == -36, "Dcreate {n}: {rc}");
    }
    let base = curdir::slots_in_use();
    // A program that keeps overwriting its own p_curdir[2] (PD+0x42) with 0
    // after each Dsetpath: every slot it took keeps a use nobody names — the
    // stale state a PD that vanished without `ixterm` leaves behind too.
    let p = Proc::kernel_started();
    p.run();
    let mut ok = 0;
    for n in &names {
        if dsetpath(n) != E_OK {
            break;
        }
        ok += 1;
        // SAFETY: the box is alive.
        unsafe { *(p.bp as *mut u8).add(0x42) = 0 };
    }
    assert!(ok >= curdir::NCURDIR - 1 - base, "{ok} directories before the table was full");
    // Every slot is free again after the recount the first full `incr` did
    // (nobody names them), so all 45 succeeded and only the last is in use.
    assert_eq!(ok, names.len(), "the recount freed the unnamed uses");
    assert_eq!(dgetpath(0), "", "the byte is 0 = root");
    assert_eq!(dsetpath("C:\\D44"), E_OK);
    p.pterm();
    // The slots taken after the recount and orphaned again stay counted until
    // the next full table; recount once more by filling it.
    let q = Proc::kernel_started();
    q.run();
    for n in &names {
        assert_eq!(dsetpath(n), E_OK, "{n}: a full table of unnamed uses never blocks");
    }
    q.pterm();
    drop(q);
    drop(p);
    teardown();
}
