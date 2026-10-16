//! ARCH03 stage 3b — `Fsetdta` from a kernel (EL1) caller while a process is
//! running must not cut the kernel address to 32 bits into that process's
//! `p_xdta` (3a observation). EmuTOS `xsetdta` stores into `run->p_xdta`
//! (`bdos/fsmain.c:403-405`) — every EmuTOS caller is a process; NewTOS's
//! kernel callers (the built-in shell EmuCON, the selftests) keep their own
//! DTA cell (`bdos::fs::find`, `KERNEL_DTA`).

use tos_api::bdos::fs::find;
use tos_api::bdos::process;
use tos_api::trap::TrapArgs;
use tos_api::uaccess::Space;
use tos_types::{BasePage, Long};

fn args(space: Space, a0: u64) -> TrapArgs {
    TrapArgs { regs: [a0, 0, 0, 0, 0, 0, 0, 0], func: 0x1A, svc_imm: 1, space }
}

#[test]
fn a_kernel_dta_never_lands_in_the_running_processs_basepage() {
    // A running process with its own DTA.
    let slot = process::allocate_slot().expect("a slot");
    let mut b = Box::new([0u8; 256]);
    let user_dta: u32 = 0x0123_4080;
    b[0x20..0x24].copy_from_slice(&user_dta.to_le_bytes()); // p_xdta
    let bp = Box::into_raw(b) as *mut BasePage;
    process::PROCESS_TABLE[slot].basepage_kva.store(bp, std::sync::atomic::Ordering::Release);
    process::set_current(process::pd_of(slot));

    // A kernel caller sets a high-half kernel DTA.
    let kdta: u64 = 0xFFFF_0000_0012_3440;
    assert_eq!(find::fsetdta(&args(Space::Kernel, kdta)), 0);
    // SAFETY: `bp` is the live test basepage.
    assert_eq!(unsafe { (*bp).p_xdta } as u32, user_dta, "the program's DTA is untouched");
    assert_eq!(find::fgetdta(&args(Space::User, 0)) as u32, user_dta, "the program reads its own");
    assert_eq!(find::fgetdta(&args(Space::Kernel, 0)), kdta as Long, "the kernel caller its own");

    // The program's own Fsetdta still goes to its basepage.
    assert_eq!(find::fsetdta(&args(Space::User, 0x0123_5000)), 0);
    // SAFETY: as above.
    assert_eq!(unsafe { (*bp).p_xdta } as u32, 0x0123_5000);

    process::set_current(std::ptr::null_mut());
    process::free_slot(slot);
    // SAFETY: allocated by `Box::new` above.
    drop(unsafe { Box::from_raw(bp as *mut [u8; 256]) });
}
