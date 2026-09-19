//! `graf_mouse` (78) with a form number the AES does not know.
//!
//! EmuTOS `gr_mouse` (`references/emutos/aes/gemgrlib.c:419-455`, pinned tree
//! `1a7484ab`) is `void` and has no error path. Its `switch(mode)` names
//! `USER_DEF`, `M_OFF`, `M_ON`, `M_SAVE`, `M_RESTORE` and `M_PREVIOUS`;
//! **everything else** — the eight presets and every number that is not one —
//! lands in `default:` (`:426-429`):
//!
//! ```c
//! if ((mode < ARROW) || (mode > OUTLN_CROSS))
//!     mode = ARROW;       /* fail safe */
//! maddr = mouse_cursor[mode];
//! break;
//! ```
//!
//! and the `break` falls through to `gsx_mfset(maddr)` (`:454`). So a call
//! with 8, 254, 261 or -1 does not leave the cursor as it was: it **installs
//! the arrow**. NewTOS had `_ => {}` for those numbers — the busy bee stayed
//! on screen. Finding A-3 of the 2026-09-12 sub-function audit
//! (`docs/api/aes/_full-function-inventory.md`, part II), ticket
//! `TASK/A44.md`.
//!
//! **How the active form is observed.** `graf_mouse` returns 1 whatever the
//! number (`crysbind` `ret = TRUE`, `gemsuper.c:85`), and neither the AES nor
//! the VDI has a call that reads the cursor form back. What `gsx_mfset` does
//! is *draw* the form, so the test reads it where it lands: the harness's
//! memory raster. The cursor is hidden (`v_hide_c`), a square around the
//! pointer is painted in a colour the sprite never uses, the cursor is shown
//! again (`v_show_c`), and the 16×16 image the VDI's cursor draw produces
//! (`tos_api::vdi::mouse`: data row → pen 1, otherwise mask row → pen 0,
//! hotspot-relative to the pointer position) is compared against the image
//! `PRESET_FORMS[n]` must produce. The arrow and the hourglass differ in
//! bitmap *and* hotspot, so "still the hourglass" and "now the arrow" are two
//! different pictures in two different places. (At the reference geometry's
//! four planes the workstation seeds no pseudo-palette, so the harness raster
//! maps pen 0 and pen 1 to the same device word; the eight pictures are still
//! pairwise distinct by shape and hotspot, and the test checks that they are
//! before it relies on it.)
//!
//! The cursor is one process-wide sprite, and every step depends on the one
//! before it, so everything runs inside one test function.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, ABI, AES_LOCK, SCREEN_H, SCREEN_W};
use tos_api::vdi::backend::RasterBackend;
use tos_api::vdi::mouse::PRESET_FORMS;

const OP_GRAF_MOUSE: i16 = 78;
const OP_GRAF_MKSTATE: i16 = 79;
const VDI_V_SHOW_C: i16 = 122;
const VDI_V_HIDE_C: i16 = 123;

/// `aesdefs.h:424-438`.
const ARROW: i16 = 0;
const HOURGLASS: i16 = 2;
const OUTLN_CROSS: i16 = 7;
const USER_DEF: i16 = 255;
const M_PREVIOUS: i16 = 260;

/// A device pixel no preset form paints: the sprite uses the workstation's
/// pen 0 and pen 1 only.
const BACKDROP: u32 = 0x00FF_00FF;

/// Half the side of the square that is inspected around the pointer. The
/// sprite is 16×16 and a hotspot is at most 15 pixels off its corner, so 24
/// covers every placement of a preset with room to spare on all sides.
const HALF: i16 = 24;
const SIDE: usize = (2 * HALF) as usize;

/// `graf_mouse(number)` through the LP64 dispatcher; `addr_in[0]` stays the
/// harness poison — `gr_mouse` only reads it for `USER_DEF`.
fn graf_mouse(pb: &mut Pb, number: i16) {
    pb.poison();
    pb.intin[0] = number;
    pb.call(OP_GRAF_MOUSE, 1, 1, 1, 0);
    assert_eq!(pb.intout[0], 1, "graf_mouse({number}) return");
}

/// `graf_mkstate`: the pointer position the cursor is drawn relative to.
fn pointer(pb: &mut Pb) -> (i16, i16) {
    pb.poison();
    pb.call(OP_GRAF_MKSTATE, 0, 5, 0, 0);
    assert_eq!(pb.intout[0], 1);
    (pb.intout[1], pb.intout[2])
}

fn backend() -> &'static dyn RasterBackend {
    tos_api::host::host()
        .expect("harness host installed")
        .raster()
        .expect("harness raster")
}

/// One VDI call on the context's workstation: `v_hide_c` (no input) or
/// `v_show_c(reset)` (`intin[0]`), through the same dispatcher the
/// application trap reaches. `v_show_c(0)` is the force-show
/// (`vdi_mouse.c:1125-1131`): whatever the hide depth, one call uncovers.
fn vdi_cursor(handle: i16, opcode: i16, intin0: Option<i16>) {
    let mut contrl = [0i16; 12];
    let mut intin = [0i16; 8];
    let ptsin = [0i16; 8];
    let mut intout = [0i16; 8];
    let mut ptsout = [0i16; 8];
    contrl[0] = opcode;
    contrl[6] = handle;
    if let Some(v) = intin0 {
        contrl[3] = 1;
        intin[0] = v;
    }
    let block = [
        contrl.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        ptsin.as_ptr() as u64,
        intout.as_mut_ptr() as u64,
        ptsout.as_mut_ptr() as u64,
    ];
    // SAFETY: the five arrays outlive the call; neither opcode writes output.
    unsafe { tos_api::vdi::dispatch(block.as_ptr() as u64, ABI) };
}

/// The square around the pointer, row-major.
fn region(mx: i16, my: i16) -> impl Iterator<Item = (i16, i16)> {
    (my - HALF..my + HALF).flat_map(move |y| (mx - HALF..mx + HALF).map(move |x| (x, y)))
}

/// Paint the inspected square with [`BACKDROP`] while the cursor is off it.
fn lay_backdrop(b: &dyn RasterBackend, mx: i16, my: i16) {
    for (x, y) in region(mx, my) {
        b.put_device_pixel(x, y, BACKDROP);
    }
}

fn snapshot(b: &dyn RasterBackend, mx: i16, my: i16) -> Vec<u32> {
    region(mx, my)
        .map(|(x, y)| b.get_device_pixel(x, y))
        .collect()
}

/// The picture preset `n` must leave in the square when the pointer is at
/// `(mx, my)`: EmuTOS's cursor draw, restated — the sprite's origin is the
/// pointer minus the hotspot, a set data bit paints pen 1, an otherwise set
/// mask bit paints pen 0, and everything else is left alone.
fn expected(b: &dyn RasterBackend, mx: i16, my: i16, n: i16) -> Vec<u32> {
    let (hx, hy, mask, data) = &PRESET_FORMS[n as usize];
    let fg = b.rgb565_to_device(b.pel_to_device16(1));
    let bg = b.rgb565_to_device(b.pel_to_device16(0));
    assert_ne!(fg, BACKDROP);
    assert_ne!(bg, BACKDROP);
    let (ox, oy) = (mx - hx, my - hy);
    region(mx, my)
        .map(|(x, y)| {
            let (dx, dy) = (x - ox, y - oy);
            if !(0..16).contains(&dx) || !(0..16).contains(&dy) {
                return BACKDROP;
            }
            let bit = 0x8000u16 >> dx;
            if data[dy as usize] & bit != 0 {
                fg
            } else if mask[dy as usize] & bit != 0 {
                bg
            } else {
                BACKDROP
            }
        })
        .collect()
}

/// Which preset is on screen right now, or `None` if the picture is none of
/// the eight. Hides the cursor, lays the backdrop, shows the cursor, reads.
fn shown_preset(b: &dyn RasterBackend, handle: i16, mx: i16, my: i16) -> Option<i16> {
    vdi_cursor(handle, VDI_V_HIDE_C, None);
    lay_backdrop(b, mx, my);
    vdi_cursor(handle, VDI_V_SHOW_C, Some(0));
    let got = snapshot(b, mx, my);
    (0..PRESET_FORMS.len() as i16).find(|&n| expected(b, mx, my, n) == got)
}

#[test]
fn an_unknown_form_number_installs_the_arrow() {
    let _guard = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let wk = context().handle;
    let b = backend();
    let mut pb = Pb::new();

    let (mx, my) = pointer(&mut pb);
    assert!(
        mx - HALF >= 0
            && my - HALF >= 0
            && mx + HALF <= SCREEN_W as i16
            && my + HALF <= SCREEN_H as i16,
        "pointer ({mx},{my}) too close to the edge for a {SIDE}-pixel square"
    );

    // The observation channel itself: the eight expected pictures are
    // pairwise distinct, every preset is told apart from every other one on
    // screen, and a valid number keeps meaning exactly its own form.
    for n in ARROW..=OUTLN_CROSS {
        for m in n + 1..=OUTLN_CROSS {
            assert_ne!(
                expected(b, mx, my, n),
                expected(b, mx, my, m),
                "presets {n} and {m}"
            );
        }
    }
    for n in ARROW..=OUTLN_CROSS {
        graf_mouse(&mut pb, n);
        assert_eq!(shown_preset(b, wk, mx, my), Some(n), "graf_mouse({n})");
    }

    // The finding: from the busy bee, an unknown number ends on the arrow —
    // `default:` clamps and then installs (`gemgrlib.c:426-429`, `:454`).
    for bad in [
        99i16,
        -1,
        OUTLN_CROSS + 1,
        USER_DEF - 1,
        M_PREVIOUS + 1,
        i16::MAX,
        i16::MIN,
    ] {
        graf_mouse(&mut pb, HOURGLASS);
        assert_eq!(
            shown_preset(b, wk, mx, my),
            Some(HOURGLASS),
            "prelude for graf_mouse({bad})"
        );
        graf_mouse(&mut pb, bad);
        assert_eq!(
            shown_preset(b, wk, mx, my),
            Some(ARROW),
            "graf_mouse({bad}) must install ARROW, not leave the cursor as it was"
        );
    }

    // And it is a real install, not a coincidence of "arrow was previous":
    // `gsx_mfset` records the outgoing form, so M_PREVIOUS brings the busy
    // bee back — exactly as it would after an explicit graf_mouse(ARROW).
    graf_mouse(&mut pb, HOURGLASS);
    graf_mouse(&mut pb, 99);
    graf_mouse(&mut pb, M_PREVIOUS);
    assert_eq!(
        shown_preset(b, wk, mx, my),
        Some(HOURGLASS),
        "M_PREVIOUS after an unknown number"
    );
}
