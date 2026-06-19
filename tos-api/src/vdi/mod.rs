//! VDI (Virtual Device Interface) — Trap #2.
//!
//! Spec: [`docs/api/vdi/_dispatcher.md`] and the per-opcode files under
//! `docs/api/vdi/`. Unlike GEMDOS/BIOS/XBIOS, the VDI is not
//! register-based: it takes a single **parameter-block** pointer (in
//! `x0`) and selects functions by an opcode in `contrl[0]`, passing all
//! inputs/outputs through five WORD arrays.
//!
//! Layering (see [`backend`]): this module is the hardware-independent
//! VDI layer — dispatch, the [`vwk`] attribute state, parameter
//! marshalling. The actual pixel work goes through the swappable
//! [`backend::RasterBackend`] the kernel installs via
//! [`crate::host::Host::raster`].
//!
//! **Milestone D1/D2:** dispatcher + workstation lifecycle
//! (`v_opnvwk`/`v_opnwk`/`v_clsvwk`/`v_clswk`/`v_clrwk`). Output,
//! attribute and inquiry opcodes are added in later milestones; until
//! then a non-routed opcode is a VDI no-op (the VDI has no error
//! return — see the dispatcher spec), **not** a silent stub of a
//! drawing call.

pub mod backend;
mod attributes;
mod contour;
mod escape;
mod fill_patterns;
pub mod font;
mod font_data;
mod gdp;
mod input;
mod inquiry;
mod mouse;
/// Input-subsystem entry point + queries: feed a relative-mouse event
/// (the kernel's IKBD mouse decoder calls this) and read back the cursor
/// state (used by the IKBD self-test).
pub use mouse::{buttons as mouse_buttons, feed_relative, position as mouse_position};
mod primitives;
mod raster;
mod text;
mod trig;
mod vwk;
mod workstation;

use crate::host;

/// The VDI parameter block: five pointers to WORD (`i16`) arrays. On
/// native AArch64 each pointer is 64-bit; the block is what `x0` points
/// at when the `svc #2` trap is taken.
///
/// Accessors read/write WORD slots directly. For the bring-up the VDI
/// is exercised from EL1 (kernel selftest), so the arrays are ordinary
/// kernel memory; EL0 user-pointer access (à la the GEMDOS path) is a
/// later concern when real programs issue Trap #2.
pub(crate) struct Vdipb {
    contrl: *mut i16,
    intin: *mut i16,
    ptsin: *mut i16,
    intout: *mut i16,
    ptsout: *mut i16,
}

impl Vdipb {
    /// Build from the raw `x0` pointer to the 5-pointer block.
    ///
    /// # Safety
    /// `ptr` must point at five consecutive valid array pointers, each
    /// to a WORD array large enough for the opcode's counts.
    pub(crate) unsafe fn from_raw(ptr: u64) -> Self {
        // SAFETY: caller guarantees the 5-pointer block is valid.
        let p = ptr as *const u64;
        unsafe {
            Self {
                contrl: p.add(0).read() as *mut i16,
                intin: p.add(1).read() as *mut i16,
                ptsin: p.add(2).read() as *mut i16,
                intout: p.add(3).read() as *mut i16,
                ptsout: p.add(4).read() as *mut i16,
            }
        }
    }

    #[inline]
    pub fn contrl(&self, i: usize) -> i16 {
        // SAFETY: callers index within the CONTRL array (≥ 7 WORDs).
        unsafe { self.contrl.add(i).read() }
    }

    /// Read a native 64-bit pointer packed into four consecutive CONTRL
    /// WORDs starting at index `i` (NewTOS native MFDB-pointer
    /// convention; see [`crate::vdi::raster`]).
    #[inline]
    pub fn contrl_u64(&self, i: usize) -> u64 {
        // SAFETY: callers ensure CONTRL has ≥ i+4 WORDs.
        unsafe { (self.contrl.add(i) as *const u64).read_unaligned() }
    }
    #[inline]
    pub fn set_contrl(&self, i: usize, v: i16) {
        // SAFETY: see `contrl`.
        unsafe { self.contrl.add(i).write(v) }
    }
    /// Write a native 64-bit pointer into four consecutive CONTRL WORDs
    /// (the inverse of [`contrl_u64`](Self::contrl_u64); used to return
    /// the previous routine pointer from the `vex_*` calls).
    #[inline]
    pub fn set_contrl_u64(&self, i: usize, v: u64) {
        // SAFETY: callers ensure CONTRL has ≥ i+4 WORDs.
        unsafe { (self.contrl.add(i) as *mut u64).write_unaligned(v) }
    }
    #[inline]
    pub fn intin(&self, i: usize) -> i16 {
        // SAFETY: callers index within `contrl[3]` input integers.
        unsafe { self.intin.add(i).read() }
    }
    #[inline]
    pub fn ptsin(&self, i: usize) -> i16 {
        // SAFETY: callers index within `contrl[1]*2` input coordinates.
        unsafe { self.ptsin.add(i).read() }
    }
    #[inline]
    pub fn set_intout(&self, i: usize, v: i16) {
        // SAFETY: callers index within `contrl[4]` output integers.
        unsafe { self.intout.add(i).write(v) }
    }
    #[inline]
    pub fn set_ptsout(&self, i: usize, v: i16) {
        // SAFETY: callers index within `contrl[2]*2` output coordinates.
        unsafe { self.ptsout.add(i).write(v) }
    }
}

// CONTRL indices with fixed meaning (see dispatcher spec).
const C_OPCODE: usize = 0;
const C_NPTSOUT: usize = 2;
const C_NINTOUT: usize = 4;
const C_HANDLE: usize = 6;

/// VDI dispatch entry. `vdipb_ptr` is the `x0` value at the `svc #2`
/// trap. The VDI returns nothing in a register — all results go into
/// the parameter-block arrays — so this returns `()`.
///
/// # Safety
/// `vdipb_ptr` must be a valid VDI parameter block (see [`Vdipb`]).
pub unsafe fn dispatch(vdipb_ptr: u64) {
    // SAFETY: caller guarantees a valid parameter block.
    let pb = unsafe { Vdipb::from_raw(vdipb_ptr) };

    let opcode = pb.contrl(C_OPCODE);
    // Default output counts; handlers refine them.
    pb.set_contrl(C_NPTSOUT, 0);
    pb.set_contrl(C_NINTOUT, 0);

    // Every primitive needs the device backend. With no backend
    // installed (hostless test, no framebuffer) the VDI is inert.
    let backend = match host::host().and_then(|h| h.raster()) {
        Some(b) => b,
        None => return,
    };

    match opcode {
        // ── workstation open (no handle required on input) ──
        1 => workstation::v_opnwk(&pb, backend),
        100 => workstation::v_opnvwk(&pb, backend),

        // ── ops needing a valid, open workstation ──
        2 | 101 => {
            if let Some(h) = open_handle(&pb) {
                workstation::v_clsvwk(h);
            }
        }
        3 if open_handle(&pb).is_some() => workstation::v_clrwk(backend),

        // ── attribute setters + output primitives (need the vwk) ──
        5 | 6 | 7 | 8 | 9 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 | 25
        | 26 | 28 | 30 | 31 | 32 | 33 | 35 | 36 | 37 | 38 | 39 | 103 | 104 | 105 | 106 | 107 | 108
        | 109 | 110 | 111 | 112 | 113 | 114 | 115 | 116 | 117 | 118 | 119 | 120 | 121 | 122 | 123
        | 124 | 125 | 126 | 127 | 128 | 129 | 130 | 131 | 134 => {
            // SAFETY: single trap path.
            if let Some(vwk) = unsafe { vwk::resolve(pb.contrl(C_HANDLE)) } {
                let ncol = backend.num_colors();
                match opcode {
                    14 => attributes::vs_color(&pb, ncol, backend),
                    15 => attributes::vsl_type(&pb, vwk),
                    16 => attributes::vsl_width(&pb, vwk),
                    17 => attributes::vsl_color(&pb, vwk, ncol),
                    108 => attributes::vsl_ends(&pb, vwk),
                    113 => attributes::vsl_udsty(&pb, vwk),
                    18 => attributes::vsm_type(&pb, vwk),
                    19 => attributes::vsm_height(&pb, vwk),
                    20 => attributes::vsm_color(&pb, vwk, ncol),
                    25 => attributes::vsf_color(&pb, vwk, ncol),
                    22 => attributes::vst_color(&pb, vwk, ncol),
                    23 => attributes::vsf_interior(&pb, vwk),
                    24 => attributes::vsf_style(&pb, vwk),
                    104 => attributes::vsf_perimeter(&pb, vwk),
                    32 => attributes::vswr_mode(&pb, vwk),
                    39 => attributes::vst_alignment(&pb, vwk),
                    106 => attributes::vst_effects(&pb, vwk),
                    13 => attributes::vst_rotation(&pb, vwk),
                    21 => attributes::vst_font(&pb, vwk),
                    12 => attributes::vst_height(&pb, vwk),
                    107 => attributes::vst_point(&pb, vwk),
                    112 => attributes::vsf_udpat(&pb, vwk),
                    129 => attributes::vs_clip(&pb, vwk, backend),
                    5 => escape::v_escape(&pb, backend),
                    119 => attributes::vst_load_fonts(&pb),
                    120 => attributes::vst_unload_fonts(&pb),
                    6 => primitives::v_pline(&pb, vwk, backend),
                    7 => primitives::v_pmarker(&pb, vwk, backend),
                    8 => text::v_gtext(&pb, vwk, backend),
                    9 => primitives::v_fillarea(&pb, vwk, backend),
                    11 => gdp::v_gdp(&pb, vwk, backend),
                    103 => contour::v_contourfill(&pb, vwk, backend),
                    // ── inquiry (read-only) ──
                    26 => inquiry::vq_color(&pb, ncol, backend),
                    35 => inquiry::vql_attributes(&pb, vwk),
                    36 => inquiry::vqm_attributes(&pb, vwk),
                    37 => inquiry::vqf_attributes(&pb, vwk),
                    38 => inquiry::vqt_attributes(&pb, vwk),
                    105 => inquiry::v_get_pixel(&pb, backend),
                    33 => input::vsin_mode(&pb),
                    115 => input::vqin_mode(&pb),
                    28 => input::v_locator(&pb),
                    30 => input::v_choice(&pb),
                    31 => input::v_string(&pb),
                    // ── mouse cursor + exchange vectors ──
                    111 => mouse::vsc_form(&pb, backend),
                    122 => mouse::v_show_c(&pb, backend),
                    123 => mouse::v_hide_c(backend),
                    124 => mouse::vq_mouse(&pb),
                    118 => mouse::vex_timv(&pb),
                    125 => mouse::vex_butv(&pb),
                    126 => mouse::vex_motv(&pb),
                    127 => mouse::vex_curv(&pb),
                    134 => mouse::vex_wheelv(&pb),
                    116 => inquiry::vqt_extent(&pb, vwk),
                    117 => inquiry::vqt_width(&pb, vwk),
                    128 => inquiry::vq_key_s(&pb),
                    130 => inquiry::vqt_name(&pb),
                    131 => inquiry::vqt_fontinfo(&pb, vwk),
                    114 => primitives::vr_recfl(&pb, vwk, backend),
                    121 => raster::vrt_cpyfm(&pb, vwk, backend, ncol),
                    109 => raster::vro_cpyfm(&pb, vwk, backend),
                    110 => raster::vr_trnfm(&pb),
                    _ => unreachable!(),
                }
            }
        }

        // ── genuine VDI no-ops + not-yet-implemented opcodes ──
        // (remaining output/inquiry/raster opcodes land in later
        // milestones; an unrouted opcode is a no-op by VDI semantics.)
        _ => {}
    }
}

/// Resolve `contrl[6]` to an open handle, returning it if valid.
fn open_handle(pb: &Vdipb) -> Option<i16> {
    let handle = pb.contrl(C_HANDLE);
    // SAFETY: single trap path.
    if unsafe { vwk::resolve(handle).is_some() } {
        Some(handle)
    } else {
        None
    }
}
