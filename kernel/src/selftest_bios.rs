//! BIOS character-device self-test: `Bconstat`/`Bconin`/`Bconout`/
//! `Bcostat` through the real `svc #13` trap path.

use crate::{console, kprintln};

/// Issue a BIOS call: `svc #13`, function id in `x8`, args in `x0`/`x1`.
fn bios(func: u16, a0: u64, a1: u64) -> i64 {
    let ret: i64;
    // SAFETY: BIOS trap ABI; results come back in x0.
    unsafe {
        core::arch::asm!(
            "svc #13",
            in("x8") func as u64,
            inout("x0") a0 => ret,
            in("x1") a1,
            options(nostack),
        );
    }
    ret
}

/// Run the BIOS character-device self-test.
pub fn run() {
    kprintln!("[bios] === BIOS character devices test (Bcon*) ===");
    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[bios]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // Bcostat (0x08): CON (handle 2) ready; PRT (0) absent.
    check(bios(0x08, 2, 0) == -1, "Bcostat(CON) ready = -1");
    check(bios(0x08, 0, 0) == 0, "Bcostat(PRT) absent = 0");

    // Bconstat (0x01): no input queued yet.
    check(bios(0x01, 2, 0) == 0, "Bconstat(CON) no input = 0");

    // Bconout (0x03): CON writes and returns 1; PRT discards, returns 0.
    check(bios(0x03, 2, b'X' as u64) == 1, "Bconout(CON,'X') = 1");
    check(bios(0x03, 0, b'X' as u64) == 0, "Bconout(PRT,'X') = 0");

    // Bconin (0x02): queue a byte, expect it back; Bconstat now ready.
    console::push_rx(b'Z');
    check(bios(0x01, 2, 0) == -1, "Bconstat(CON) with input = -1");
    check(bios(0x02, 2, 0) == 0x5A, "Bconin(CON) returns 'Z' = 0x5A");

    if fails == 0 {
        kprintln!("[bios] === BIOS character devices PASS ===");
    } else {
        kprintln!("[bios] === BIOS character devices FAILED ({}) ===", fails);
    }
}
