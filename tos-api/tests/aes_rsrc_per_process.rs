//! A39, stage 1 — **the resource state is kept per AES process.**
//!
//! EmuTOS keeps no resource descriptor in the AES at all: the three words
//! `ap_ptree`/`ap_rscmem`/`ap_rsclen` in the application's own `AESGLOBAL`
//! *are* the descriptor, and `rs_sglobe(pglobal)` re-reads them out of the
//! caller's array on every `rs_free`/`rs_gaddr`/`rs_saddr`
//! (`references/emutos/aes/gemrslib.c:673-679`, calls `:690`, `:717`, `:732`).
//! Two AES processes with separate `global[]` arrays therefore never touch
//! each other's resource; a child that loads and frees leaves its parent's
//! load exactly as it was. `rs_load` frees nothing (`:786-788`), `ap_exit`
//! touches nothing (`aes/gemaplib.c:251-261`), and at process end the blob
//! dies with the process's GEMDOS memory (`bdos/proc.c:136`,
//! `free_all_owned`).
//!
//! NewTOS held that state in **one kernel static for every process**, so a
//! child's `rsrc_load`/`rsrc_free` replaced and then erased the parent's —
//! measured in H05 (`TASK/H05-STATUS-DONE.MD` § 4): after the inner
//! TeraDesk's `rsrc_load`/`rsrc_free`, all 36 `rsrc_gaddr` of the outer one
//! answered 0. Stage 1 of the fix keys the descriptor by the calling AES
//! pid, one list per process, and selects a process's **most recently
//! loaded** entry; stage 2 (after A60) will select through the caller's
//! `ap_rscmem` instead (spec `docs/api/aes/resource-fileselector.md`
//! § `rsrc_load`, A39 callout, items 1–4; analysis
//! `docs/analysis/rsrc-per-app-state-2026-09-19.md` § 4.3).
//!
//! ## How this binary stands in for two processes
//!
//! The AES pid of a caller is its GEMDOS slot + 1 (`appl::current_pid`,
//! `aes::pid_of_slot`), read off the running basepage `RUN`. A test can make
//! a slot "run" the way the loader's own unit tests do
//! (`src/bdos/process/loader.rs`, `mod tests`): claim it with
//! `allocate_slot`, publish a page of its own as `basepage_kva`, and switch
//! `RUN` to it with `set_current`. `rsrc_gaddr`/`rsrc_free`/`appl_init`
//! then go through the real LP64 dispatcher as that pid.
//!
//! What the host **cannot** drive is the EL0 *load* path
//! (`load_into_user`): it takes a BDOS-pool block — a physical address in
//! `Long`, which no host allocation can be — and reaches it through the
//! kernel high-half alias. `rsrc_load` through the dispatcher would take that
//! path as soon as `RUN` is set, and fail for want of a pool, not for any
//! reason under test. Each load here therefore goes through
//! `resource::relocate_and_register`, the in-place relocation + registration
//! that `rsrc_load`'s in-kernel branch calls (`kernel/src/selftest_objc.rs`
//! uses the same entry point), on a buffer the test owns. It registers for
//! the current pid exactly as the dispatcher's `rsrc_load` does; nothing about
//! the descriptor is faked. The `rsrc_load` dispatcher path itself, including
//! the `AESGLOBAL` report, is covered by `aes_rsrc_global_fields.rs` and
//! `aes_rsrc_load_path.rs`.

use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};

use tos_api::bdos::process::{self, Abi, PROCESS_TABLE};
use tos_types::BasePage;

/// The AES's own file-selector resource — a genuine `NRSC` v7 blob.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

const R_TREE: i16 = 0;
const R_OBJECT: i16 = 1;

/// The process table, `RUN` and the AES descriptor tables are process-wide
/// statics with no locking (single-CPU trap path); the tests in this binary
/// must not overlap.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ── a process slot the host can "run" ─────────────────────────────────────

/// A claimed GEMDOS process slot with a basepage page of its own. Its AES
/// pid is `slot + 1`. Released (`free_slot`) on drop, so a failing test does
/// not leave the table full for the next one.
struct Proc {
    slot: usize,
    page: *mut [u8; 4096],
}

impl Proc {
    fn new() -> Self {
        let slot = process::allocate_slot().expect("a free process slot");
        let page = Box::into_raw(Box::new([0u8; 4096]));
        PROCESS_TABLE[slot]
            .basepage_kva
            .store(page.cast::<BasePage>(), Ordering::Release);
        Proc { slot, page }
    }

    fn pid(&self) -> i16 {
        tos_api::aes::pid_of_slot(self.slot)
    }

    fn bp(&self) -> *mut BasePage {
        self.page.cast::<BasePage>()
    }

    /// Make this the running process: every AES call now carries its pid.
    fn run(&self) {
        process::set_current(self.bp());
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if process::current() == self.bp() {
            process::set_current(std::ptr::null_mut());
        }
        // The slot may already have been released by a `Pterm` in the test;
        // `free_slot` on a free slot only re-zeroes it.
        process::free_slot(self.slot);
        // SAFETY: `page` came from `Box::into_raw` above and is not aliased
        // once the slot no longer names it.
        unsafe { drop(Box::from_raw(self.page)) };
    }
}

// ── a resource the test owns ──────────────────────────────────────────────

/// A writable, 8-byte-aligned copy of the blob, registered for the **current**
/// pid the way `rsrc_load`'s in-kernel branch registers it. The buffer must
/// outlive every later `rsrc_gaddr` into it, hence it is kept.
struct Blob {
    words: Vec<u64>,
}

impl Blob {
    fn load_for_current_pid() -> Self {
        let mut words = vec![0u64; RSC.len().div_ceil(8) + 1];
        let p = words.as_mut_ptr() as *mut u8;
        // SAFETY: `words` holds at least `RSC.len()` bytes.
        unsafe { std::ptr::copy_nonoverlapping(RSC.as_ptr(), p, RSC.len()) };
        // SAFETY: `p` addresses `RSC.len()` writable bytes that stay alive as
        // long as this `Blob` does.
        let ok = unsafe { tos_api::aes::resource::relocate_and_register(p, RSC.len()) };
        assert!(ok, "the blob must relocate and register");
        Blob { words }
    }

    fn base(&self) -> u64 {
        self.words.as_ptr() as u64
    }

    fn contains(&self, addr: u64) -> bool {
        addr >= self.base() && addr < self.base() + RSC.len() as u64
    }
}

// ── AES calls through the real dispatcher ─────────────────────────────────

fn call(op: i16, n_intin: i16, n_intout: i16, intin: [i16; 4]) -> (i16, u64) {
    let mut control = [op, n_intin, n_intout, 0, 0];
    let mut global = [0i16; 16];
    let mut intin = intin;
    let mut intout = [0i16; 8];
    let mut addrin = [0u64; 2];
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
    (intout[0], addrout[0])
}

/// `rsrc_gaddr(rtype, index)` → (return code, address).
fn gaddr(rtype: i16, index: i16) -> (i16, u64) {
    call(112, 2, 1, [rtype, index, 0, 0])
}

/// `rsrc_free()` → return code.
fn rsrc_free() -> i16 {
    call(111, 0, 1, [0; 4]).0
}

/// `appl_init()` → the AES id.
fn appl_init() -> i16 {
    call(10, 0, 1, [0; 4]).0
}

/// `appl_exit()` → return code.
fn appl_exit() -> i16 {
    call(19, 0, 1, [0; 4]).0
}

/// `Pterm` of the running process — the real `terminate` path, followed by
/// the handler's `release_slot`. On the host the pool sweep finds no pool
/// (`free_all_owned` → 0) and the parent frame is a marker; what is under
/// test is the AES hook that runs in between.
fn pterm(p: &Proc) {
    // `terminate` refuses a slot whose parent context was never captured; a
    // launched process always has one, so mark it as the loader would.
    PROCESS_TABLE[p.slot].parent_frame.elr().store(1, Ordering::Release);
    // SAFETY: single-threaded under `LOCK`; `RUN` is `p`'s basepage.
    let resume = unsafe { process::pterm::terminate(0) }.expect("Pterm of the running process");
    assert_eq!(resume.parent_frame_slot, p.slot);
    // SAFETY: the frame snapshot is not needed on the host.
    unsafe { process::pterm::release_slot(p.slot) };
}

// ── (a) a child's load + free leaves the parent alone ─────────────────────

/// H05 F2 / analysis § 6 item 1 — the defect itself. pid 1 loads; pid 2
/// loads and frees; pid 1's `rsrc_gaddr(R_TREE, 0)` must still answer 1 and
/// the **same** address, and its own `rsrc_free` must still find the load.
///
/// EmuTOS: `rs_sglobe(pglobal)` binds every call to the caller's own
/// `ap_rscmem` (`gemrslib.c:677-678`); pid 2's `dos_free` (`:699`) frees the
/// blob *its* array names.
#[test]
fn a_childs_load_and_free_leaves_the_parents_resource_untouched() {
    let _g = lock();
    let parent = Proc::new();
    let child = Proc::new();
    assert_ne!(parent.pid(), child.pid());

    parent.run();
    let parent_blob = Blob::load_for_current_pid();
    let (ok, tree_before) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1, "the parent's load must resolve");
    assert!(parent_blob.contains(tree_before), "tree 0 lies in the parent's blob");

    child.run();
    let child_blob = Blob::load_for_current_pid();
    let (ok, child_tree) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1, "the child's load must resolve");
    assert!(child_blob.contains(child_tree), "the child resolves in its own blob");
    assert_eq!(rsrc_free(), 1, "the child frees its own load");
    let (ok, addr) = gaddr(R_TREE, 0);
    assert_eq!((ok, addr), (0, 0), "after its rsrc_free the child has nothing loaded");

    parent.run();
    let (ok, tree_after) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1, "the parent's rsrc_gaddr must still find its resource");
    assert_eq!(
        tree_after, tree_before,
        "the parent's tree 0 must be the same address as before the child ran"
    );
    let (ok, obj) = gaddr(R_OBJECT, 0);
    assert_eq!(ok, 1);
    assert!(parent_blob.contains(obj), "every selector resolves in the parent's blob");
    assert_eq!(rsrc_free(), 1, "the parent's own rsrc_free finds its load");
    assert_eq!(rsrc_free(), 0, "a second rsrc_free finds nothing (gemrslib.c:702-704)");
}

// ── (b) process end frees only the dying process's entries ────────────────

/// Analysis § 6 item 6 (R6). pid 2 loads, calls **no** `rsrc_free`, and ends
/// through the real `Pterm` path. pid 1's resource is unchanged, and a new
/// program that lands in the same slot — same pid, and `appl_init` reuses the
/// descriptor (`pd::open`, A29) — sees "nothing loaded".
///
/// EmuTOS: `ixterm` → `free_all_owned` frees the blob with the process
/// (`bdos/proc.c:136`); the `AESGLOBAL` dies with its memory, so the next
/// process starts with `ap_rscmem == NULL` (`rs_free` → 0, `gemrslib.c:685`).
#[test]
fn process_end_drops_only_the_dying_processs_entries() {
    let _g = lock();
    let parent = Proc::new();
    let child = Proc::new();
    let child_slot = child.slot;

    parent.run();
    let _parent_blob = Blob::load_for_current_pid();
    let (ok, tree_before) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1);

    child.run();
    let _child_blob = Blob::load_for_current_pid();
    assert_eq!(appl_init(), child.pid(), "the child registers as itself");
    assert_eq!(gaddr(R_TREE, 0).0, 1, "the child's load resolves");
    // No rsrc_free, no appl_exit: the program just ends.
    pterm(&child);
    drop(child);

    // The next program in the same slot: same pid, reused descriptor.
    let next = Proc::new();
    assert_eq!(next.slot, child_slot, "the slot is reused, so the pid is too");
    next.run();
    assert_eq!(appl_init(), next.pid());
    let (ok, addr) = gaddr(R_TREE, 0);
    assert_eq!(
        (ok, addr),
        (0, 0),
        "a new program in the dead process's slot must inherit no resource"
    );
    assert_eq!(rsrc_free(), 0, "…and has nothing to free");
    assert_eq!(appl_exit(), 1);

    parent.run();
    let (ok, tree_after) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1, "the parent's resource survives the child's Pterm");
    assert_eq!(tree_after, tree_before);
    assert_eq!(rsrc_free(), 1);
}

/// The other half of R6: `appl_exit` leaves the resource alone (EmuTOS
/// `ap_exit`, `aes/gemaplib.c:251-261`, touches only the queue and
/// `AP_OPEN`). XaAES frees the client's whole list there
/// (`xa_appl.c:745-750`); that deviation is not adopted.
#[test]
fn appl_exit_does_not_touch_the_resource() {
    let _g = lock();
    let app = Proc::new();
    app.run();
    assert_eq!(appl_init(), app.pid());
    let _blob = Blob::load_for_current_pid();
    let (ok, tree) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1);
    assert_eq!(appl_exit(), 1);
    assert_eq!(gaddr(R_TREE, 0), (1, tree), "still loaded after appl_exit");
    assert_eq!(rsrc_free(), 1, "still ours to free after appl_exit");
}

// ── (c) two loads by one process: the second is selected ──────────────────

/// Analysis § 6 item 2 (R1/R5 in stage 1). A second `rsrc_load` frees nothing
/// (`rs_load` never does, `gemrslib.c:786-788`) and becomes the process's
/// selected resource — the one `rsrc_gaddr` resolves in and `rsrc_free`
/// frees. After that `rsrc_free` the process has **no** selected resource:
/// the first load stays loaded but unselected, exactly as on EmuTOS, where
/// `ap_rscmem` keeps naming the freed second blob (`rs_free` writes none of
/// the three words), a second `rsrc_free` fails (`dos_free` of a freed block,
/// `:699-704`), and the first blob is reachable only by restoring the words —
/// the selector stage 1 does not have. It dies with the process (R6).
#[test]
fn a_second_load_is_selected_and_freed_the_first_stays_loaded_unselected() {
    let _g = lock();
    let app = Proc::new();
    app.run();

    let first = Blob::load_for_current_pid();
    let (ok, tree_first) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1);
    assert!(first.contains(tree_first));

    let second = Blob::load_for_current_pid();
    let (ok, tree_second) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1, "the second load resolves");
    assert!(second.contains(tree_second), "rsrc_gaddr resolves in the second blob");
    assert!(!first.contains(tree_second), "…and not in the first");

    assert_eq!(rsrc_free(), 1, "rsrc_free frees the selected (second) load");
    let (ok, addr) = gaddr(R_TREE, 0);
    assert_eq!(
        (ok, addr),
        (0, 0),
        "after freeing the selected load nothing is selected — the first \
         blob stays loaded but is not reachable through the AES in stage 1"
    );
    assert_eq!(rsrc_free(), 0, "nothing selected, nothing to free");

    // The first blob is still the process's: it goes with the process (R6),
    // and a fresh tenant of the slot starts clean.
    let slot = app.slot;
    pterm(&app);
    drop(app);
    let next = Proc::new();
    assert_eq!(next.slot, slot);
    next.run();
    assert_eq!(gaddr(R_TREE, 0), (0, 0), "the unselected first blob died with its process");
    drop(first);
    drop(second);
}

// ── (R7) no `appl_init` needed ────────────────────────────────────────────

/// `rsrc_load` needs no prior `appl_init` (`crysbind` checks nothing,
/// `gemsuper.c:363-365`): a process that never opened an AES descriptor
/// loads, resolves and frees like any other.
#[test]
fn a_process_without_appl_init_loads_resolves_and_frees() {
    let _g = lock();
    let app = Proc::new();
    app.run();
    let blob = Blob::load_for_current_pid();
    let (ok, tree) = gaddr(R_TREE, 0);
    assert_eq!(ok, 1, "rsrc_gaddr without appl_init");
    assert!(blob.contains(tree));
    assert_eq!(rsrc_free(), 1, "rsrc_free without appl_init");
}
