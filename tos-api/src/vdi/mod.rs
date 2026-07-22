//! VDI (Virtual Device Interface) — Trap #2.
//!
//! Spec: [`docs/api/vdi/_dispatcher.md`] and the per-opcode files under
//! `docs/api/vdi/`. Unlike GEMDOS/BIOS/XBIOS, the VDI is not
//! register-based: the kernel routes selector 115 (`x0`) here with a
//! single **parameter-block** pointer (`x1`), and functions are selected
//! by an opcode in `contrl[0]`, passing all inputs/outputs through five
//! WORD arrays.
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
pub mod mouse;
/// Input-subsystem entry point + queries: feed a relative-mouse event
/// (the kernel's IKBD mouse decoder calls this) and read back the cursor
/// state (used by the IKBD self-test).
pub use mouse::{
    buttons as mouse_buttons, feed_relative, hide as cursor_hide, position as mouse_position,
    show as cursor_show, show_reset as cursor_show_reset,
};
mod primitives;
mod raster;
/// AES form-manager screen save/restore (`bb_save`/`bb_restore`): a
/// `vro_cpyfm` Screen↔Memory copy over a caller-owned 32-bpp device-format
/// buffer. Used by `form_alert`/`form_dial` to preserve the screen behind
/// a dialog. See [`raster`] for the memory-MFDB format.
pub use raster::{bb_restore, bb_save, bb_save_words};
mod req_col;
mod text;
mod trig;
mod vwk;
mod workstation;

use crate::bdos::process::Abi;
use crate::host;

/// The VDI parameter block: five pointers to WORD (`i16`) arrays
/// (`contrl`/`intin`/`ptsin`/`intout`/`ptsout`). The five block pointers
/// are the caller's pointer width — 8-byte for an LP64 caller, 4-byte for
/// an ILP32 one (ADR-020) — so [`Vdipb::from_raw`] is told the caller's
/// [`Abi`] and reads them at the right stride. The arrays they address
/// are WORD arrays (16-bit either way), so the accessors keep plain
/// `*mut i16`.
pub(crate) struct Vdipb {
    contrl: *mut i16,
    intin: *mut i16,
    ptsin: *mut i16,
    intout: *mut i16,
    ptsout: *mut i16,
    /// The calling process's pointer-width ABI (ADR-020), recorded at
    /// construction. The raster ops need it to read the CONTRL MFDB
    /// pointers (`N_PTRINTS` = 4 LP64 / 2 ILP32) and the MFDB struct
    /// fields at the caller's width — see [`Vdipb::contrl_ptr`] and
    /// [`crate::vdi::raster`].
    abi: Abi,
}

impl Vdipb {
    /// Build from the raw `x1` pointer to the 5-pointer block, reading
    /// the block pointers at the caller's pointer width (`abi`): 8-byte
    /// LP64 pointers, or 4-byte (zero-extended) ILP32 pointers.
    ///
    /// # Safety
    /// `ptr` must point at five consecutive valid array pointers of the
    /// caller's width, each to a WORD array large enough for the opcode's
    /// counts.
    pub(crate) unsafe fn from_raw(ptr: u64, abi: Abi) -> Self {
        // SAFETY: caller guarantees the 5-pointer block is valid and of
        // `abi`'s width; `read_ptr` reads one such pointer per index.
        unsafe {
            Self {
                contrl: abi.read_ptr(ptr, 0) as *mut i16,
                intin: abi.read_ptr(ptr, 1) as *mut i16,
                ptsin: abi.read_ptr(ptr, 2) as *mut i16,
                intout: abi.read_ptr(ptr, 3) as *mut i16,
                ptsout: abi.read_ptr(ptr, 4) as *mut i16,
                abi,
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
    ///
    /// This is the fixed **LP64** packing (4 WORDs / 8 bytes at word `i`),
    /// used by the `vex_*` mouse-vector calls, which pass a single native
    /// EL1 routine pointer and are only reachable from LP64 callers. The
    /// raster ops instead use [`Vdipb::contrl_ptr`], which honours the
    /// caller's ABI (`N_PTRINTS`) so an ILP32 MFDB pointer is read at the
    /// right offset and width.
    #[inline]
    pub fn contrl_u64(&self, i: usize) -> u64 {
        // SAFETY: callers ensure CONTRL has ≥ i+4 WORDs.
        unsafe { (self.contrl.add(i) as *const u64).read_unaligned() }
    }

    /// The calling process's pointer-width ABI (ADR-020). Used by the
    /// raster ops to read the MFDB struct fields at the caller's width.
    #[inline]
    pub fn abi(&self) -> Abi {
        self.abi
    }

    /// Read the source (`n = 0`) / destination (`n = 1`) MFDB pointer from
    /// CONTRL at the **caller's** pointer width. gemlib packs VDI pointer
    /// `n` at CONTRL word `7 + n * N_PTRINTS`, where `N_PTRINTS =
    /// sizeof(void*) / sizeof(short)` = 4 for LP64 / 2 for ILP32
    /// (`sdk/gemlib/gemlib-0.44.0/gem_vdiP.h:177,204`); the pointer itself
    /// is an 8-byte load (LP64) or a 4-byte zero-extended load (ILP32).
    /// Unlike [`Vdipb::contrl_u64`], this honours the ILP32 packing
    /// instead of assuming the LP64 layout, so an ILP32 app (QED, cflib)
    /// no longer mis-reads its MFDB pointers.
    #[inline]
    pub fn contrl_ptr(&self, n: usize) -> u64 {
        let nptrints = match self.abi {
            Abi::Lp64 => 4,
            Abi::Ilp32 => 2,
        };
        // SAFETY: a raster op's CONTRL holds ≥ 7 + (n+1)*N_PTRINTS WORDs;
        // the packed pointer is WORD- but not pointer-aligned, so read it
        // unaligned at the caller's width.
        let base = unsafe { self.contrl.add(7 + n * nptrints) } as *const u8;
        match self.abi {
            Abi::Lp64 => unsafe { (base as *const u64).read_unaligned() },
            Abi::Ilp32 => unsafe { (base as *const u32).read_unaligned() as u64 },
        }
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

/// VDI dispatch entry. `vdipb_ptr` is the `x1` value at the `svc #2`
/// trap (selector 115 in `x0`); `abi` is the calling process's pointer
/// width (the kernel passes [`crate::bdos::process::current_abi`]). The
/// VDI returns nothing in a register — all results go into the
/// parameter-block arrays — so this returns `()`.
///
/// # Safety
/// `vdipb_ptr` must be a valid VDI parameter block of `abi`'s pointer
/// width (see [`Vdipb`]).
pub unsafe fn dispatch(vdipb_ptr: u64, abi: Abi) {
    // SAFETY: caller guarantees a valid parameter block of `abi`'s width.
    let pb = unsafe { Vdipb::from_raw(vdipb_ptr, abi) };

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

    // ── workstation open (no handle on input; must NOT reset multifill) ──
    if opcode == 1 {
        workstation::v_opnwk(&pb, backend);
        return;
    }
    if opcode == 100 {
        workstation::v_opnvwk(&pb, backend);
        return;
    }

    // Every other op needs a valid, open workstation (EmuTOS
    // `vdi_main.c:139-148`). Resolve it and, unless the user fill style is
    // active, reset the per-op `multifill` flag (AUD-V1-009). This borrow
    // is scoped so the mutable handle-table borrow is released before a
    // close op (`v_clswk`/`v_clsvwk`) re-borrows the table.
    let handle = pb.contrl(C_HANDLE);
    // SAFETY: single trap path.
    match unsafe { vwk::resolve(handle) } {
        Some(vwk) => {
            if vwk.fill_style != 4 {
                vwk.multifill = false;
            }
        }
        None => return, // no such open workstation → VDI no-op
    }

    // Point the backend's PEL resolution at THIS workstation's private
    // pseudo-palette for the op about to run: under Falcon truecolor every
    // primitive keeps a plain PEL and resolves it at the write point through
    // `CUR_WORK->ext->palette` (EmuTOS `swblit_rect_common16`,
    // `vdi_line.c:459-460`), so the active palette must track the drawing
    // workstation. Unless `vs_color` changed it, this equals the device
    // default, so `read_pixel`/inquiry stay consistent too.
    // SAFETY: single trap path; `handle` resolved to an open ws above.
    if let Some(ext) = unsafe { vwk::ext(handle) } {
        backend.set_active_palette(&ext.palette);
    }

    match opcode {
        // ── workstation close (dispatched by handle, not the vwk ref) ──
        // v_clswk (2) closes ALL virtual workstations and leaves the
        // physical one open (AUD-V1-001); v_clsvwk (101) closes a single
        // virtual workstation, with a physical-handle guard (AUD-V1-002).
        2 => workstation::v_clswk(),
        101 => workstation::v_clsvwk(handle),
        3 => workstation::v_clrwk(backend),

        // ── attribute setters + output primitives + inquiry (need the vwk) ──
        _ => {
            // SAFETY: single trap path; `handle` was validated above.
            if let Some(vwk) = unsafe { vwk::resolve(handle) } {
                let ncol = backend.num_colors();
                match opcode {
                    102 => workstation::vq_extnd(&pb, vwk, backend),
                    14 => attributes::vs_color(&pb, vwk, ncol, backend),
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
                    112 => attributes::vsf_udpat(&pb, vwk, ncol),
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
                    26 => inquiry::vq_color(&pb, vwk, ncol, backend),
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
                    110 => raster::vr_trnfm(&pb, backend),
                    // ── genuine VDI no-ops + not-yet-implemented opcodes ──
                    // (v_updwk #4, v_cellarray #10, vq_cellarray #27,
                    // v_valuator #29, #34, and any out-of-range opcode). By
                    // VDI semantics an unrouted opcode is a no-op — EmuTOS
                    // maps these to `vdi_v_nop` / returns without drawing.
                    _ => {}
                }
            }
        }
    }
}
