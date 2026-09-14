//! The cursor's park position at workstation-open time.
//!
//! EmuTOS `vdimouse_init` (`vdi/vdi_mouse.c:582-583`) parks the cursor with
//!
//! ```c
//! GCURX = xres / 2;
//! GCURY = yres / 2;
//! ```
//!
//! and `xres`/`yres` are **not** the screen dimensions: `vdi/vdi_defs.h:94-95`
//! aliases them onto `DEV_TAB[0]`/`DEV_TAB[1]`, which `update_rez_dependent`
//! fills with `V_REZ_HZ - 1` / `V_REZ_VT - 1` (`vdi/vdi_control.c:203-204`) —
//! the **last** pixel coordinate, one less than the pixel count. At 640×480
//! that is `639/2, 479/2` = 319/239, not 320/240.
//!
//! The formula is checked at several geometries — including odd ones, where
//! halving the count and halving the last coordinate coincide — so that this
//! pins the *formula* rather than one memorised pair of numbers.

#[path = "vdi_reference/harness.rs"]
mod harness;

use tos_api::vdi::mouse;

/// EmuTOS' own expression, written out here in terms of the screen
/// dimensions, so the expected value does not come out of the function under
/// test: `xres = w - 1`, then `xres / 2`.
fn emutos_park(w: u16, h: u16) -> (i16, i16) {
    let xres = w as i16 - 1; // DEV_TAB[0], vdi_control.c:203
    let yres = h as i16 - 1; // DEV_TAB[1], vdi_control.c:204
    (xres / 2, yres / 2) // vdi_mouse.c:582-583
}

#[test]
fn park_position_halves_the_last_coordinate_not_the_pixel_count() {
    // Even widths/heights: the two readings differ by one pixel. Odd ones:
    // they coincide, which is what makes this a test of the formula.
    for &(w, h) in &[
        (640u16, 480u16),   // the TT/reference profile
        (320, 200),         // ST low
        (640, 400),         // ST high
        (641, 481),         // odd: both readings agree
        (1920, 1080),       // the Pi 4's panel
        (1, 1),             // degenerate: last coordinate is 0
    ] {
        assert_eq!(
            mouse::park_position(w, h),
            emutos_park(w, h),
            "park position at {w}x{h}"
        );
    }
}

/// The 640×480 case end to end, through the real trap: `v_opnwk` runs
/// `init_wk`, which runs `vdimouse_init`, and `vq_mouse` (opcode 124) reports
/// `GCURX`/`GCURY` in `PTSOUT[0..2]`.
#[test]
fn vq_mouse_reports_the_parked_centre_after_open() {
    let _guard = harness::VDI_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let wk = harness::workstation();
    let mut pb = harness::Pb::new();
    pb.poison();
    pb.contrl.fill(0);
    pb.contrl[0] = 124;
    pb.contrl[6] = wk.handle;
    pb.call();
    assert_eq!(
        (pb.ptsout[0], pb.ptsout[1]),
        emutos_park(harness::SCREEN_W, harness::SCREEN_H),
        "vq_mouse position after v_opnwk"
    );
}
