//! Virtual workstation (`Vwk`) attribute state + the handle table.
//!
//! Spec: [`docs/api/structures/vwk.md`] and
//! [`docs/api/vdi/workstation/100-v_opnvwk.md`]. A `Vwk` holds the
//! per-handle attribute state a workstation draws with. `v_opnvwk`
//! allocates one, `init_wk` fills it from the caller's `work_in`, and
//! the dispatcher resolves `contrl[6]` to it.
//!
//! NewTOS keeps the 16-handle cap but, for the bring-up, backs the
//! handles with a **static slot table** rather than `Mxalloc` (the
//! `Vwk` layout is internal — not an ABI promise — so this is free to
//! choose; a heap-allocated `Vwk` can come later if needed).

use core::cell::UnsafeCell;

use super::backend::{ClipRect, Pel};

/// Total VDI handles. Handle 1 is the physical workstation; 2..=16 are
/// virtual. Index 0 is unused (handle 0 = "no/failed handle").
pub const HANDLE_COUNT: usize = 16;

/// The physical-workstation handle (`v_opnwk`).
pub const PHYS_HANDLE: i16 = 1;

// Attribute defaults (EmuTOS `vdi_defs.h`), expressed in our 0-based
// internal form where noted.
const DEF_LINE_INDEX: i16 = 0; // solid (style 1, stored 0-based)
const DEF_LINE_WIDTH: i16 = 1;
const SQUARED: i16 = 0; // line end style
const DEF_MARK_INDEX: i16 = 2; // marker type 3 (asterisk), 0-based
const DEF_MARK_HEIGHT: i16 = 11;
const DEF_FILL_STYLE: i16 = 0; // FIS_HOLLOW
const WM_REPLACE: i16 = 0; // writing mode (0-based: replace)
const DEF_PEN: Pel = 1; // default drawing colour (black) when out of range

/// Coordinate transform mode (`work_in[10]`): NDC vs raster.
pub const XFM_RC: i16 = 2;

/// Per-handle attribute area. The field layout is internal (see module
/// docs); the attribute groups mirror `docs/api/structures/vwk.md`.
///
/// `init_wk` populates the whole area; the workstation-lifecycle
/// milestone (D1/D2) reads `handle`/`in_use` and the geometry, while the
/// drawing/inquiry opcodes (D3+) consume the attribute groups. Hence the
/// allow: the fields are genuinely written now and read as those opcodes
/// land.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct Vwk {
    /// The handle this attribute area belongs to (`0` = free slot).
    pub handle: i16,
    /// Slot occupancy.
    pub in_use: bool,

    // Line attributes.
    pub line_index: i16,
    pub line_color: Pel,
    pub line_width: i16,
    pub line_beg: i16,
    pub line_end: i16,
    pub ud_ls: u16,

    // Marker attributes.
    pub mark_index: i16,
    pub mark_color: Pel,
    pub mark_height: i16,
    pub mark_scale: i16,

    // Fill attributes.
    pub fill_style: i16,
    pub fill_index: i16,
    pub fill_color: Pel,
    pub fill_per: bool,
    pub multifill: bool,
    /// User-defined fill pattern (16 rows), used when `fill_style` is
    /// `FIS_USER`; set by `vsf_udpat`.
    pub ud_patrn: [u16; 16],

    // Text attributes.
    pub text_color: Pel,
    pub style: i16,
    pub h_align: i16,
    pub v_align: i16,
    pub chup: i16,

    // Mode + clipping.
    pub wrt_mode: i16,
    pub xfm_mode: i16,
    pub clip: bool,
    pub xmn_clip: i16,
    pub ymn_clip: i16,
    pub xmx_clip: i16,
    pub ymx_clip: i16,
}

impl Vwk {
    /// The workstation's clip rectangle as an inclusive [`ClipRect`]
    /// (always valid: `init_wk` sets it to the full screen, `vs_clip`
    /// updates it). Every drawing primitive passes this to the backend.
    pub fn clip_rect(&self) -> ClipRect {
        ClipRect {
            xmn: self.xmn_clip,
            ymn: self.ymn_clip,
            xmx: self.xmx_clip,
            ymx: self.ymx_clip,
        }
    }

    /// An empty (free) slot.
    pub const EMPTY: Vwk = Vwk {
        handle: 0,
        in_use: false,
        line_index: DEF_LINE_INDEX,
        line_color: DEF_PEN,
        line_width: DEF_LINE_WIDTH,
        line_beg: SQUARED,
        line_end: SQUARED,
        ud_ls: 0xFFFF,
        mark_index: DEF_MARK_INDEX,
        mark_color: DEF_PEN,
        mark_height: DEF_MARK_HEIGHT,
        mark_scale: 1,
        fill_style: DEF_FILL_STYLE,
        fill_index: 0,
        fill_color: DEF_PEN,
        fill_per: true,
        multifill: false,
        ud_patrn: [0; 16],
        text_color: DEF_PEN,
        style: 0,
        h_align: 0,
        v_align: 0,
        chup: 0,
        wrt_mode: WM_REPLACE,
        xfm_mode: XFM_RC,
        clip: false,
        xmn_clip: 0,
        ymn_clip: 0,
        xmx_clip: 0,
        ymx_clip: 0,
    };
}

/// Clamp a requested colour index to the device's palette, falling back
/// to the default pen when out of range (EmuTOS `validate_color_index`).
pub fn validate_color_index(c: i16, num_colors: u16) -> Pel {
    if c >= 0 && (c as u16) < num_colors {
        c as Pel
    } else {
        DEF_PEN
    }
}

/// Static handle table. Mirrors the `MdArenaCell` idiom: an
/// `UnsafeCell` array accessed only from the single-CPU, IRQ-off trap
/// context (serialised like the rest of the TOS-API surface).
struct VwkTableCell(UnsafeCell<[Vwk; HANDLE_COUNT]>);

// SAFETY: accessed only from the synchronous trap handler on the boot
// CPU with interrupts masked; never concurrently. Same discipline as
// the GEMDOS/BDOS mutable statics.
unsafe impl Sync for VwkTableCell {}

static TABLE: VwkTableCell = VwkTableCell(UnsafeCell::new([Vwk::EMPTY; HANDLE_COUNT]));

/// Borrow the handle table mutably.
///
/// # Safety
/// Caller must be on the single trap path (no re-entrancy).
#[allow(clippy::mut_from_ref)]
unsafe fn table() -> &'static mut [Vwk; HANDLE_COUNT] {
    // SAFETY: see `VwkTableCell`'s Sync impl.
    unsafe { &mut *TABLE.0.get() }
}

/// Allocate a free virtual-workstation handle (2..=16) and return a
/// mutable reference to its freshly-reset slot, or `None` if all are in
/// use. The slot's `handle`/`in_use` are set; the caller runs `init_wk`.
///
/// # Safety
/// Single trap path (see [`table`]).
pub unsafe fn alloc_handle() -> Option<&'static mut Vwk> {
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    for (i, slot) in t.iter_mut().enumerate() {
        // Handle 1 (index 0) is reserved for the physical workstation.
        if i == 0 {
            continue;
        }
        if !slot.in_use {
            *slot = Vwk::EMPTY;
            slot.in_use = true;
            slot.handle = (i + 1) as i16;
            return Some(slot);
        }
    }
    None
}

/// Open (or re-open) the physical workstation slot (handle 1) and
/// return a mutable reference to its freshly-reset slot. The caller runs
/// `init_wk`.
///
/// # Safety
/// Single trap path (see [`table`]).
pub unsafe fn open_physical() -> &'static mut Vwk {
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    t[0] = Vwk::EMPTY;
    t[0].in_use = true;
    t[0].handle = PHYS_HANDLE;
    &mut t[0]
}

/// Resolve a handle to its slot, or `None` if the handle is invalid or
/// not open.
///
/// # Safety
/// Single trap path (see [`table`]).
pub unsafe fn resolve(handle: i16) -> Option<&'static mut Vwk> {
    if handle < 1 || handle as usize > HANDLE_COUNT {
        return None;
    }
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    let slot = &mut t[(handle - 1) as usize];
    if slot.in_use {
        Some(slot)
    } else {
        None
    }
}

/// Release a handle's slot.
///
/// # Safety
/// Single trap path (see [`table`]).
pub unsafe fn free_handle(handle: i16) {
    if handle < 1 || handle as usize > HANDLE_COUNT {
        return;
    }
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    t[(handle - 1) as usize] = Vwk::EMPTY;
}
