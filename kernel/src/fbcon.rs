//! VT52-style framebuffer text console (Phase 10 D5).
//!
//! Renders GEMDOS console output (`Cconout` / `Cconws` …) to the
//! framebuffer through the VDI raster backend's 8×16 system font, so
//! program output is visible on HDMI without a serial console. The kernel
//! [`Host::console_write_byte`](crate::host_env) forwards each byte here
//! (in addition to the UART) once the console is [`activate`]d.
//!
//! Scope: printable glyphs, `CR`/`LF`/`BS`/`TAB`, line wrap and upward
//! scroll. Full VT52 escape sequences (cursor addressing, clear-to-EOL)
//! can layer on later.

use core::sync::atomic::{AtomicBool, AtomicI16, Ordering};

use tos_api::vdi::backend::{ClipRect, RasterBackend, Rect, WriteMode};
use tos_api::vdi::font;

use crate::vdi_backend::{self, VDI_SCREEN};

/// The console draws to the whole screen, so every primitive uses the
/// full-device clip rectangle.
fn full_clip() -> ClipRect {
    let (w, h) = vdi_backend::dims();
    ClipRect::full(w, h)
}

const CELL_W: i16 = 8;
const CELL_H: i16 = 16;
/// VDI palette indices (see `vdi_backend::PALETTE`): black text on a
/// white background, matching the Atari TOS desktop.
const FG: u8 = 1;
const BG: u8 = 0;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static COL: AtomicI16 = AtomicI16::new(0);
static ROW: AtomicI16 = AtomicI16::new(0);

fn cols() -> i16 {
    (vdi_backend::dims().0 as i16) / CELL_W
}
fn rows() -> i16 {
    (vdi_backend::dims().1 as i16) / CELL_H
}

/// Clear the screen and start directing console output to it.
pub fn activate() {
    if !vdi_backend::available() {
        return;
    }
    let (w, h) = vdi_backend::dims();
    VDI_SCREEN.fill_rect(0, 0, w as i16, h as i16, BG, full_clip());
    COL.store(0, Ordering::Relaxed);
    ROW.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
}

/// Whether the framebuffer console is receiving output.
pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Write one byte to the console. No-op until [`activate`]d.
pub fn write_byte(b: u8) {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    match b {
        b'\r' => COL.store(0, Ordering::Relaxed),
        b'\n' => newline(),
        0x08 => {
            // Backspace: step back and erase the cell.
            let c = COL.load(Ordering::Relaxed);
            if c > 0 {
                COL.store(c - 1, Ordering::Relaxed);
                cell_clear(c - 1, ROW.load(Ordering::Relaxed));
            }
        }
        0x09 => {
            // Tab to the next multiple of 8 columns.
            let n = ((COL.load(Ordering::Relaxed) & !7) + 8).min(cols());
            COL.store(n, Ordering::Relaxed);
            if n >= cols() {
                newline();
            }
        }
        0x20..=0xFF => put_glyph(b),
        _ => {} // ignore other control characters
    }
}

fn newline() {
    COL.store(0, Ordering::Relaxed);
    let r = ROW.load(Ordering::Relaxed) + 1;
    if r >= rows() {
        scroll();
        ROW.store(rows() - 1, Ordering::Relaxed);
    } else {
        ROW.store(r, Ordering::Relaxed);
    }
}

fn put_glyph(b: u8) {
    let (col, row) = (COL.load(Ordering::Relaxed), ROW.load(Ordering::Relaxed));
    let (x, y) = (col * CELL_W, row * CELL_H);
    let clip = full_clip();
    VDI_SCREEN.fill_rect(x, y, CELL_W, CELL_H, BG, clip);
    // Cell background is cleared above, so the glyph blits transparently.
    VDI_SCREEN.draw_glyph(x, y, font::glyph(b as u16), FG, WriteMode::Transparent, clip);
    let nc = col + 1;
    if nc >= cols() {
        newline();
    } else {
        COL.store(nc, Ordering::Relaxed);
    }
}

fn cell_clear(col: i16, row: i16) {
    VDI_SCREEN.fill_rect(col * CELL_W, row * CELL_H, CELL_W, CELL_H, BG, full_clip());
}

fn scroll() {
    let (w, h) = vdi_backend::dims();
    let (w, h) = (w as i16, h as i16);
    let clip = full_clip();
    // Move everything below the top line up by one cell, clear the gap.
    VDI_SCREEN.copy_rect(Rect { x: 0, y: CELL_H, w, h: h - CELL_H }, 0, 0, 0, clip);
    VDI_SCREEN.fill_rect(0, h - CELL_H, w, CELL_H, BG, clip);
}
