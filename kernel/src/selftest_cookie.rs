//! Cookie-jar / `Ssystem` self-test: exercise the cookie subfunctions
//! through the real GEMDOS trap (`svc #1`, function id `0x154` in `x8`).

use crate::kprintln;

const NTOS: u32 = u32::from_be_bytes(*b"NTOS");
const MCH: u32 = u32::from_be_bytes(*b"_MCH");
const TEST: u32 = u32::from_be_bytes(*b"TEST");
const ZZZZ: u32 = u32::from_be_bytes(*b"ZZZZ");

/// Issue `Ssystem(mode, arg1, arg2)`: GEMDOS trap, id `0x154` in `x8`.
fn ssystem(mode: u16, arg1: u32, arg2: u64) -> i64 {
    let ret: i64;
    // SAFETY: GEMDOS trap ABI; result comes back in x0.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") 0x154u64,
            inout("x0") mode as u64 => ret,
            in("x1") arg1 as u64,
            in("x2") arg2,
            options(nostack),
        );
    }
    ret
}

const S_GETCOOKIE: u16 = 8;
const S_SETCOOKIE: u16 = 9;
const S_DELCOOKIE: u16 = 26;

/// Run the cookie-jar self-test.
pub fn run() {
    kprintln!("[cookie] === Cookie jar / Ssystem test ===");
    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[cookie]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // Seeded set: NTOS, _MCH, _IDT, _AKP (4 cookies).
    check(ssystem(S_GETCOOKIE, NTOS, 0) == 0x0000_0100, "Getcookie(NTOS) = version 0x100");
    check(ssystem(S_GETCOOKIE, 0, 0) == 4, "Getcookie(0) = count 4");
    check(ssystem(S_GETCOOKIE, MCH, 0) == 0x0005_0000, "Getcookie(_MCH) = 0x00050000");
    check(ssystem(S_GETCOOKIE, ZZZZ, 0) == -1, "Getcookie(unknown) = ERROR (-1)");

    // Slot enumeration: slot 1 is the first seeded cookie (NTOS).
    check(ssystem(S_GETCOOKIE, 1, 0) == NTOS as i64, "Getcookie(slot 1) = NTOS tag");

    // Setcookie adds; Getcookie reads it back; count grows.
    check(ssystem(S_SETCOOKIE, TEST, 0xCAFE) == 0, "Setcookie(TEST) = E_OK");
    check(ssystem(S_GETCOOKIE, TEST, 0) == 0xCAFE, "Getcookie(TEST) = 0xCAFE");
    check(ssystem(S_GETCOOKIE, 0, 0) == 5, "Getcookie(0) = count 5 after add");

    // Delcookie removes it.
    check(ssystem(S_DELCOOKIE, TEST, 0) == 0, "Delcookie(TEST) = E_OK");
    check(ssystem(S_GETCOOKIE, TEST, 0) == -1, "Getcookie(TEST) after del = ERROR");

    // Invalid tag (a zero byte) and unimplemented mode → EINVFN (-32).
    check(ssystem(S_SETCOOKIE, 0x0041_4243, 1) == -32, "Setcookie(invalid tag) = EINVFN");
    check(ssystem(99, 0, 0) == -32, "Ssystem(unimplemented mode) = EINVFN");

    if fails == 0 {
        kprintln!("[cookie] === Cookie jar PASS ===");
    } else {
        kprintln!("[cookie] === Cookie jar FAILED ({}) ===", fails);
    }
}
