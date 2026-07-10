//! Device-wide `REQ_COL` palette-request shadow.
//!
//! Spec: [`docs/api/vdi/attributes/_mode-clip-color.md`] (`vs_color` raw
//! store) + [`docs/api/vdi/inquiry/_color-pixel-input.md`] (`vq_color`
//! flag-0 raw read-back); model in [`docs/api/vdi/_state.md`] "The
//! colour-request palette". EmuTOS `vdi/vdi_col.c` `REQ_COL[16][3]`.
//!
//! `vs_color` stores the **raw, unclamped** `INTIN[1..3]` here; only the
//! copy programmed into the CLUT is clamped to the VDI 0..1000 range. So
//! `vq_color(idx, flag=0)` round-trips the exact request — possibly
//! outside 0..1000 — exactly like EmuTOS (`vdi_col.c:690` stores raw;
//! `:821` returns `REQ_COL` for flag 0). This is a **device-global**
//! array (Line-A), not per-`Vwk`, matching the reference.
//!
//! Registers a program never wrote via `vs_color` fall back to the live
//! backend palette (the hardware default the workstation opened with),
//! which is what EmuTOS' init-time `REQ_COL` seeding (`init_colors`)
//! holds; NewTOS seeds the backend CLUT instead, so the observable
//! flag-0 read-back matches without duplicating the seed table here.

use core::cell::UnsafeCell;

/// Number of shadowed palette registers. 16 on the default (non-
/// `EXTENDED_PALETTE`) build; entries 16.. would live in `req_col2`,
/// which is out of NewTOS bring-up scope (palette-only, `MAXCOLOURS=16`).
const REQ_COL_COUNT: usize = 16;

struct ReqColCell {
    /// Raw R/G/B per register, unclamped (may lie outside 0..1000).
    rgb: UnsafeCell<[[i16; 3]; REQ_COL_COUNT]>,
    /// Bit `i` set → register `i` was written by `vs_color`.
    set: UnsafeCell<u16>,
}

// SAFETY: touched only from the single-CPU, IRQ-off VDI trap path — the
// same discipline as the `Vwk` handle table and the mouse state.
unsafe impl Sync for ReqColCell {}

static REQ_COL: ReqColCell = ReqColCell {
    rgb: UnsafeCell::new([[0; 3]; REQ_COL_COUNT]),
    set: UnsafeCell::new(0),
};

/// Store the raw (unclamped) request for register `idx` (EmuTOS
/// `REQ_COL[colnum][i] = *intin`, `vdi_col.c:690`). Indices ≥ 16 are
/// ignored (they belong to `req_col2`, out of scope).
///
/// # Safety
/// Single trap path (no re-entrancy), as for the `Vwk` table.
pub(crate) unsafe fn store_request(idx: u16, r: i16, g: i16, b: i16) {
    if idx as usize >= REQ_COL_COUNT {
        return;
    }
    // SAFETY: see `ReqColCell`'s Sync impl.
    unsafe {
        (*REQ_COL.rgb.get())[idx as usize] = [r, g, b];
        *REQ_COL.set.get() |= 1 << idx;
    }
}

/// The raw last-requested RGB for register `idx` if `vs_color` set it,
/// else `None` (the caller falls back to the live hardware palette).
///
/// # Safety
/// Single trap path (no re-entrancy), as for the `Vwk` table.
pub(crate) unsafe fn requested(idx: u16) -> Option<(i16, i16, i16)> {
    if idx as usize >= REQ_COL_COUNT {
        return None;
    }
    // SAFETY: see `ReqColCell`'s Sync impl.
    unsafe {
        if *REQ_COL.set.get() & (1 << idx) == 0 {
            return None;
        }
        let e = (*REQ_COL.rgb.get())[idx as usize];
        Some((e[0], e[1], e[2]))
    }
}
