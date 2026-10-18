//! The process descriptors EmuTOS has and NewTOS has no process for —
//! `initial_basepage`, the AES PD, the **accdesk PD** and the **EmuCON PD** —
//! as **stand-ins** (TASK/PPARENT "Way A", 2026-10-06; extended by TASK/PROC03
//! stage 1, user decision 2026-10-18 "alles EmuTOS-treu").
//!
//! Spec: `docs/api/gemdos/process/_parent-and-environment.md` (§1 the chain,
//! §2 the field contents, §3 the environments, §4.4 as built) and
//! `docs/api/gemdos/process/_run-ownership-curdir.md` §6.4 (the decisions
//! D-1…D-4 for the fields the source defines but NewTOS cannot represent).
//!
//! EmuTOS writes `p_parent` only in `proc_go`, `p->p_parent = run`
//! (`bdos/proc.c:541`), and gives every process an environment, a copy of
//! `run->p_env` unless the caller names one (`alloc_env`, `bdos/proc.c:413-432`).
//! `run` over time (`_run-ownership-curdir.md` §1.2):
//!
//! * `initial_basepage` while the BIOS runs the AUTO folder
//!   (`bdos/bdosmain.c:78,337-342`, `bios/bios.c:685-695`);
//! * the **AES PD** while the AES initialises (`bios/bios.c:855-866`,
//!   `aes/geminit.c:753-894`);
//! * the **accdesk PD** from `aes_run_rom_program(accdesk_start)`
//!   (`aes/geminit.c:894`, `aes/gemshlib.c:570-581`) for the whole of
//!   `run_accs_and_desktop` (`aes/geminit.c:654-750`): the accessories are
//!   loaded under it and the desktop, the `#Z` program and every program the
//!   desktop asks for get it as their parent (`aes/gemshlib.c:600-611,645`);
//! * the **EmuCON PD** while the console runs (`aes/gemshlib.c:614-620`),
//!   parent of what it `Pexec`s (`cli/cmdexec.c:52`).
//!
//! NewTOS runs the AES, the shell loop and EmuCON in the kernel; `RUN` is
//! `null` while kernel code is on the stack. This module provides the PDs that
//! `run` would be there: real, readable basepages with EmuTOS's field
//! contents, in one kernel page that is published read-only to every process
//! at VA = PA (`usermem::expose`, `PROT_PR`, ADR-023), so a program can follow
//! its `p_parent` chain in every protection mode. The kernel writes the page
//! (the accdesk and EmuCON PDs are rebuilt at every start, the AES PD's
//! `p_cmdlin` is `ad_stail`, D-4, and the current drive and directories of
//! whichever stand-in is the kernel's `run`, TASK/PROC04 point 2); programs
//! cannot. They are **never** the running process: `RUN`, `os_run`, memory
//! owner tags, `parent_slot` and the `Pterm` return path of a process do not
//! see them (owner tags and `os_run` are TASK/PROC03 B5, not built).
//!
//! They **are** `run` for the current drive and directory while kernel code
//! is on the stack (`_run-ownership-curdir.md` §4.4, spec finding §6.2.4:
//! their drive and directories are mutable state): the AES shell's
//! `sh_chdef` moves the accdesk PD's directory before each start, EmuCON's
//! `cd`/`d:` the EmuCON PD's, the restart's `dos_sdrv(bootdev)` the AES PD's
//! ([`kernel_run_kva`], `crate::bdos::fs::curdir`).
//!
//! What uses them: [`super::pexec::set_parent`] and
//! [`super::pexec::set_kernel_parent`] (`p_parent` of a program a kernel
//! caller starts), the loader's `EnvSpec::Inherit` and handle inheritance with
//! no running process ([`kernel_run_env`], [`kernel_run_uft`]), the file
//! table's owner of kernel-context opens, and `shel_envrn` (`ad_envrn` is the
//! AES PD's `p_env`, `aes/gemstart.S:78-96` — [`aes_env`]).

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tos_types::{BasePage, Long, NUMSTD, PDCLSIZE, PF_STANDARD};

use super::kva_to_user_va;

/// `ENV_SIZE` (`bios/bios.c:77`): "sufficient for standard PATH=^X:\^^
/// (^=nul byte)" — the size of `default_env`, and (11 counted bytes rounded
/// up to even, `bdos/proc.c:421`) of the AES PD's copy of it. The accdesk and
/// EmuCON copies have the same size (`alloc_env` of the same bytes).
pub const AES_ENV_SIZE: usize = 12;

/// `default_handle` (`bdos/console.c:60-68`): `H_Console`, `H_Console`,
/// `H_Aux`, `H_Print`, `H_Console`, `H_Console` (`bdos/console.h:23-25`) —
/// what `stdhdl_init` puts into `initial_basepage.p_uft`
/// (`bdos/console.c:107-112`) and `init_pd_files` copies into every PD the
/// AES and its shell create (`bdos/proc.c:390-398`, a handle `<= 0` is copied
/// as is). `Fclose` of a standard handle reverts it to this
/// (`get_default_handle`, `bdos/console.c:132-135`, `bdos/fsopnclo.c:453`).
pub const DEFAULT_HANDLE: [i8; NUMSTD] = [-1, -1, -2, -3, -1, -1];

/// Size of one PD (`sizeof(PD)`, 0x100).
const PD_SIZE: usize = core::mem::size_of::<BasePage>();

/// Offsets in the page. The first four are those of TASK/PPARENT and stay
/// where they were; the accdesk and EmuCON PDs and their environments follow.
const AES_OFFSET: usize = 0x100;
const DOUBLE_NUL_OFFSET: usize = 0x200;
const AES_ENV_OFFSET: usize = 0x210;
const ACCDESK_OFFSET: usize = 0x300;
const EMUCON_OFFSET: usize = 0x400;
const ACCDESK_ENV_OFFSET: usize = 0x500;
const EMUCON_ENV_OFFSET: usize = 0x510;

/// The page: the four PDs and the environments their `p_env` name, and
/// nothing else — the whole page becomes readable to every process.
#[repr(C, align(4096))]
struct Page {
    /// `initial_basepage` (`bdos/bdosmain.c:78`), +0x000.
    initial: BasePage,
    /// The AES PD (`bios/bios.c:862`), +0x100.
    aes: BasePage,
    /// `double_nul` (`bdos/bdosmain.c:81`), +0x200.
    double_nul: [u8; 2],
    _pad0: [u8; AES_ENV_OFFSET - DOUBLE_NUL_OFFSET - 2],
    /// The AES PD's environment, a copy of `default_env`
    /// (`bios/bios.c:626-636`) — `ad_envrn`, +0x210.
    aes_env: [u8; AES_ENV_SIZE],
    _pad1: [u8; ACCDESK_OFFSET - AES_ENV_OFFSET - AES_ENV_SIZE],
    /// The accdesk PD (`aes/gemshlib.c:570-581` with `accdesk_start`), +0x300.
    accdesk: BasePage,
    /// The EmuCON PD (`aes/gemshlib.c:614-620`, `coma_start`), +0x400.
    emucon: BasePage,
    /// The accdesk PD's environment, a copy of the AES PD's, +0x500.
    accdesk_env: [u8; AES_ENV_SIZE],
    _pad2: [u8; EMUCON_ENV_OFFSET - ACCDESK_ENV_OFFSET - AES_ENV_SIZE],
    /// The EmuCON PD's environment, a copy of its parent's, +0x510.
    emucon_env: [u8; AES_ENV_SIZE],
}

const _: () = assert!(core::mem::offset_of!(Page, aes) == AES_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, double_nul) == DOUBLE_NUL_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, aes_env) == AES_ENV_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, accdesk) == ACCDESK_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, emucon) == EMUCON_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, accdesk_env) == ACCDESK_ENV_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, emucon_env) == EMUCON_ENV_OFFSET);
const _: () = assert!(core::mem::size_of::<Page>() <= 4096);

struct Cell(UnsafeCell<Page>);
// SAFETY: written by [`init`] on the boot CPU before any program runs, and
// afterwards only from kernel context on the single CPU (the shell loop and
// EmuCON entering/leaving, the AES writing `ad_stail`) — never while a trap
// handler holds a reference into the page; programs only read it through
// their read-only mapping.
unsafe impl Sync for Cell {}

static PAGE: Cell = Cell(UnsafeCell::new(Page {
    // Before [`init`] the kernel's `run` is already `initial_basepage` (EmuTOS
    // sets `run` in `osinit_after_xmaddalt`, before the BIOS knows `bootdev`);
    // its drive is `C:` — the first hard-disk drive, the default the current
    // drive had before it moved into the PD — until `init` writes `bootdev`.
    initial: BasePage { p_curdrv: crate::bdos::fs::curdir::DEFAULT_DRIVE, ..BasePage::zeroed() },
    aes: BasePage::zeroed(),
    double_nul: [0, 0],
    _pad0: [0; AES_ENV_OFFSET - DOUBLE_NUL_OFFSET - 2],
    aes_env: [0; AES_ENV_SIZE],
    _pad1: [0; ACCDESK_OFFSET - AES_ENV_OFFSET - AES_ENV_SIZE],
    accdesk: BasePage::zeroed(),
    emucon: BasePage::zeroed(),
    accdesk_env: [0; AES_ENV_SIZE],
    _pad2: [0; EMUCON_ENV_OFFSET - ACCDESK_ENV_OFFSET - AES_ENV_SIZE],
    emucon_env: [0; AES_ENV_SIZE],
}));

/// The page is built ([`init`]).
static READY: AtomicBool = AtomicBool::new(false);

/// Which stand-in is `run` while kernel code is on the stack.
const RUN_INITIAL: u8 = 0;
const RUN_AES: u8 = 1;
const RUN_ACCDESK: u8 = 2;
const RUN_EMUCON: u8 = 3;
static KERNEL_RUN: AtomicU8 = AtomicU8::new(RUN_INITIAL);
/// `run` before [`enter_emucon`] — the EmuCON PD's parent, which `Pterm0`
/// makes `run` again (`cli/cmdasm.S:48-49`, `bdos/proc.c:609`).
static EMUCON_PARENT: AtomicU8 = AtomicU8::new(RUN_ACCDESK);

/// The page offset of stand-in `which`'s PD.
const fn pd_offset(which: u8) -> usize {
    match which {
        RUN_AES => AES_OFFSET,
        RUN_ACCDESK => ACCDESK_OFFSET,
        RUN_EMUCON => EMUCON_OFFSET,
        _ => 0,
    }
}

/// The user address (= physical address, VA = PA) of the page's byte `off`.
fn user_va(off: usize) -> u64 {
    kva_to_user_va(PAGE.0.get() as u64 + off as u64)
}

/// Kernel pointer to stand-in `which`'s PD.
fn pd_ptr(which: u8) -> *mut BasePage {
    // SAFETY: in bounds of the static page (`pd_offset` names a PD field).
    unsafe { (PAGE.0.get() as *mut u8).add(pd_offset(which)) as *mut BasePage }
}

/// The bytes the environment of stand-in `which` holds, up to and including
/// its double NUL (EmuTOS `envsize`, `bdos/proc.c:150-159`).
fn env_of(which: u8) -> &'static [u8] {
    // SAFETY: shared read of a field of the static page; written only from
    // kernel context while no such reference is alive (`Cell` contract).
    let p = unsafe { &*PAGE.0.get() };
    match which {
        RUN_INITIAL => &p.double_nul,
        RUN_AES => env_image(&p.aes_env),
        RUN_ACCDESK => env_image(&p.accdesk_env),
        _ => env_image(&p.emucon_env),
    }
}

/// Build all PDs that exist at boot — `initial_basepage` and the AES PD —
/// and the AES environment for boot drive `bootdev` (0 = `A:`), and publish
/// the page read-only to every process. Returns the page's user address, or
/// `None` when it could not be published (no program could then read the
/// chain; the kernel reports it). Idempotent in content: a second call
/// rebuilds the same page (the accdesk and EmuCON PDs are cleared; they are
/// built when they start, [`enter_accdesk`], [`enter_emucon`]).
///
/// What EmuTOS does at these points:
///
/// * `osinit_after_xmaddalt` (`bdos/bdosmain.c:337-347`): `run =
///   &initial_basepage; run->p_flags = PF_STANDARD; run->p_env = double_nul;
///   stdhdl_init()` — the six default handles.
/// * `Dsetdrv(bootdev)` (`bios/bios.c:827`) — `initial_basepage.p_curdrv`.
/// * `init_default_environment()` (`bios/bios.c:626-636,828`).
/// * `Pexec(PE_BASEPAGEFLAGS, PF_STANDARD, "", default_env)` +
///   `p_tbase = exec_os` + `Pexec(PE_GO)` (`bios/bios.c:862-865`): the AES PD
///   per `init_pd_fields`/`init_pd_files`/`proc_go` (`bdos/proc.c:370-409,
///   541`).
///
/// # Safety
///
/// Boot CPU, before any program is started (nothing may hold a reference into
/// the page while it is written); the BDOS pools and the user tables exist
/// (the publication needs them).
pub unsafe fn init(bootdev: u16) -> Option<u64> {
    // SAFETY: caller contract — exclusive access during boot.
    let p = unsafe { &mut *PAGE.0.get() };
    let initial_va = user_va(0);
    let double_nul_va = user_va(DOUBLE_NUL_OFFSET);
    let aes_env_va = user_va(AES_ENV_OFFSET);

    // `double_nul` (`bdos/bdosmain.c:81`).
    p.double_nul = [0, 0];

    // `default_env` (`bios/bios.c:626-636`): "PATH=" NUL, "A:\" with the
    // drive letter fixed up (`*p += bootdev`), NUL — and the rest of the
    // `ENV_SIZE` static zero. `alloc_env` copies `(envsize + 1) & ~1` = 12
    // bytes of it into the AES PD's block (`bdos/proc.c:421-429`).
    let mut env = [0u8; AES_ENV_SIZE];
    env[..5].copy_from_slice(b"PATH=");
    env[6] = b'A' + (bootdev as u8 & 0x1F);
    env[7] = b':';
    env[8] = b'\\';
    p.aes_env = env;

    // `initial_basepage` (`bdos/bdosmain.c:78`, static → all zero) with what
    // `osinit_after_xmaddalt`, `stdhdl_init` and `Dsetdrv(bootdev)` write. It
    // is the same object EmuTOS's `run` was before: its directories (none,
    // at boot) stay.
    let mut ib = BasePage::zeroed();
    ib.p_curdir = p.initial.p_curdir;
    ib.p_flags = PF_STANDARD; // bdosmain.c:340
    ib.p_env = double_nul_va as Long; // bdosmain.c:341
    ib.p_uft = DEFAULT_HANDLE; // console.c:107-112
    ib.p_curdrv = bootdev as u8; // bios.c:827 (xsetdrv: run->p_curdrv)
    p.initial = ib;

    // The AES PD: `init_pd_fields` (`bdos/proc.c:367-384`) — zeroed,
    // `p_lowtpa = p`, `p_hitpa = p + max`, `p_xdta = &p_cmdlin`, `p_env`, the
    // empty tail; `p_flags = PF_STANDARD` (`:234`); `init_pd_files` (`:388-409`)
    // — the handles, the current drive of `run` = `initial_basepage`;
    // `p_tbase = exec_os`, `p_tlen = p_dlen = p_blen = 0` (`bios/bios.c:
    // 863-864`); `proc_go`: `p_parent = run` (`bdos/proc.c:541`). The fields
    // NewTOS cannot represent follow the decisions of `_run-ownership-curdir.md`
    // §6.4 (D-1 `p_tbase` 0, D-2 `p_hitpa` p + 0x100) — see [`rom_pd`].
    // SAFETY: `p.aes` is a live PD of the page; its directory uses go first.
    unsafe { crate::bdos::fs::curdir::release(&p.aes) };
    p.aes = rom_pd(AES_OFFSET, initial_va, aes_env_va, &p.initial);

    // Not started yet: built by `enter_accdesk` / `enter_emucon`.
    // SAFETY: live PDs of the page; their directory uses go with them.
    unsafe {
        crate::bdos::fs::curdir::release(&p.accdesk);
        crate::bdos::fs::curdir::release(&p.emucon);
    }
    p.accdesk = BasePage::zeroed();
    p.emucon = BasePage::zeroed();
    p.accdesk_env = [0; AES_ENV_SIZE];
    p.emucon_env = [0; AES_ENV_SIZE];

    READY.store(true, Ordering::Release);

    // Publish the page read-only at VA = PA (ADR-023). `expose` answers the
    // same address for every process; the slot only matters for an uncached
    // publication, which this is not.
    #[cfg(all(target_arch = "aarch64", target_os = "none"))]
    {
        // SAFETY: caller contract (boot, lock); the page is a `'static`,
        // page-aligned kernel object that holds nothing but the stand-ins.
        let published = unsafe {
            super::usermem::expose(
                0,
                PAGE.0.get() as *const u8,
                core::mem::size_of::<Page>(),
                tos_types::ProtMode::Pr,
            )
        };
        published.filter(|&va| va == initial_va)
    }
    // On the host user memory is host memory (`usermem::resolve_kva`): there
    // are no user tables to enter the page into.
    #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
    {
        Some(initial_va)
    }
}

/// A PD as `Pexec(PE_BASEPAGEFLAGS, PF_STANDARD, "", env)` creates it and
/// `proc_go` starts it — the AES PD (`bios/bios.c:861-865`), the accdesk and
/// the EmuCON PD (`aes_run_rom_program`, `aes/gemshlib.c:570-581`) — at page
/// offset `off`, with parent `parent_va`, environment block `env_va`, and the
/// handles and current drive of the creating `run`, `creator`.
///
/// * `init_pd_fields` (`bdos/proc.c:367-384`): zeroed, `p_lowtpa = p`,
///   `p_xdta = &p_cmdlin` (D-3: EmuTOS's later AES-internal `dos_sdta` has
///   no counterpart — NewTOS's AES keeps its DTAs in kernel buffers), the
///   empty tail (`p_cmdlin` all zero);
/// * `p_flags = PF_STANDARD` (`:234`);
/// * `init_pd_files` (`:388-409`): `p_uft`, `p_curdir[]` (one `dirtbl` use
///   each) and `p_curdrv` of `run` (TASK/PROC04 point 2);
/// * `proc_go`: `p_parent = run` (`:541`);
/// * `p_tbase` (the entry `exec_os`/`accdesk_start`/`coma_start`) is **0**
///   — decision D-1, `_run-ownership-curdir.md` §6.4: the code is kernel
///   code, at no address of any program's map; a non-zero value would be
///   invented or a kernel address no program can use;
/// * `p_hitpa` is **`p + 0x100`** — decision D-2: EmuTOS's `p + max` is the
///   size of the largest free block at creation, which the code `Mshrink`s
///   to the 0x100-byte PD at once without updating the field
///   (`aes/gemstart.S:59-69,143-152`, `cli/cmdasm.S:26-37`); `p + 0x100` is
///   what the block really is. The PD stays a kernel page outside the pools,
///   so its address can never be a pool block's (see D-2 for stage 3).
fn rom_pd(off: usize, parent_va: u64, env_va: u64, creator: &BasePage) -> BasePage {
    let va = user_va(off);
    let mut pd = BasePage::zeroed();
    pd.p_lowtpa = va as Long;
    pd.p_hitpa = (va + PD_SIZE as u64) as Long;
    pd.p_xdta = (va + core::mem::offset_of!(BasePage, p_cmdlin) as u64) as Long;
    pd.p_parent = parent_va as Long;
    pd.p_flags = PF_STANDARD;
    pd.p_env = env_va as Long;
    pd.p_uft = creator.p_uft;
    // SAFETY: both are live basepages (a local and a PD of the page).
    unsafe { crate::bdos::fs::curdir::inherit(&mut pd, creator) };
    pd
}

/// Start a stand-in that `aes_run_rom_program` creates (`which` = accdesk or
/// EmuCON) under the current kernel `run`, `parent`: build its PD and its
/// environment — a copy of `run->p_env` (`env == NULL`, `bdos/proc.c:419-429`)
/// — and make it the kernel's `run` (`PE_GOTHENFREE` → `proc_go`, `:575`).
fn start_rom_pd(which: u8, env_off: usize, parent: u8) {
    if !ready() {
        return;
    }
    let parent_env = env_of(parent);
    let mut env = [0u8; AES_ENV_SIZE];
    let n = parent_env.len().min(AES_ENV_SIZE);
    env[..n].copy_from_slice(&parent_env[..n]);
    // The PD this stand-in had before (a previous session's accdesk, an
    // EmuCON that was not left) gives its directory uses back first.
    // SAFETY: a PD of the static page, kernel context (`Cell` contract).
    unsafe { crate::bdos::fs::curdir::release(pd_ptr(which)) };
    // SAFETY: the parent PD is a field of the static page; short shared read.
    let creator = unsafe { &*pd_ptr(parent) };
    let pd = rom_pd(pd_offset(which), user_va(pd_offset(parent)), user_va(env_off), creator);
    // SAFETY: kernel context, single CPU; no reference into the page is alive
    // (`Cell` contract). Field-wise raw writes.
    unsafe {
        let page = PAGE.0.get();
        if which == RUN_ACCDESK {
            (*page).accdesk_env = env;
            (*page).accdesk = pd;
        } else {
            (*page).emucon_env = env;
            (*page).emucon = pd;
        }
    }
    KERNEL_RUN.store(which, Ordering::Release);
}

/// `true` once [`init`] has built the page.
pub fn ready() -> bool {
    READY.load(Ordering::Acquire)
}

/// The AES has started — EmuTOS `exec_os` (`bios/bios.c:855-866`), which
/// follows `autoexec`: from now on `run` while kernel code is on the stack is
/// the AES PD, not `initial_basepage`. The AES PD is created now in EmuTOS,
/// with an empty `p_cmdlin` (`init_pd_fields`, `bdos/proc.c:378-384`) — which
/// is `ad_stail` from here on (D-4, [`set_aes_cmdlin`]).
pub fn enter_aes() {
    if ready() {
        // SAFETY: kernel context, single CPU (`Cell` contract).
        unsafe { (*PAGE.0.get()).aes.p_cmdlin = [0; PDCLSIZE] };
    }
    KERNEL_RUN.store(RUN_AES, Ordering::Release);
}

/// Back to the state before the AES (`initial_basepage`). Boot does not need
/// it; host tests do.
pub fn leave_aes() {
    KERNEL_RUN.store(RUN_INITIAL, Ordering::Release);
}

/// `aes_run_rom_program(accdesk_start)` (`aes/geminit.c:894`,
/// `aes/gemshlib.c:570-581`) — one AES session's accdesk process starts:
/// its PD is created under the AES PD (`p_parent` = AES PD, environment a
/// copy of the AES PD's, handles and drive of the AES PD) and is `run` from
/// now on, for everything `run_accs_and_desktop` does (`aes/geminit.c:
/// 654-750`): the accessories are loaded under it, the desktop, the `#Z`
/// program and the programs the desktop asks for are its children. A new PD
/// per AES session (`aes/gemstart.S:121-124`); NewTOS rebuilds the one
/// stand-in at the same address.
pub fn enter_accdesk() {
    start_rom_pd(RUN_ACCDESK, ACCDESK_ENV_OFFSET, RUN_AES);
}

/// `accdesk_start` ends (`Pterm0`, `aes/gemstart.S:160-163`): `run =
/// run->p_parent` = the AES PD (`bdos/proc.c:609`) — shutdown or resolution
/// change.
pub fn leave_accdesk() {
    if KERNEL_RUN.load(Ordering::Acquire) == RUN_ACCDESK {
        end_rom_pd(RUN_ACCDESK);
        KERNEL_RUN.store(RUN_AES, Ordering::Release);
    }
}

/// The directory half of `ixterm` for a stand-in that ends (`Pterm0` of
/// accdesk or EmuCON, `bdos/proc.c:126-132`): its `dirtbl` uses are dropped
/// and, the PD being gone in EmuTOS, its `p_curdir[]` bytes cleared so a
/// later recount does not see them.
fn end_rom_pd(which: u8) {
    let p = pd_ptr(which);
    // SAFETY: a PD of the static page, kernel context (`Cell` contract).
    unsafe {
        crate::bdos::fs::curdir::release(p);
        (*p).p_curdir = [0; tos_types::NUMCURDIR];
    }
}

/// `aes_run_rom_program(coma_start)` (`aes/gemshlib.c:614-620`) — the
/// in-kernel EmuCON starts: its PD is created under the current `run` (the
/// accdesk PD in the shell loop's CONSOLE arm and its fallbacks; EmuTOS's
/// early console would have `initial_basepage`, `bios/bios.c:834-843`),
/// environment a copy of that PD's, and is `run` while EmuCON executes — so
/// what EmuCON `Pexec`s gets it as `p_parent` (`cli/cmdexec.c:52`,
/// [`super::pexec::set_parent`] with a kernel caller).
pub fn enter_emucon() {
    let parent = KERNEL_RUN.load(Ordering::Acquire);
    let parent = if parent == RUN_EMUCON { EMUCON_PARENT.load(Ordering::Acquire) } else { parent };
    EMUCON_PARENT.store(parent, Ordering::Release);
    start_rom_pd(RUN_EMUCON, EMUCON_ENV_OFFSET, parent);
}

/// EmuCON ends (`Pterm0`, `cli/cmdasm.S:48-49`): `run` is its parent again.
pub fn leave_emucon() {
    if KERNEL_RUN.load(Ordering::Acquire) == RUN_EMUCON {
        end_rom_pd(RUN_EMUCON);
        KERNEL_RUN.store(EMUCON_PARENT.load(Ordering::Acquire), Ordering::Release);
    }
}

/// D-4: `ad_stail` **is** the AES PD's `p_cmdlin` (`aes/gemstart.S:54-57`,
/// AES PD + 0x80; written by `sh_write`, `aes/gemshlib.c:170,191`, read by
/// `sh_read`, `:105`). NewTOS keeps `ad_stail` in the AES shell
/// (`crate::aes::shell`), which mirrors every write here, so a program that
/// reads the AES PD finds the same bytes. A no-op before the AES has started
/// (no AES PD exists in EmuTOS then; [`enter_aes`] gives it its empty tail).
pub fn set_aes_cmdlin(tail: &[u8]) {
    if !ready() || KERNEL_RUN.load(Ordering::Acquire) == RUN_INITIAL {
        return;
    }
    let mut c = [0u8; PDCLSIZE];
    let n = tail.len().min(PDCLSIZE);
    c[..n].copy_from_slice(&tail[..n]);
    // SAFETY: kernel context, single CPU (`Cell` contract).
    unsafe { (*PAGE.0.get()).aes.p_cmdlin = c };
}

/// User address of the `initial_basepage` stand-in (`null` before [`init`]).
pub fn initial_basepage() -> *mut BasePage {
    if ready() { user_va(0) as *mut BasePage } else { core::ptr::null_mut() }
}

/// User address of the AES-PD stand-in (`null` before [`init`]).
pub fn aes_pd() -> *mut BasePage {
    if ready() { user_va(AES_OFFSET) as *mut BasePage } else { core::ptr::null_mut() }
}

/// User address of the accdesk-PD stand-in (`null` before [`init`]). Its
/// content is valid from the first [`enter_accdesk`] on.
pub fn accdesk_pd() -> *mut BasePage {
    if ready() { user_va(ACCDESK_OFFSET) as *mut BasePage } else { core::ptr::null_mut() }
}

/// User address of the EmuCON-PD stand-in (`null` before [`init`]). Its
/// content is valid from the first [`enter_emucon`] on.
pub fn emucon_pd() -> *mut BasePage {
    if ready() { user_va(EMUCON_OFFSET) as *mut BasePage } else { core::ptr::null_mut() }
}

/// EmuTOS's `run` while kernel code is on the stack: `initial_basepage`
/// until [`enter_aes`], the AES PD until [`enter_accdesk`], then the accdesk
/// PD — or the EmuCON PD while EmuCON runs. A user address (`null` before
/// [`init`]) — what `p_parent` of a program a kernel caller starts holds
/// (`proc_go`, `bdos/proc.c:541`).
pub fn kernel_run() -> *mut BasePage {
    if !ready() {
        return core::ptr::null_mut();
    }
    user_va(pd_offset(KERNEL_RUN.load(Ordering::Acquire))) as *mut BasePage
}

/// The **kernel** pointer to the PD [`kernel_run`] names — what the current
/// drive and directory calls read and write when no process runs
/// (`crate::bdos::fs::curdir`, TASK/PROC04 point 2). Unlike [`kernel_run`]
/// never null: before [`init`] it is `initial_basepage`, as EmuTOS's `run` is
/// from GEMDOS initialisation on (`bdos/bdosmain.c:339`), with drive `C:`.
pub fn kernel_run_kva() -> *mut BasePage {
    pd_ptr(KERNEL_RUN.load(Ordering::Acquire))
}

/// Kernel pointers to the four stand-in PDs, for a recount of the `dirtbl`
/// uses (`crate::bdos::fs::curdir`). A stand-in that is not alive holds no
/// directory bytes ([`end_rom_pd`], [`init`]).
pub fn live_pds() -> [*const BasePage; 4] {
    [
        pd_ptr(RUN_INITIAL),
        pd_ptr(RUN_AES),
        pd_ptr(RUN_ACCDESK),
        pd_ptr(RUN_EMUCON),
    ]
}

/// The standard handles of [`kernel_run`] — what `init_pd_files` inherits
/// when the caller is the kernel (`bdos/proc.c:391-398`, TASK/PROC02). Every
/// stand-in holds `default_handle` (`stdhdl_init`, `bdos/bdosmain.c:347`; the
/// AES, accdesk and EmuCON PDs copy it). Before [`init`] it is still
/// `default_handle`: EmuTOS's `run` is `initial_basepage` with those handles
/// from BDOS initialisation on, before any program exists.
pub fn kernel_run_uft() -> [i8; NUMSTD] {
    if !ready() {
        return DEFAULT_HANDLE;
    }
    // SAFETY: a field of the static page, read by value (`Cell` contract).
    unsafe { (*pd_ptr(KERNEL_RUN.load(Ordering::Acquire))).p_uft }
}

/// The environment of [`kernel_run`] as the kernel reads it — what
/// `alloc_env(flags, NULL)` copies when the caller is the kernel
/// (`bdos/proc.c:419-420`): `double_nul` before the AES, the AES / accdesk /
/// EmuCON environment after it (the same bytes, `_parent-and-environment.md`
/// §3), each with its terminating double NUL. `None` before [`init`].
pub fn kernel_run_env() -> Option<&'static [u8]> {
    if !ready() {
        return None;
    }
    Some(env_of(KERNEL_RUN.load(Ordering::Acquire)))
}

/// `ad_envrn` — the AES PD's environment (`aes/gemstart.S:78-96`), as the
/// whole `ENV_SIZE` block, with its kernel address and its published user
/// address. `shel_envrn` searches this block whatever `run` is
/// (`aes/gemshlib.c:338-347`). `None` before [`init`].
pub fn aes_env() -> Option<(&'static [u8], u64)> {
    if !ready() {
        return None;
    }
    // SAFETY: written only by `init` (boot); shared reads afterwards.
    let p = unsafe { &*PAGE.0.get() };
    Some((&p.aes_env, user_va(AES_ENV_OFFSET)))
}

/// `env` up to and including its first double NUL — EmuTOS `envsize`
/// (`bdos/proc.c:150-159`); the whole block if it has none.
fn env_image(env: &[u8]) -> &[u8] {
    let mut n = 0;
    while n + 1 < env.len() {
        if env[n] == 0 && env[n + 1] == 0 {
            return &env[..n + 2];
        }
        n += 1;
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All stand-in state is global: the tests run in one function, in order.
    #[test]
    fn stand_ins_match_emutos() {
        // SAFETY: test-only, single test touching the page.
        unsafe { init(2) };
        assert!(ready());

        // SAFETY: built; read-only view.
        let p = unsafe { &*PAGE.0.get() };
        let ib = &p.initial;
        let aes = &p.aes;
        let iv = initial_basepage() as u64;
        let av = aes_pd() as u64;
        assert_eq!(av, iv + 0x100);

        // initial_basepage (bdosmain.c:78,339-347; console.c:107-112; bios.c:827)
        assert_eq!(ib.p_parent, 0, "initial_basepage is the root");
        assert_eq!(ib.p_flags, PF_STANDARD);
        assert_eq!(ib.p_env as u32, (iv + 0x200) as u32, "p_env = &double_nul");
        assert_eq!(p.double_nul, [0, 0]);
        assert_eq!(ib.p_uft, [-1, -1, -2, -3, -1, -1]);
        assert_eq!(ib.p_curdrv, 2);
        assert_eq!((ib.p_lowtpa, ib.p_hitpa, ib.p_tbase, ib.p_xdta), (0, 0, 0, 0));

        // The AES PD (bios.c:862-865; proc.c:367-409,541)
        assert_eq!(aes.p_lowtpa as u32, av as u32, "p_lowtpa = p");
        assert_eq!(aes.p_hitpa as u32, (av + 0x100) as u32);
        assert_eq!(aes.p_xdta as u32, (av + 0x80) as u32, "p_xdta = &p_cmdlin");
        assert_eq!(aes.p_parent as u32, iv as u32, "p_parent = &initial_basepage");
        assert_eq!(aes.p_flags, PF_STANDARD);
        assert_eq!(aes.p_env as u32, (iv + 0x210) as u32);
        assert_eq!((aes.p_tbase, aes.p_tlen, aes.p_dlen, aes.p_blen), (0, 0, 0, 0));
        assert_eq!(aes.p_uft, ib.p_uft);
        assert_eq!(aes.p_curdrv, 2);
        assert_eq!(aes.p_cmdlin[0], 0);

        // default_env, bootdev 2 (bios.c:626-636)
        assert_eq!(&p.aes_env, b"PATH=\0C:\\\0\0\0");
        let (env, va) = aes_env().unwrap();
        assert_eq!(env, b"PATH=\0C:\\\0\0\0");
        assert_eq!(va, iv + 0x210);

        // run while kernel code is on the stack: initial, then the AES.
        leave_aes();
        assert_eq!(kernel_run() as u64, iv);
        assert_eq!(kernel_run_env().unwrap(), &[0u8, 0][..], "AUTO: double_nul");
        // SAFETY: a null `run` is the documented kernel case.
        let auto_env = unsafe { super::super::loader::inherited_env(core::ptr::null()) };
        assert_eq!(auto_env, &[0u8, 0][..], "the loader's Inherit from a kernel caller, AUTO");
        // D-4 before the AES: no AES PD exists, the tail is not mirrored.
        set_aes_cmdlin(&[3, b'a', b'b', b'c']);
        // SAFETY: as above.
        assert_eq!(unsafe { (*PAGE.0.get()).aes.p_cmdlin[0] }, 0);
        enter_aes();
        assert_eq!(kernel_run() as u64, av);
        // envsize = 11 (bdos/proc.c:150-159): up to and with the double NUL.
        assert_eq!(kernel_run_env().unwrap(), b"PATH=\0C:\\\0\0");
        // SAFETY: as above.
        let desk_env = unsafe { super::super::loader::inherited_env(core::ptr::null()) };
        assert_eq!(desk_env, b"PATH=\0C:\\\0\0", "the loader's Inherit from a kernel caller, AES");
        // D-4: ad_stail is the AES PD's p_cmdlin.
        set_aes_cmdlin(&[2, b'Z', b'2']);
        // SAFETY: as above.
        assert_eq!(unsafe { &(&(*PAGE.0.get()).aes.p_cmdlin)[..4] }, &[2, b'Z', b'2', 0]);
        set_aes_cmdlin(&[0]);
        // SAFETY: as above.
        assert_eq!(unsafe { (*PAGE.0.get()).aes.p_cmdlin[1] }, 0);

        // The accdesk PD (gemshlib.c:570-581; proc.c:367-432,541).
        let dv = accdesk_pd() as u64;
        assert_eq!(dv, iv + 0x300);
        enter_accdesk();
        assert_eq!(kernel_run() as u64, dv, "run = accdesk between programs");
        // SAFETY: as above.
        let d = unsafe { &*(dv as *const BasePage) };
        assert_eq!(d.p_parent as u32, av as u32, "accdesk.p_parent = AES PD");
        assert_eq!(d.p_lowtpa as u32, dv as u32);
        assert_eq!(d.p_hitpa as u32, (dv + 0x100) as u32, "D-2");
        assert_eq!(d.p_xdta as u32, (dv + 0x80) as u32, "D-3");
        assert_eq!((d.p_tbase, d.p_tlen, d.p_dlen, d.p_blen), (0, 0, 0, 0), "D-1");
        assert_eq!(d.p_flags, PF_STANDARD);
        assert_eq!(d.p_env as u32, (iv + 0x500) as u32, "its own environment block");
        assert_eq!(d.p_uft, DEFAULT_HANDLE);
        assert_eq!(d.p_curdrv, 2);
        assert_eq!(kernel_run_env().unwrap(), b"PATH=\0C:\\\0\0", "a copy of ad_envrn");
        assert_eq!(kernel_run_uft(), DEFAULT_HANDLE);

        // The EmuCON PD under accdesk (gemshlib.c:614-620).
        let ev = emucon_pd() as u64;
        assert_eq!(ev, iv + 0x400);
        enter_emucon();
        assert_eq!(kernel_run() as u64, ev);
        // SAFETY: as above.
        let e = unsafe { &*(ev as *const BasePage) };
        assert_eq!(e.p_parent as u32, dv as u32, "EmuCON.p_parent = accdesk");
        assert_eq!(e.p_lowtpa as u32, ev as u32);
        assert_eq!(e.p_env as u32, (iv + 0x510) as u32);
        assert_eq!(kernel_run_env().unwrap(), b"PATH=\0C:\\\0\0");
        leave_emucon();
        assert_eq!(kernel_run() as u64, dv, "Pterm0 of EmuCON: run = accdesk");
        leave_emucon();
        assert_eq!(kernel_run() as u64, dv, "a second leave changes nothing");
        leave_accdesk();
        assert_eq!(kernel_run() as u64, av, "Pterm0 of accdesk: run = AES PD");

        // EmuCON before the AES (EmuTOS's early console): parent initial,
        // environment double_nul.
        leave_aes();
        enter_emucon();
        // SAFETY: as above.
        let e = unsafe { &*(ev as *const BasePage) };
        assert_eq!(e.p_parent as u32, iv as u32);
        assert_eq!(kernel_run_env().unwrap(), &[0u8, 0][..]);
        leave_emucon();
        assert_eq!(kernel_run() as u64, iv);

        // A running process whose `p_env` is 0 hands down the empty env.
        let bp = BasePage::zeroed();
        // SAFETY: a live basepage with `p_env == 0`.
        assert_eq!(unsafe { super::super::loader::inherited_env(&bp) }, &[0u8, 0][..]);

        // Another boot drive changes the letter only.
        // SAFETY: as above.
        unsafe { init(3) };
        assert_eq!(&aes_env().unwrap().0[..10], b"PATH=\0D:\\\0");
        // SAFETY: as above.
        unsafe { init(2) };
        leave_aes();
    }

    #[test]
    fn env_image_is_envsize() {
        assert_eq!(env_image(b"A=1\0\0xx"), b"A=1\0\0");
        assert_eq!(env_image(b"\0\0"), b"\0\0");
        assert_eq!(env_image(b"PATH=\0C:\\\0\0\0"), b"PATH=\0C:\\\0\0");
    }
}
