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

use super::backend::{ClipRect, RasterBackend};
use super::vwk::{self, validate_color_index, Vwk};
use super::{fill_patterns, font, Vdipb};

// EmuTOS `vdi_defs.h` limits used in the capability tables.
const MAX_LINE_STYLE: i16 = 7;
const MAX_LINE_WIDTH: i16 = 40;
const MAX_FILL_PATTERN: i16 = 24;
const MAX_FILL_HATCH: i16 = 12;
const FIS_PATTERN: i16 = 2; // interior style: dither pattern

/// `DEV_TAB[39]` — the DAC's RGB choice count (`get_palette()`, EmuTOS
/// `bios/screen.c:831-869`). Under a VIDEL truecolor mode `get_palette()`
/// returns **0** (a real non-ST-compatible truecolor mode has
/// `mode & VIDEL_COMPAT == 0` and `bpp == VIDEL_TRUECOLOR != VIDEL_4BPP`,
/// so it falls through to `return 0`). This is the Falcon030/TOS4 target
/// value; the previous 4096 was correct only for the earlier 16-colour
/// VIDEL-4bpp target.
const PALETTE_PRECISION: i16 = 0;

/// `DEV_TAB` template (45 WORDs), EmuTOS `DEV_TAB_rom`. Indices
/// `[0],[1],[3],[4],[13],[35],[39]` are patched from the live device by
/// [`init_wk`] (EmuTOS `update_rez_dependent`, `vdi_control.c:203-208`).
/// `[3]/[4]` (pixel size) carry the ROM's ST-High placeholder 372 here
/// and are overwritten with [`get_std_pixel_size`] at open time.
const DEV_TAB_ROM: [i16; 45] = [
    639, 399, 0, 372, 372, // 0..4  res-1 x/y, precision, pixel w/h (patched)
    // [5] font heights = 3: EmuTOS `text_init` sets DEV_TAB[5] to the number
    // of system-font (font_id==1) sizes; the Falcon target has three
    // (6x6/8x8/8x16). VW3-dependent: only 8x16 is present today, so the
    // value is correct for the target profile but currently over-announces
    // the installed faces (the 6x6/8x8 faces land in VW3).
    3, MAX_LINE_STYLE, 0, 6, 8, // 5..9  font heights, line styles, lw flag, marker types/sizes
    1, MAX_FILL_PATTERN, MAX_FILL_HATCH, 2, 10, // 10..14 fonts, fill pats, hatches, colours, #GDP
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, // 15..24 GDP ids (bar..justified text)
    3, 0, 3, 3, 3, 0, 3, 0, 3, 2, // 25..34 GDP attribute codes #1..#10
    0, 1, 1, 0, 2, // 35..39 colour cap, text rot, polyfill, cell array, palette size
    2, 1, 1, 1, 2, // 40..44 #locator/valuator/choice/string devices, ws type
];

/// `SIZ_TAB` template (12 WORDs = 6 pairs), EmuTOS `SIZ_TAB_rom`. The
/// character-size fields `[0..3]` are placeholders here (EmuTOS' ROM
/// template is likewise `{0,7,0,7}`); [`siz_tab`] overwrites them with the
/// `text_init` fold before the table is returned.
const SIZ_TAB_ROM: [i16; 12] = [
    0, 7, 0, 7, // min/max char width/top — placeholder, filled by `siz_tab()`
    1, 0, MAX_LINE_WIDTH, 0, // min line width, reserved, max line width, reserved
    15, 11, 120, 88, // min/max marker width/height
];

/// Maximum polygon vertices (`INQ_TAB[14]`), EmuTOS `MAX_VERTICES`. Shares
/// the one definition with the real polygon cap in [`super::primitives`]
/// (`config.h` make-512 profile = 512).
const MAX_VERTICES: i16 = super::primitives::MAX_VERTS as i16;

/// `INQ_TAB` template (45 WORDs), EmuTOS `INQ_TAB_rom` — the extended
/// workstation capabilities returned by `vq_extnd` mode 1. Indices
/// `[4]` (planes), `[5]` (video LUT) and `[19]` (clip flag) are patched
/// from the live device / workstation.
const INQ_TAB_ROM: [i16; 45] = [
    4, 1, 0x1F, 0, 1, // 0..4  controller type, bg cols, text styles, scale rasters, planes
    0, 50, 1, 1, 4, // 5..9  video LUT, perf factor, contour fill, char rotation, writing modes
    2, 1, 0, 0, MAX_VERTICES, // 10..14 highest input mode, text align, inking, rubber band, max verts
    -1, 1, 0, 0, 0, // 15..19 max intin, mouse buttons, wide-line style/mode, clip flag
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 20..29 extended precision / bezier
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 30..39 raster flag / reserved
    0, 0, 0, 0, 0, // 40..44 printer borders / page size
];

/// EmuTOS' "standard" pixel size in 1/1000 mm, computed from the live
/// resolution (`bios/screen.c:872-876`, `get_std_pixel_size`, the VIDEL /
/// TT path taken by the Falcon030/TOS4 target profile — `HAS_VIDEL`).
/// Patched into `DEV_TAB[3]/[4]` by [`init_wk`] via `update_rez_dependent`
/// (`vdi_control.c:205`). For the live 1920×1080 framebuffer this yields
/// 278/278 (never the ROM template's ST-High 372).
fn get_std_pixel_size(xres: i16, yres: i16) -> (i16, i16) {
    let width = if xres < 640 { 556 } else { 278 };
    let height = if yres < 400 { 556 } else { 278 };
    (width, height)
}

/// Cell metrics `(max_char_width, top)` of every system font (`font_id ==
/// 1`), in font-ring order. EmuTOS' `text_init` (`vdi_text.c:598-663`)
/// folds `SIZ_TAB[0..3]` from the min/max of these over the whole ring.
///
/// **VW3-dependent:** only the 8×16 face exists today, so the fold yields
/// `{7,13,7,13}` (`min_cw=max_cw=7`, `min_top=max_top=13`). Adding the
/// 6×6 / 8×8 system faces in VW3 extends this array and the fold then
/// yields EmuTOS' full `{5,4,7,13}` — no change to the folding code below.
const SYSTEM_FONT_METRICS: &[(i16, i16)] = &[(font::MAX_CHAR_WIDTH, font::TOP)];

/// `SIZ_TAB` (12 WORDs) with the character-size fields `[0..3]` filled by
/// the `text_init` fold (EmuTOS `vdi_text.c:598-642`) rather than left at
/// the ROM template's placeholder `{0,7,0,7}`. `[4..11]` (line / marker
/// sizes) keep the ROM values. `[0]` = min char width, `[1]` = min top,
/// `[2]` = max char width, `[3]` = max top.
fn siz_tab() -> [i16; 12] {
    let mut siz = SIZ_TAB_ROM;
    let (mut min_cw, mut min_top, mut max_cw, mut max_top) = (i16::MAX, i16::MAX, 0i16, 0i16);
    for &(cw, top) in SYSTEM_FONT_METRICS {
        min_cw = min_cw.min(cw);
        min_top = min_top.min(top);
        max_cw = max_cw.max(cw);
        max_top = max_top.max(top);
    }
    siz[0] = min_cw;
    siz[1] = min_top;
    siz[2] = max_cw;
    siz[3] = max_top;
    siz
}

/// Populate a freshly-allocated `Vwk` from `work_in` (INTIN) and write
/// the device capability tables back into the parameter block. Mirrors
/// EmuTOS `init_wk`.
fn init_wk(vwk: &mut Vwk, pb: &Vdipb, backend: &dyn RasterBackend) {
    let xres = backend.width() as i16;
    let yres = backend.height() as i16;
    let ncol = backend.num_colors();
    let planes = backend.planes();

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
    // Clamp work_in[8] to the current interior style's index range
    // (EmuTOS `vdi_control.c:305-311`). NB: `init_wk` stores the *1-based*
    // validated value (no `-1`), unlike `vsf_style` which stores it
    // 0-based — the deliberate D-VDI-8 quirk, preserved here.
    let fi = pb.intin(8);
    let max = if vwk.fill_style == FIS_PATTERN { MAX_FILL_PATTERN } else { MAX_FILL_HATCH };
    vwk.fill_index = if (1..=max).contains(&fi) { fi } else { 1 };
    vwk.fill_color = validate_color_index(pb.intin(9), ncol);

    vwk.xfm_mode = pb.intin(10); // 0 = NDC, 2 = RC (screen is RC)

    // Non-work_in defaults: clip off → full screen (inclusive bounds).
    vwk.clip = false;
    vwk.xmn_clip = 0;
    vwk.ymn_clip = 0;
    vwk.xmx_clip = xres - 1;
    vwk.ymx_clip = yres - 1;

    // Default user-defined fill pattern = the ROM pattern (EmuTOS
    // `init_wk`, `vdi_control.c:335-339`), so `vsf_interior(FIS_USER)`
    // before any `vsf_udpat` fills with it instead of with zeroes.
    vwk.ud_patrn = fill_patterns::ROM_UD_PATRN;

    // ── outputs: DEV_TAB → INTOUT (45), SIZ_TAB → PTSOUT (12) ──
    let (pw, ph) = get_std_pixel_size(xres, yres);
    let mut dev = DEV_TAB_ROM;
    dev[0] = xres - 1;
    dev[1] = yres - 1;
    dev[3] = pw; // pixel width  (get_pixel_size)
    dev[4] = ph; // pixel height (get_pixel_size)
    dev[13] = ncol as i16;
    dev[35] = if planes == 1 { 0 } else { 1 }; // colour capable? (v_planes==1)
    dev[39] = PALETTE_PRECISION; // DAC RGB-choice count (get_palette): 0 for truecolor
    for (i, &v) in dev.iter().enumerate() {
        pb.set_intout(i, v);
    }
    for (i, &v) in siz_tab().iter().enumerate() {
        pb.set_ptsout(i, v);
    }

    // Output counts (the dispatcher pre-zeroed them).
    pb.set_contrl(4, 45);
    pb.set_contrl(2, 6);

    // ── truecolor: seed this workstation's private pseudo-palette ──
    // Under TRUECOLOR_MODE (v_planes > 8) each workstation gets its own
    // colour tables, seeded from the device-global default (EmuTOS
    // `init_wk`, `vdi_control.c:354-363`: copy `REQ_COL`/`req_col2` into
    // `ext->req_col`, then `set_color16` for each pen). NewTOS' device
    // default is the backend's 256-entry palette, exposed as the requested
    // RGB via `query_color`.
    if planes > 8 {
        // SAFETY: single trap path; `vwk.handle` is the just-opened ws.
        if let Some(ext) = unsafe { vwk::ext(vwk.handle) } {
            for (l, slot) in ext.req_col.iter_mut().enumerate() {
                let (r, g, b) = backend.query_color(l as u16);
                *slot = [r as i16, g as i16, b as i16];
            }
            // Build the pseudo-palette for pens 0..254 (EmuTOS loops
            // `l < 255`, `vdi_control.c:360`): pen 255 maps to hardware
            // register 15, which no seeded pen writes, so its slot stays 0
            // — folded, `palette[255]` likewise stays 0 (its default is
            // black anyway), so `vq_color(255, flag=1)` is `(0,0,0)` as in
            // EmuTOS. The default req_col is already in 0..1000 range.
            for l in 0..255usize {
                let rgb = ext.req_col[l];
                ext.set_color16(l, rgb);
            }
        }
    }

    // Materialise the parked cursor centre + default arrow form now (EmuTOS
    // `vdimouse_init`), so `vq_mouse`/`v_locator` report the centre before the
    // first explicit cursor op instead of the power-up (0, 0). Idempotent.
    super::mouse::init(backend);
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

/// Opcode 102 — `vq_extnd`: extended workstation inquiry (EmuTOS
/// `vdi_vq_extnd`). Mode 0 (`INTIN[0] == 0`) re-returns the standard open
/// tables (`DEV_TAB` → `INTOUT`, `SIZ_TAB` → `PTSOUT`); mode ≠ 0 returns the
/// extended `INQ_TAB` plus the workstation's current clipping rectangle.
pub fn vq_extnd(pb: &Vdipb, vwk: &crate::vdi::vwk::Vwk, backend: &dyn RasterBackend) {
    let xres = backend.width() as i16;
    let yres = backend.height() as i16;
    let ncol = backend.num_colors();
    let planes = backend.planes();

    if pb.intin(0) == 0 {
        // Mode 0: identical to v_opnvwk's device tables.
        let (pw, ph) = get_std_pixel_size(xres, yres);
        let mut dev = DEV_TAB_ROM;
        dev[0] = xres - 1;
        dev[1] = yres - 1;
        dev[3] = pw; // pixel width  (get_pixel_size)
        dev[4] = ph; // pixel height (get_pixel_size)
        dev[13] = ncol as i16;
        dev[35] = if planes == 1 { 0 } else { 1 }; // colour capable? (v_planes==1)
        dev[39] = PALETTE_PRECISION; // DAC RGB-choice count (get_palette): 0 for truecolor
        for (i, &v) in dev.iter().enumerate() {
            pb.set_intout(i, v);
        }
        for (i, &v) in siz_tab().iter().enumerate() {
            pb.set_ptsout(i, v);
        }
    } else {
        // Mode 1: the extended table + the live clipping rectangle.
        let mut inq = INQ_TAB_ROM;
        inq[4] = planes as i16; // number of planes = v_planes (vdi_control.c:210)
        // Video lookup table present: 0 for truecolor (v_planes==16) or a
        // mono monitor, 1 otherwise (vdi_control.c:211-213). The colour-only
        // framebuffer never reports MON_MONO, so this reduces to the
        // truecolor test.
        inq[5] = if planes == 16 { 0 } else { 1 };
        inq[19] = i16::from(vwk.clip); // live clip flag
        for (i, &v) in inq.iter().enumerate() {
            pb.set_intout(i, v);
        }
        pb.set_ptsout(0, vwk.xmn_clip);
        pb.set_ptsout(1, vwk.ymn_clip);
        pb.set_ptsout(2, vwk.xmx_clip);
        pb.set_ptsout(3, vwk.ymx_clip);
        for i in 4..12 {
            pb.set_ptsout(i, 0);
        }
    }
    pb.set_contrl(4, 45); // # INTOUT
    pb.set_contrl(2, 6); // # PTSOUT (6 pairs)
}

/// Opcode 1 — open the physical workstation (the screen device).
pub fn v_opnwk(pb: &Vdipb, backend: &dyn RasterBackend) {
    // SAFETY: single trap path.
    let vwk = unsafe { vwk::open_physical() };
    let handle = vwk.handle;
    init_wk(vwk, pb, backend);
    pb.set_contrl(6, handle);
}

/// Opcode 2 — `v_clswk`: close the physical workstation. EmuTOS
/// `vdi_v_clswk` (`vdi_control.c:573-590`) closes **all** open virtual
/// workstations (handles 2..=`NUM_VDI_HANDLES`) and leaves the physical
/// workstation (handle 1) open, making it line-A's current workstation
/// again. This is the opposite of the previous NewTOS behaviour, which
/// aliased opcode 2 onto `v_clsvwk` and so closed the *physical* screen
/// device on the standard standalone-GEM call (AUD-V1-001).
///
/// The timer / mouse / VT52-alpha teardown (`timer_exit` /
/// `vdimouse_exit` / `esc_exit`) that EmuTOS also runs here is out of this
/// package's scope — the mouse/input (VW6) and escape (VW5) audits own it
/// — and NewTOS does not model line-A's `CUR_WORK` (documented acceptable
/// deviation AUD-V1-010).
pub fn v_clswk() {
    // SAFETY: single trap path.
    unsafe { vwk::close_all_virtual() };
}

/// Opcode 101 — `v_clsvwk`: close a single virtual workstation. EmuTOS
/// `vdi_v_clsvwk` (`vdi_control.c:462-491`) refuses to close the physical
/// workstation this way: `if (handle == VDI_PHYS_HANDLE) return;`
/// (AUD-V1-002). Closing an already-closed handle is a no-op (the
/// dispatcher already resolved it to an open workstation).
pub fn v_clsvwk(handle: i16) {
    if handle == vwk::PHYS_HANDLE {
        return; // can't close the physical workstation this way
    }
    // SAFETY: single trap path.
    unsafe { vwk::free_handle(handle) };
}

/// Opcode 3 — clear the workstation (fill the screen with the
/// background colour, PEL 0).
pub fn v_clrwk(backend: &dyn RasterBackend) {
    let (w, h) = (backend.width(), backend.height());
    backend.fill_rect(0, 0, w as i16, h as i16, 0, ClipRect::full(w, h));
}
