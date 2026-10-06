//! The two process descriptors EmuTOS has and NewTOS has no process for —
//! `initial_basepage` and the AES PD — as read-only **stand-ins** (TASK/PPARENT,
//! user decision 2026-10-06 "Way A").
//!
//! Spec: `docs/api/gemdos/process/_parent-and-environment.md`.
//!
//! EmuTOS writes `p_parent` only in `proc_go`, `p->p_parent = run`
//! (`bdos/proc.c:541`), and gives every process an environment, a copy of
//! `run->p_env` unless the caller names one (`alloc_env`, `bdos/proc.c:413-432`).
//! While the BIOS runs the AUTO folder `run` is the static `initial_basepage`
//! (`bdos/bdosmain.c:78,337-342`, `bios/bios.c:685-695`); from the start of
//! the AES on it is the AES's own PD (`bios/bios.c:855-866`), which is what the
//! desktop, the `#Z` program and every program the desktop asks for get as
//! their parent (`aes/gemshlib.c:569-580,645`).
//!
//! NewTOS runs the AES and the desktop in the kernel; `RUN` is `null` while
//! kernel code is on the stack. This module provides the two PDs that `run`
//! would be there: real, readable basepages with EmuTOS's field contents, in
//! one kernel page that is published read-only to every process at VA = PA
//! (`usermem::expose`, `PROT_PR`, ADR-023), so a program can follow its
//! `p_parent` chain in every protection mode. They are **never** the running
//! process: `RUN`, `os_run`, owner tags, `parent_slot` and the `Pterm` return
//! path do not see them (Way B — the AES as a real process — is
//! `TASK/PROC03.md`).
//!
//! What uses them: [`super::pexec::set_parent`] and
//! [`super::pexec::set_kernel_parent`] (`p_parent` of a program a kernel
//! caller starts), the loader's `EnvSpec::Inherit` with no running process
//! ([`kernel_run_env`]), and `shel_envrn` (`ad_envrn` is the AES PD's `p_env`,
//! `aes/gemstart.S:78-96` — [`aes_env`]).

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tos_types::{BasePage, Long, NUMSTD, PF_STANDARD};

use super::kva_to_user_va;

/// `ENV_SIZE` (`bios/bios.c:77`): "sufficient for standard PATH=^X:\^^
/// (^=nul byte)" — the size of `default_env`, and (11 counted bytes rounded
/// up to even, `bdos/proc.c:421`) of the AES PD's copy of it.
pub const AES_ENV_SIZE: usize = 12;

/// `default_handle` (`bdos/console.c:60-68`): `H_Console`, `H_Console`,
/// `H_Aux`, `H_Print`, `H_Console`, `H_Console` (`bdos/console.h:23-25`) —
/// what `stdhdl_init` puts into `initial_basepage.p_uft`
/// (`bdos/console.c:107-112`) and `init_pd_files` copies into the AES PD
/// (`bdos/proc.c:390-398`, a handle `<= 0` is copied as is).
const DEFAULT_HANDLE: [i8; NUMSTD] = [-1, -1, -2, -3, -1, -1];

/// Offset of the AES PD in the page.
const AES_OFFSET: usize = 0x100;
/// Offset of `double_nul`.
const DOUBLE_NUL_OFFSET: usize = 0x200;
/// Offset of the AES environment.
const AES_ENV_OFFSET: usize = 0x210;

/// The page: both PDs and the two environments their `p_env` name, and
/// nothing else — the whole page becomes readable to every process.
#[repr(C, align(4096))]
struct Page {
    /// `initial_basepage` (`bdos/bdosmain.c:78`), +0x000.
    initial: BasePage,
    /// The AES PD (`bios/bios.c:862`), +0x100.
    aes: BasePage,
    /// `double_nul` (`bdos/bdosmain.c:81`), +0x200.
    double_nul: [u8; 2],
    _pad: [u8; AES_ENV_OFFSET - DOUBLE_NUL_OFFSET - 2],
    /// The AES PD's environment, a copy of `default_env`
    /// (`bios/bios.c:626-636`), +0x210.
    aes_env: [u8; AES_ENV_SIZE],
}

const _: () = assert!(core::mem::offset_of!(Page, aes) == AES_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, double_nul) == DOUBLE_NUL_OFFSET);
const _: () = assert!(core::mem::offset_of!(Page, aes_env) == AES_ENV_OFFSET);
const _: () = assert!(core::mem::size_of::<Page>() <= 4096);

struct Cell(UnsafeCell<Page>);
// SAFETY: written only by [`init`] on the boot CPU before any program runs;
// read afterwards (the BDOS giant lock / single-CPU trap context).
unsafe impl Sync for Cell {}

static PAGE: Cell = Cell(UnsafeCell::new(Page {
    initial: BasePage::zeroed(),
    aes: BasePage::zeroed(),
    double_nul: [0, 0],
    _pad: [0; AES_ENV_OFFSET - DOUBLE_NUL_OFFSET - 2],
    aes_env: [0; AES_ENV_SIZE],
}));

/// The page is built ([`init`]).
static READY: AtomicBool = AtomicBool::new(false);

/// Which stand-in is `run` while kernel code is on the stack.
const RUN_INITIAL: u8 = 0;
const RUN_AES: u8 = 1;
static KERNEL_RUN: AtomicU8 = AtomicU8::new(RUN_INITIAL);

/// The user address (= physical address, VA = PA) of the page's byte `off`.
fn user_va(off: usize) -> u64 {
    kva_to_user_va(PAGE.0.get() as u64 + off as u64)
}

/// Build both PDs and the AES environment for boot drive `bootdev`
/// (0 = `A:`), and publish the page read-only to every process. Returns the
/// page's user address, or `None` when it could not be published (no program
/// could then read the chain; the kernel reports it). Idempotent in content:
/// a second call rebuilds the same page.
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
    let aes_va = user_va(AES_OFFSET);
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
    // `osinit_after_xmaddalt`, `stdhdl_init` and `Dsetdrv(bootdev)` write.
    let mut ib = BasePage::zeroed();
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
    // 863-864`); `proc_go`: `p_parent = run` (`bdos/proc.c:541`).
    //
    // No counterpart in NewTOS (deviations by absence, spec §4): the AES has
    // no TPA — its "TPA" is its basepage, so `p_hitpa = p + 0x100`; and no
    // code in the user address space — `p_tbase = 0`.
    let mut aes = BasePage::zeroed();
    aes.p_lowtpa = aes_va as Long;
    aes.p_hitpa = (aes_va + core::mem::size_of::<BasePage>() as u64) as Long;
    aes.p_xdta = (aes_va + core::mem::offset_of!(BasePage, p_cmdlin) as u64) as Long;
    aes.p_parent = initial_va as Long;
    aes.p_flags = PF_STANDARD;
    aes.p_env = aes_env_va as Long;
    aes.p_uft = p.initial.p_uft;
    aes.p_curdrv = p.initial.p_curdrv;
    p.aes = aes;

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

/// `true` once [`init`] has built the page.
pub fn ready() -> bool {
    READY.load(Ordering::Acquire)
}

/// The AES has started — EmuTOS `exec_os` (`bios/bios.c:855-866`), which
/// follows `autoexec`: from now on `run` while kernel code is on the stack is
/// the AES PD, not `initial_basepage`.
pub fn enter_aes() {
    KERNEL_RUN.store(RUN_AES, Ordering::Release);
}

/// Back to the state before the AES (`initial_basepage`). Boot does not need
/// it; host tests do.
pub fn leave_aes() {
    KERNEL_RUN.store(RUN_INITIAL, Ordering::Release);
}

/// User address of the `initial_basepage` stand-in (`null` before [`init`]).
pub fn initial_basepage() -> *mut BasePage {
    if ready() { user_va(0) as *mut BasePage } else { core::ptr::null_mut() }
}

/// User address of the AES-PD stand-in (`null` before [`init`]).
pub fn aes_pd() -> *mut BasePage {
    if ready() { user_va(AES_OFFSET) as *mut BasePage } else { core::ptr::null_mut() }
}

/// EmuTOS's `run` while kernel code is on the stack: the `initial_basepage`
/// stand-in until [`enter_aes`], the AES-PD stand-in after it. A user address
/// (`null` before [`init`]) — what `p_parent` of a program a kernel caller
/// starts holds (`proc_go`, `bdos/proc.c:541`).
pub fn kernel_run() -> *mut BasePage {
    if KERNEL_RUN.load(Ordering::Acquire) == RUN_AES { aes_pd() } else { initial_basepage() }
}

/// The environment of [`kernel_run`] as the kernel reads it — what
/// `alloc_env(flags, NULL)` copies when the caller is the kernel
/// (`bdos/proc.c:419-420`): `double_nul` before the AES, the AES environment
/// after it, each with its terminating double NUL. `None` before [`init`].
pub fn kernel_run_env() -> Option<&'static [u8]> {
    if !ready() {
        return None;
    }
    // SAFETY: written only by `init` (boot); shared reads afterwards.
    let p = unsafe { &*PAGE.0.get() };
    if KERNEL_RUN.load(Ordering::Acquire) == RUN_AES {
        Some(env_image(&p.aes_env))
    } else {
        Some(&p.double_nul)
    }
}

/// `ad_envrn` — the AES PD's environment (`aes/gemstart.S:78-96`), as the
/// whole `ENV_SIZE` block, with its kernel address and its published user
/// address. `None` before [`init`].
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
        enter_aes();
        assert_eq!(kernel_run() as u64, av);
        // envsize = 11 (bdos/proc.c:150-159): up to and with the double NUL.
        assert_eq!(kernel_run_env().unwrap(), b"PATH=\0C:\\\0\0");
        // SAFETY: as above.
        let desk_env = unsafe { super::super::loader::inherited_env(core::ptr::null()) };
        assert_eq!(desk_env, b"PATH=\0C:\\\0\0", "the loader's Inherit from a kernel caller, AES");
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
