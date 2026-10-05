//! ARCH03 stage 3a: the GEM trap reads and writes the caller's AES / VDI
//! parameter blocks in the caller's space (`aes::dispatch_in`,
//! `vdi::dispatch_in` with `uaccess::Space`). On the host both spaces are
//! plain host-memory accesses, so these tests pin that the conversion changed
//! no result: the same call through `Space::User` and through the kernel
//! entry (`dispatch`, `Space::Kernel`) leaves the same words behind. The
//! fault side (a bad block or array pointer ends the calling program) needs
//! the MMU and is driven in QEMU (probe UACC3A, cases A I G F V C M E X).

use std::sync::Mutex;

use tos_api::bdos::process::Abi;
use tos_api::uaccess::Space;

/// The AES and VDI keep global state; one call at a time.
static LOCK: Mutex<()> = Mutex::new(());

/// One AES call with a fresh block → (`int_out`, `global`).
fn aes(space: Option<Space>, control: [i16; 5], intin: [i16; 4], addrin: [u64; 2]) -> ([i16; 8], [i16; 16]) {
    let mut control = control;
    let mut global = [0x55i16; 16];
    let mut intin = intin;
    let mut intout = [0x77i16; 8];
    let mut addrin = addrin;
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
    unsafe {
        match space {
            Some(sp) => tos_api::aes::dispatch_in(raw.as_ptr() as u64, Abi::Lp64, sp),
            None => tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64),
        }
    };
    (intout, global)
}

#[test]
fn appl_init_fills_the_same_aesglobal_in_both_spaces() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let k = aes(None, [10, 0, 1, 0, 0], [0; 4], [0; 2]);
    let u = aes(Some(Space::User), [10, 0, 1, 0, 0], [0; 4], [0; 2]);
    assert_eq!(k, u);
    // ap_version was written (0x0340) and the words past index 12 were not.
    assert_eq!(u.1[0], 0x0340);
    assert_eq!(u.1[13], 0x55);
    // Exactly OUT_LEN = 1 word reached the caller's int_out.
    assert_eq!(u.0[1], 0x77);
}

#[test]
fn appl_find_reads_the_name_in_the_callers_space() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let name = *b"SCRENMGR";
    let ctl = [13, 0, 1, 1, 0];
    let k = aes(None, ctl, [0; 4], [name.as_ptr() as u64, 0]);
    let u = aes(Some(Space::User), ctl, [0; 4], [name.as_ptr() as u64, 0]);
    assert_eq!(k.0, u.0);
    assert_eq!(u.0[0], 1, "the Screen Manager's pid");
    // A null name: -1, nothing read.
    let n = aes(Some(Space::User), ctl, [0; 4], [0, 0]);
    assert_eq!(n.0[0], -1);
}

#[test]
fn out_len_zero_writes_nothing_in_user_space() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // appl_yield (17) with OUT_LEN = 0: not even RET_CODE is copied back.
    let u = aes(Some(Space::User), [17, 0, 0, 0, 0], [0; 4], [0; 2]);
    assert_eq!(u.0, [0x77; 8]);
}

#[test]
fn vdi_block_and_contrl_go_through_the_callers_space() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for space in [Space::User, Space::Kernel] {
        let mut contrl = [102i16, 0, 9, 1, 9, 0, 1, 0, 0, 0, 0, 0];
        let mut intin = [0i16; 4];
        let mut ptsin = [0i16; 4];
        let mut intout = [0i16; 64];
        let mut ptsout = [0i16; 16];
        let raw = [
            contrl.as_mut_ptr() as u64,
            intin.as_mut_ptr() as u64,
            ptsin.as_mut_ptr() as u64,
            intout.as_mut_ptr() as u64,
            ptsout.as_mut_ptr() as u64,
        ];
        // SAFETY: a well-formed LP64 VDI parameter block, alive for the call.
        let r = unsafe { tos_api::vdi::dispatch_in(raw.as_ptr() as u64, Abi::Lp64, space) };
        // Hostless: no raster backend, so the VDI is inert after it has set
        // the default output counts — CONTRL[2] and CONTRL[4] — to 0.
        assert_eq!(r, 0, "{space:?}");
        assert_eq!((contrl[2], contrl[4]), (0, 0), "{space:?}");
        assert_eq!(contrl[0], 102, "{space:?}");
    }
}
