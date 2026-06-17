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
    /// Input coordinate array. Part of the VDIPB contract; read by the
    /// output primitives (D3+, `v_pline`/`v_fillarea`/…), not by the
    /// workstation-lifecycle milestone.
    #[allow(dead_code)]
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
    #[inline]
    pub fn set_contrl(&self, i: usize, v: i16) {
        // SAFETY: see `contrl`.
        unsafe { self.contrl.add(i).write(v) }
    }
    #[inline]
    pub fn intin(&self, i: usize) -> i16 {
        // SAFETY: callers index within `contrl[3]` input integers.
        unsafe { self.intin.add(i).read() }
    }
    #[inline]
    #[allow(dead_code)] // consumed by the output primitives (D3+)
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

        // ── genuine VDI no-ops + not-yet-implemented opcodes ──
        // (output/attribute/inquiry/raster opcodes land in later
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
