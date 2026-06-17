//! Workstation lifecycle — `v_opnvwk` / `v_opnwk` / `v_clsvwk` /
//! `v_clswk` / `v_clrwk` and `init_wk`.
//!
//! Spec: [`docs/api/vdi/workstation/`]. `init_wk` validates the
//! caller's `work_in` into the [`Vwk`](super::vwk::Vwk) and returns the
//! device capability tables (`DEV_TAB` → `INTOUT`, `SIZ_TAB` → `PTSOUT`).
//!
//! `DEV_TAB_ROM` / `SIZ_TAB_ROM` are the EmuTOS templates
//! (`vdi/vdi_control.c`), patched at runtime with the real screen
//! geometry and colour count from the [`RasterBackend`].

use super::backend::RasterBackend;
use super::vwk::{self, validate_color_index, Vwk};
use super::Vdipb;

// EmuTOS `vdi_defs.h` limits used in the capability tables.
const MAX_LINE_STYLE: i16 = 7;
const MAX_LINE_WIDTH: i16 = 40;
const MAX_FILL_PATTERN: i16 = 24;
const MAX_FILL_HATCH: i16 = 12;

/// `DEV_TAB` template (45 WORDs), EmuTOS `DEV_TAB_rom`. Indices
/// `[0],[1],[13],[35],[39]` are patched from the live device.
const DEV_TAB_ROM: [i16; 45] = [
    639, 399, 0, 372, 372, // 0..4  res-1 x/y, precision, pixel w/h
    3, MAX_LINE_STYLE, 0, 6, 8, // 5..9  font heights, line styles, lw flag, marker types/sizes
    1, MAX_FILL_PATTERN, MAX_FILL_HATCH, 2, 10, // 10..14 fonts, fill pats, hatches, colours, #GDP
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, // 15..24 GDP ids (bar..justified text)
    3, 0, 3, 3, 3, 0, 3, 0, 3, 2, // 25..34 GDP attribute codes #1..#10
    0, 1, 1, 0, 2, // 35..39 colour cap, text rot, polyfill, cell array, palette size
    2, 1, 1, 1, 2, // 40..44 #locator/valuator/choice/string devices, ws type
];

/// `SIZ_TAB` template (12 WORDs = 6 pairs), EmuTOS `SIZ_TAB_rom`.
const SIZ_TAB_ROM: [i16; 12] = [
    0, 7, 0, 7, // min/max char width/height
    1, 0, MAX_LINE_WIDTH, 0, // min line width, reserved, max line width, reserved
    15, 11, 120, 88, // min/max marker width/height
];

/// Populate a freshly-allocated `Vwk` from `work_in` (INTIN) and write
/// the device capability tables back into the parameter block. Mirrors
/// EmuTOS `init_wk`.
fn init_wk(vwk: &mut Vwk, pb: &Vdipb, backend: &dyn RasterBackend) {
    let xres = backend.width() as i16;
    let yres = backend.height() as i16;
    let ncol = backend.num_colors();

    // ── attributes from work_in (INTIN[1..10]); [0] device id, [5]
    //    font are ignored per spec. Validate-or-default each. ──
    let line_type = pb.intin(1);
    vwk.line_index = if (1..=MAX_LINE_STYLE).contains(&line_type) {
        line_type - 1
    } else {
        0 // DEF_LINE_STYLE (solid)
    };
    vwk.line_color = validate_color_index(pb.intin(2), ncol);

    let mark_type = pb.intin(3);
    vwk.mark_index = if (1..=6).contains(&mark_type) {
        mark_type - 1
    } else {
        2 // DEF_MARK_STYLE (asterisk)
    };
    vwk.mark_color = validate_color_index(pb.intin(4), ncol);

    vwk.text_color = validate_color_index(pb.intin(6), ncol);

    let fill_int = pb.intin(7);
    vwk.fill_style = if (0..=4).contains(&fill_int) { fill_int } else { 0 };
    vwk.fill_index = pb.intin(8); // refined against style by vsf_style later
    vwk.fill_color = validate_color_index(pb.intin(9), ncol);

    vwk.xfm_mode = pb.intin(10); // 0 = NDC, 2 = RC (screen is RC)

    // Non-work_in defaults: clip off → full screen (inclusive bounds).
    vwk.clip = false;
    vwk.xmn_clip = 0;
    vwk.ymn_clip = 0;
    vwk.xmx_clip = xres - 1;
    vwk.ymx_clip = yres - 1;

    // ── outputs: DEV_TAB → INTOUT (45), SIZ_TAB → PTSOUT (12) ──
    let mut dev = DEV_TAB_ROM;
    dev[0] = xres - 1;
    dev[1] = yres - 1;
    dev[13] = ncol as i16;
    dev[35] = if ncol <= 2 { 0 } else { 1 }; // colour capable?
    dev[39] = ncol as i16; // palette size
    for (i, &v) in dev.iter().enumerate() {
        pb.set_intout(i, v);
    }
    for (i, &v) in SIZ_TAB_ROM.iter().enumerate() {
        pb.set_ptsout(i, v);
    }

    // Output counts (the dispatcher pre-zeroed them).
    pb.set_contrl(4, 45);
    pb.set_contrl(2, 6);
}

/// Opcode 100 — open a virtual workstation.
pub fn v_opnvwk(pb: &Vdipb, backend: &dyn RasterBackend) {
    // SAFETY: single trap path.
    match unsafe { vwk::alloc_handle() } {
        Some(vwk) => {
            let handle = vwk.handle;
            init_wk(vwk, pb, backend);
            pb.set_contrl(6, handle);
        }
        None => pb.set_contrl(6, 0), // no free handle → fail (VDI: no error code)
    }
}

/// Opcode 1 — open the physical workstation (the screen device).
pub fn v_opnwk(pb: &Vdipb, backend: &dyn RasterBackend) {
    // SAFETY: single trap path.
    let vwk = unsafe { vwk::open_physical() };
    let handle = vwk.handle;
    init_wk(vwk, pb, backend);
    pb.set_contrl(6, handle);
}

/// Opcodes 101 / 2 — close a (virtual or physical) workstation.
pub fn v_clsvwk(handle: i16) {
    // SAFETY: single trap path.
    unsafe { vwk::free_handle(handle) };
}

/// Opcode 3 — clear the workstation (fill the screen with the
/// background colour, PEL 0).
pub fn v_clrwk(backend: &dyn RasterBackend) {
    backend.fill_rect(0, 0, backend.width() as i16, backend.height() as i16, 0);
}
