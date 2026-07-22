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
    // AESGLOBAL. gemlib's real array is `short aes_global[16]` = 32 bytes
    // (indices 0..15). This buffer is deliberately oversized: indices
    // 16..31 are a canary that appl_init must leave untouched — the fixed
    // LP64 writer stays inside the 32-byte array (the previous 56-byte
    // native-struct writer overran it and would trip this canary).
    let mut global = [0i16; 32];
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

    // Register a desktop-background tree so appl_init's `ap_private` has a
    // known value to read back — NewTOS's equivalent of EmuTOS
    // `rs_trees[DESKTOP][ROOT].ob_spec` (`gemsuper.c:96`) is the WF_NEWDESK
    // registered tree's root `ob_spec`. One G_BOX at index 0 whose `ob_spec`
    // is a distinctive desktop-background word (IP_4PATT|green | border/text).
    const TEST_BG_OBSPEC: u64 = 0x1143;
    let bg_tree = [tos_api::aes::object::Object {
        ob_next: -1,
        ob_head: -1,
        ob_tail: -1,
        ob_type: 20, // G_BOX
        ob_flags: 0,
        ob_state: 0,
        ob_spec: TEST_BG_OBSPEC,
        ob_x: 0,
        ob_y: 0,
        ob_width: 100,
        ob_height: 100,
    }];
    // wind_set(DESKWH=0, WF_NEWDESK=14, tree=&bg_tree, root=0).
    addrin[0] = bg_tree.as_ptr() as i64;
    control = [0; 8];
    control[0] = 105; // wind_set
    control[1] = 3; // three intin
    control[3] = 1; // one addrin
    intin[0] = 0; // DESKWH
    intin[1] = 14; // WF_NEWDESK
    intin[2] = 0; // root object index
    aes(pb);

    // The screen colour-plane count appl_init should report (same source the
    // AES uses for `gl_nplanes`): log2 of the framebuffer colour count.
    let expected_planes = tos_api::host::host()
        .and_then(|h| h.raster())
        .map(|r| r.num_colors().trailing_zeros() as i16)
        .unwrap_or(4);

    // ── appl_init (10) ──
    control = [0; 8];
    control[0] = 10;
    aes(pb);
    let ap_id = intout[0];
    check(ap_id >= 1, "appl_init returns an application id");
    // AESGLOBAL, gemlib LP64 layout (this is an in-kernel / EL1 caller →
    // Abi::Lp64): ap_version @ index 0, ap_count @ 1, ap_id @ 2, ap_private
    // (LONG @ byte 6 / index 3), ap_ptree/ap_rscmem/ap_rsclen 0 (indices
    // 4..9), ap_planes @ index 10 (byte 20), ap_3resv (4 bytes @ byte 22 /
    // index 11). See the tos-api aes::appl module ABI note.
    check(global[0] == 0x0340, "AESGLOBAL ap_version = 0x0340");
    check(global[1] == 1, "AESGLOBAL ap_count = 1");
    check(global[2] == ap_id, "AESGLOBAL ap_id = returned id");
    // ap_private = the registered desktop-background ROOT ob_spec.
    // SAFETY: `global` is a 32-byte `short[16]`; the LP64 `ap_private` LONG is
    // the 8 bytes at &global[3] (byte 6), read here unaligned.
    let ap_private = unsafe { (global.as_ptr().add(3) as *const u64).read_unaligned() };
    check(
        ap_private == TEST_BG_OBSPEC,
        "AESGLOBAL ap_private = registered WF_NEWDESK ROOT ob_spec (0x1143)",
    );
    check(global[10] == expected_planes, "AESGLOBAL ap_planes = screen colour planes");
    check(global[10] >= 1, "AESGLOBAL ap_planes non-zero (LP64 index 10, byte 20)");
    // ap_3resv (4 bytes at byte 22 / index 11) = AES process-table address.
    // SAFETY: within the 32-byte array; the 4-byte ap_3resv field is at byte 22.
    let ap_3resv = unsafe { (global.as_ptr().add(11) as *const u32).read_unaligned() };
    check(ap_3resv != 0, "AESGLOBAL ap_3resv (byte 22) = AES table addr, non-zero");
    check(
        (4..10).all(|i| global[i] == 0),
        "AESGLOBAL ap_ptree/ap_rscmem/ap_rsclen zero (indices 4..9)",
    );
    // ap_maxchar/ap_minchar (indices 13/14) stay 0 as at EmuTOS appl_init
    // (AES 0x0400+ fields, not reported at version 0x0340) — the 4-byte
    // ap_3resv must not spill into them.
    check(
        global[13] == 0 && global[14] == 0,
        "AESGLOBAL ap_maxchar/ap_minchar zero (0x0340)",
    );
    // No overrun: gemlib's array ends at index 15. The canary past it must
    // be pristine. The old 56-byte native-struct writer placed ap_planes
    // at byte 34 = index 17 and reached index 27 — this would fail then.
    check(
        (16..32).all(|i| global[i] == 0),
        "AESGLOBAL appl_init does not write past the 32-byte array",
    );

    // Un-register the test background tree (it is a stack array; the desktop
    // registers its own tree later during `desk::run`).
    addrin[0] = 0;
    control = [0; 8];
    control[0] = 105; // wind_set
    control[1] = 3;
    control[3] = 1;
    intin[0] = 0; // DESKWH
    intin[1] = 14; // WF_NEWDESK
    intin[2] = 0;
    aes(pb);

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

/// Read the host's millisecond clock (for timing the event waits).
fn millis() -> u64 {
    tos_api::host::host().map(|h| h.monotonic_millis()).unwrap_or(0)
}

/// Phase 13 D3 Event-Manager self-test, through the GEM trap path.
#[allow(unused_assignments)]
pub fn run_phase13_event_test() {
    let mut control = [0i16; 8];
    let mut global = [0i16; 32];
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

    kprintln!("[aes] === Phase 13 AES Event Manager test (D3) ===");
    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[aes]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // ── evnt_timer (24): wait 50 ms, return 1 ──
    let t0 = millis();
    control = [0; 8];
    control[0] = 24;
    control[1] = 2; // two intin (lo, hi)
    intin[0] = 50; // T_LOCOUNT
    intin[1] = 0; // T_HICOUNT
    aes(pb);
    let elapsed = millis().wrapping_sub(t0);
    check(intout[0] == 1, "evnt_timer returns 1");
    check(elapsed >= 50, "evnt_timer waited >= 50ms");

    // ── evnt_timer (24): zero count returns immediately ──
    control = [0; 8];
    control[0] = 24;
    control[1] = 2;
    intin[0] = 0;
    intin[1] = 0;
    intout[0] = 0;
    aes(pb);
    check(intout[0] == 1, "evnt_timer(0) returns 1 immediately");

    // Register the application (for the message pipe below).
    control = [0; 8];
    control[0] = 10; // appl_init
    aes(pb);
    let ap_id = intout[0];

    // ── evnt_keybd (20): a queued key is returned (ASCII in low byte) ──
    crate::console::push_rx(b'A' as u16);
    control = [0; 8];
    control[0] = 20;
    intout[0] = 0;
    aes(pb);
    check(intout[0] == 0x41, "evnt_keybd returns the queued key 'A'");

    // ── evnt_mesag (23): a written message is delivered ──
    let mut msg = [0i16; 8];
    msg[0] = 0x4242; // message type
    msg[5] = 7;
    addrin[0] = msg.as_ptr() as i64;
    control = [0; 8];
    control[0] = 12; // appl_write
    control[1] = 2;
    control[3] = 1;
    intin[0] = ap_id; // target = self
    intin[1] = 16;
    aes(pb);

    let mut rx = [0i16; 8];
    addrin[0] = rx.as_mut_ptr() as i64;
    control = [0; 8];
    control[0] = 23; // evnt_mesag
    control[3] = 1;
    intout[0] = 0;
    aes(pb);
    check(intout[0] == 1, "evnt_mesag returns 1");
    check(rx[0] == 0x4242 && rx[5] == 7, "evnt_mesag copies the message");

    // ── evnt_dclick (26): get default (3), set to 1, get back ──
    control = [0; 8];
    control[0] = 26;
    control[1] = 2;
    intin[0] = 0;
    intin[1] = 0; // setit = 0 → get
    intout[0] = 0;
    aes(pb);
    check(intout[0] == 3, "evnt_dclick default index = 3");
    control[0] = 26;
    intin[0] = 1;
    intin[1] = 1; // setit = 1 → set to 1
    intout[0] = 0;
    aes(pb);
    check(intout[0] == 1, "evnt_dclick set returns new index");

    // ── evnt_button (21): no buttons down, wait for state 0 (immediate) ──
    control = [0; 8];
    control[0] = 21;
    control[1] = 3;
    intin[0] = 1; // clicks
    intin[1] = 1; // mask = left
    intin[2] = 0; // want left up
    intout[0] = 99;
    intout[3] = 99;
    aes(pb);
    check(intout[0] == 1, "evnt_button returns the click count");
    check(intout[3] == 0, "evnt_button reports no buttons down");

    // ── evnt_mouse (22): leave an off-screen rect (already outside) ──
    control = [0; 8];
    control[0] = 22;
    control[1] = 5;
    intin[0] = 1; // flags = 1 (leave)
    intin[1] = 1000;
    intin[2] = 1000;
    intin[3] = 10;
    intin[4] = 10;
    intout[0] = 0;
    aes(pb);
    check(intout[0] == 1, "evnt_mouse returns 1");

    // ── evnt_multi (25): a queued key beats a long timer ──
    crate::console::push_rx(b'B' as u16);
    intin = [0i16; 16];
    control = [0; 8];
    control[0] = 25;
    intin[0] = 0x01 | 0x20; // MU_KEYBD | MU_TIMER
    intin[14] = 5000; // 5 s timer (should not win)
    intout = [0; 16];
    aes(pb);
    check(intout[0] == 0x01, "evnt_multi reports MU_KEYBD");
    check(intout[5] == 0x42, "evnt_multi returns key 'B' in EV_KRET");

    // ── evnt_multi (25): timer fires when nothing else is pending ──
    let t0 = millis();
    intin = [0i16; 16];
    control = [0; 8];
    control[0] = 25;
    intin[0] = 0x20; // MU_TIMER only
    intin[14] = 30; // 30 ms
    intout = [0; 16];
    aes(pb);
    let elapsed = millis().wrapping_sub(t0);
    check(intout[0] == 0x20, "evnt_multi reports MU_TIMER");
    check(elapsed >= 30, "evnt_multi timer waited >= 30ms");

    // Deregister.
    control = [0; 8];
    control[0] = 19; // appl_exit
    aes(pb);

    if fails == 0 {
        kprintln!("[aes] === AES Event Manager PASS ===");
    } else {
        kprintln!("[aes] === AES Event Manager FAILED ({}) ===", fails);
    }
}
