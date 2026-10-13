//! TASK/FS10 — `Fseek` on character-device and standard handles, and the
//! signed-WORD reading of the handle by `Fseek` / `Fread` / `Fwrite`.
//!
//! Spec: `docs/api/gemdos/file/42-fseek.md` §"Character-device and standard
//! handles (CORRECTION 2026-10-13)", test cases T1–T13. EmuTOS `osif`
//! (`bdos/bdosmain.c:571-642`) resolves the handle word of `Fread`/`Fwrite`/
//! `Fseek` before the function runs: a handle that ends on a character device
//! (`-1..-3`) answers `0` for `Fseek` whatever the offset and mode; `<= -4`
//! and an unmapped handle answer `EIHNDL`; a file goes to `xlseek`.
//!
//! Basepages are host buffers; on the host a PD's user address is its
//! kernel address (`process::kalias` is the identity).

mod fatvol;

use fatvol::*;
use tos_api::bdos::process;
use tos_types::BasePage;

const FSEEK: u16 = 0x42;
const FDUP: u16 = 0x45;
const FFORCE: u16 = 0x46;
const FDATIME: u16 = 0x57;
const EINVFN: i32 = -32;
const EIHNDL: i32 = -37;
const ERANGE: i32 = -64;
const EINTRN: i32 = -65;

/// EmuTOS `default_handle` (`bdos/console.c:60-68`).
const DEFAULT: [i8; 6] = [-1, -1, -2, -3, -1, -1];

/// A process basepage in host memory with the default standard handles, made
/// the running process while it lives (`run`).
struct Pd(*mut BasePage);
impl Pd {
    fn new() -> Pd {
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        // SAFETY: fresh box.
        unsafe { (*bp).p_uft = DEFAULT };
        process::set_current(bp);
        Pd(bp)
    }
    fn uft(&self) -> [i8; 6] {
        // SAFETY: the box is alive.
        unsafe { (*self.0).p_uft }
    }
}
impl Drop for Pd {
    fn drop(&mut self) {
        if process::current() == self.0 {
            process::set_current(core::ptr::null_mut());
        }
        // SAFETY: nothing names the box any more.
        drop(unsafe { Box::from_raw(self.0 as *mut [u8; 256]) });
    }
}

/// `Fseek(off, h, flg)` with the handle and mode as a WORD sign-extended into
/// the register, as a C caller's `short` arrives.
fn fseek(off: i32, h: i32, flg: i32) -> i32 {
    call(FSEEK, [off as i64 as u64, h as i64 as u64, flg as i64 as u64, 0, 0, 0, 0, 0])
}

/// `Fseek` with the handle WORD zero-extended (`unsigned short` caller): only
/// the low 16 bits are the WORD, so the result must not differ.
fn fseek_zx(off: i32, h: i32, flg: i32) -> i32 {
    call(FSEEK, [off as i64 as u64, h as u16 as u64, flg as u16 as u64, 0, 0, 0, 0, 0])
}

fn fwrite(h: i32, data: &[u8]) -> i32 {
    call(FWRITE, [h as i64 as u64, data.len() as u64, data.as_ptr() as u64, 0, 0, 0, 0, 0])
}

fn fread(h: i32, buf: &mut [u8]) -> i32 {
    call(FREAD, [h as i64 as u64, buf.len() as u64, buf.as_mut_ptr() as u64, 0, 0, 0, 0, 0])
}

fn fclose(h: i32) -> i32 {
    gemdos1(FCLOSE, h as u16 as u64)
}

fn fdup(h: i32) -> i32 {
    gemdos1(FDUP, h as u64)
}

fn fforce(std: i32, h: i32) -> i32 {
    gemdos2(FFORCE, std as u16 as u64, h as u16 as u64)
}

fn fcreate(name: &str) -> i32 {
    let p = cpath(name);
    let h = gemdos2(FCREATE, p.as_ptr() as u64, 0);
    assert!(h >= 6, "Fcreate {name} -> {h}");
    h
}

/// T1, T2: every default standard handle is a character device -> `0`.
#[test]
fn t1_t2_default_standard_handles_seek_to_zero() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    assert_eq!(fseek(0, 1, 1), 0, "T1 Fseek(0, 1, SEEK_CUR)");
    for h in 0..6 {
        assert_eq!(fseek(0, h, 1), 0, "T2 Fseek(0, {h}, 1)");
    }
}

/// T3, T4: the device answer ignores the offset and the mode — an invalid
/// mode is not `EINVFN` here (`bdosmain.c:642`, `xlseek` never runs).
#[test]
fn t3_t4_offset_and_mode_are_ignored_on_a_device() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    assert_eq!(fseek(12345, 1, 0), 0, "T3 SEEK_SET 12345");
    assert_eq!(fseek(-7, 1, 2), 0, "T3 SEEK_END -7");
    assert_eq!(fseek(0, 1, 3), 0, "T4 mode 3");
    assert_eq!(fseek(0, 1, 0xFFFF), 0, "T4 mode 0xFFFF");
}

/// T5, T6: negative handles are the device itself; `<= -4` is no device.
/// Both register encodings of the WORD give the same answer.
#[test]
fn t5_t6_negative_handles() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    for h in [-1, -2, -3] {
        assert_eq!(fseek(0, h, 0), 0, "T5 Fseek(0, {h}, 0)");
        assert_eq!(fseek_zx(0, h, 0), 0, "T5 Fseek(0, {h} as u16, 0)");
    }
    for h in [-4, -32768] {
        assert_eq!(fseek(0, h, 0), EIHNDL, "T6 Fseek(0, {h}, 0)");
        assert_eq!(fseek_zx(0, h, 0), EIHNDL, "T6 Fseek(0, {h} as u16, 0)");
    }
}

/// T7–T9: a standard handle forced to a file seeks the file; `Fclose` of the
/// standard handle reverts it to the console.
#[test]
fn t7_t8_t9_forced_standard_handle_seeks_the_file() {
    let (_g, _host) = fs();
    let p = Pd::new();
    let f = fcreate("C:\\FS10A.TXT");
    assert_eq!(fwrite(f, &[0x55; 100]), 100);
    assert_eq!(fseek(10, f, 0), 10);
    assert_eq!(fforce(1, f), E_OK);

    assert_eq!(fseek(0, 1, 1), 10, "T7 SEEK_CUR through handle 1");
    assert_eq!(fseek(0, 1, 2), 100, "T8 SEEK_END");
    assert_eq!(fseek(200, 1, 0), ERANGE, "T8 past EOF");
    assert_eq!(fseek(0, 1, 3), EINVFN, "T8 invalid mode on a file");

    assert_eq!(fclose(1), E_OK, "reverts handle 1, first ixclose");
    assert_eq!(p.uft(), DEFAULT);
    assert_eq!(fseek(0, 1, 1), 0, "T9 handle 1 is the console again");
    assert_eq!(fclose(f), EINTRN, "second ixclose; entry freed");
}

/// `Fforce` of a standard handle onto a BIOS device (row 3 of the result
/// table), and onto `-4`, which `osif` rejects (`bdosmain.c:601-602`).
#[test]
fn forced_to_a_device_seeks_to_zero_forced_to_minus_4_is_invalid() {
    let (_g, _host) = fs();
    let p = Pd::new();
    assert_eq!(fforce(1, -3), E_OK);
    assert_eq!(fseek(0, 1, 1), 0, "forced to PRN");
    assert_eq!(fforce(1, -4), E_OK, "Fforce stores a negative handle as-is");
    assert_eq!(fseek(0, 1, 1), EIHNDL, "-4 is not a character device");
    assert_eq!(fwrite(1, b"x"), EIHNDL, "same resolution for Fwrite");
    assert_eq!(fclose(1), E_OK);
    assert_eq!(p.uft(), DEFAULT);
}

/// T10, T11: an `Fdup` of a console standard handle is a device (`0`); once
/// closed, its slot is free (`EIHNDL`).
#[test]
fn t10_t11_fdup_of_a_device_handle() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    let d = fdup(1);
    assert!(d >= 6, "Fdup(1) -> {d}");
    assert_eq!(fseek(0, d, 1), 0, "T10");
    assert_eq!(fclose(d), E_OK);
    assert_eq!(fseek(0, d, 1), EIHNDL, "T11");
}

/// T12 and the `>= NUMHANDLES` contract row.
#[test]
fn t12_free_and_out_of_range_handles() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    // This binary closes every handle it opens: the last entry is free.
    assert_eq!(fseek(0, 6 + 15, 0), EIHNDL, "T12 free sft slot");
    assert_eq!(fseek(0, 6 + 16, 0), EIHNDL, "NUMHANDLES");
    assert_eq!(fseek(0, 0x7FFF, 0), EIHNDL, "largest positive WORD");
}

/// T13: `Fdatime` is not in the `osif` device block; a console handle has no
/// OFD (`bdos/fsdir.c:686-690`).
#[test]
fn t13_fdatime_on_a_console_handle_is_invalid() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    let mut buf = [0u8; 4];
    assert_eq!(call(FDATIME, [buf.as_mut_ptr() as u64, 1, 0, 0, 0, 0, 0, 0]), EIHNDL);
    assert_eq!(
        call(FDATIME, [buf.as_mut_ptr() as u64, -1i64 as u64, 0, 0, 0, 0, 0, 0]),
        EIHNDL
    );
}

/// `Fread` / `Fwrite` read the handle as a signed WORD too: `-1..-3` resolve
/// to a character device (all three to the console for now), `<= -4` is
/// `EIHNDL` (`bdosmain.c:589,601-602`).
#[test]
fn fread_fwrite_take_negative_handles_as_devices() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    for h in [-1, -2, -3] {
        assert_eq!(fwrite(h, b"fs10"), 4, "Fwrite({h})");
        let zx = call(FWRITE, [h as u16 as u64, 2, b"ok".as_ptr() as u64, 0, 0, 0, 0, 0]);
        assert_eq!(zx, 2, "Fwrite({h} as u16)");
    }
    assert_eq!(fwrite(-4, b"x"), EIHNDL);
    assert_eq!(fwrite(-32768, b"x"), EIHNDL);
    let mut b = [0u8; 1];
    assert_eq!(fread(-4, &mut b), EIHNDL);
    // A zero-length read of a device moves nothing and does not block.
    assert_eq!(fread(-1, &mut b[..0]), 0);
}
