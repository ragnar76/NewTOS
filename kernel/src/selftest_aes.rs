//! Phase 13 AES Application-Manager self-test (D1/D2).
//!
//! Exercises the AES dispatcher + Application Manager through the **real
//! GEM trap path** (`svc #2`, selector 200 in `x0`, AESPB pointer in
//! `x1`): `appl_init` (fills AESGLOBAL, returns the app id), `appl_find`
//! (unknown name → −1), the message pipe (`appl_write`/`appl_read`), and
//! `appl_exit`.

use crate::kprintln;

/// Issue an AES call: the GEM trap with selector 200 in `x0` and the
/// AESPB pointer in `x1`.
#[inline(always)]
fn aes(aespb: u64) {
    // SAFETY: GEM trap; the AES returns results in the parameter block.
    unsafe {
        core::arch::asm!(
            "svc #2",
            inout("x0") 200u64 => _,
            in("x1") aespb,
            options(nostack),
        );
    }
}

/// Run the Phase 13 AES Application-Manager self-test (dispatcher +
/// `appl_*` lifecycle + message pipe) through the GEM trap path.
#[allow(unused_assignments)]
pub fn run_phase13_aes_test() {
    // AESPB arrays.
    let mut control = [0i16; 8];
    let mut global = [0i16; 32]; // AESGLOBAL (native layout) lives here
    let mut intin = [0i16; 16];
    let mut intout = [0i16; 16];
    let mut addrin = [0i64; 4];
    let mut addrout = [0i64; 4];
    let block: [u64; 6] = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    let pb = block.as_ptr() as u64;

    kprintln!("[aes] === Phase 13 AES Application-Manager test (D1/D2) ===");
    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[aes]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // ── appl_init (10) ──
    control = [0; 8];
    control[0] = 10;
    aes(pb);
    let ap_id = intout[0];
    check(ap_id >= 1, "appl_init returns an application id");
    // AESGLOBAL (native): ap_version @ word0, ap_count @ word1, ap_id @ word2.
    check(global[0] == 0x0340, "AESGLOBAL ap_version = 0x0340");
    check(global[1] == 1, "AESGLOBAL ap_count = 1");
    check(global[2] == ap_id, "AESGLOBAL ap_id = returned id");

    // ── appl_find (13) for an unknown name → −1 ──
    let name = *b"NOSUCH  ";
    addrin[0] = name.as_ptr() as i64;
    control = [0; 8];
    control[0] = 13;
    control[3] = 1; // one addrin
    aes(pb);
    check(intout[0] == -1, "appl_find(unknown) = -1");

    // ── message pipe: appl_write to self, then appl_read it back ──
    let mut msg = [0i16; 8];
    msg[0] = 0x1234; // a fake message type
    msg[3] = 42;
    addrin[0] = msg.as_ptr() as i64;
    control = [0; 8];
    control[0] = 12; // appl_write
    control[1] = 2; // two intin
    control[3] = 1;
    intin[0] = ap_id; // target = self
    intin[1] = 16; // 16-byte message
    aes(pb);
    check(intout[0] == 1, "appl_write delivers a message");

    let mut rx = [0i16; 8];
    addrin[0] = rx.as_mut_ptr() as i64;
    control = [0; 8];
    control[0] = 11; // appl_read
    control[1] = 2;
    control[3] = 1;
    intin[0] = ap_id;
    intin[1] = 16;
    aes(pb);
    check(intout[0] == 1, "appl_read returns the message");
    check(rx[0] == 0x1234 && rx[3] == 42, "message contents round-trip");

    // ── appl_exit (19) ──
    control = [0; 8];
    control[0] = 19;
    aes(pb);
    check(intout[0] == 1, "appl_exit returns 1");

    if fails == 0 {
        kprintln!("[aes] === AES Application-Manager PASS ===");
    } else {
        kprintln!("[aes] === AES Application-Manager FAILED ({}) ===", fails);
    }
}
