//! Software VDI rasterizer backend (Phase 12).
//!
//! Implements [`tos_api::vdi::backend::RasterBackend`] over the board's
//! linear [`Framebuffer`]. This is the **bring-up** backend: every
//! primitive is plain scalar CPU code. It is the swappable boundary the
//! VDI layer draws through — a later NEON / DMA / VideoCore-V3D backend
//! replaces this impl without the VDI layer changing (see the
//! `RasterBackend` docs).
//!
//! Colours arrive as PEL (palette index); we resolve them through a
//! fixed 16-entry GEM palette into the truecolor framebuffer. VDI
//! convention: index 0 = white (background), 1 = black (foreground).

use tos_api::vdi::backend::{ClipRect, Pel, RasterBackend};

use crate::hal::framebuffer::{rgb, Framebuffer};

/// Number of bring-up palette entries (advertised as `DEV_TAB[13]`).
const NUM_COLORS: u16 = 16;

/// The standard GEM 16-colour palette (PEL → packed XRGB). Index 0 is
/// white and 1 is black, matching VDI's background/foreground default.
const PALETTE: [u32; NUM_COLORS as usize] = [
    rgb(255, 255, 255), // 0  white (background)
    rgb(0, 0, 0),       // 1  black (foreground)
    rgb(255, 0, 0),     // 2  red
    rgb(0, 255, 0),     // 3  green
    rgb(0, 0, 255),     // 4  blue
    rgb(0, 255, 255),   // 5  cyan
    rgb(255, 255, 0),   // 6  yellow
    rgb(255, 0, 255),   // 7  magenta
    rgb(192, 192, 192), // 8  light grey
    rgb(128, 128, 128), // 9  dark grey
    rgb(128, 0, 0),     // 10 dark red
    rgb(0, 128, 0),     // 11 dark green
    rgb(0, 0, 128),     // 12 dark blue
    rgb(0, 128, 128),   // 13 dark cyan
    rgb(128, 128, 0),   // 14 dark yellow
    rgb(128, 0, 128),   // 15 dark magenta
];

/// Resolve the active board framebuffer (exactly one per board).
#[cfg(feature = "board-qemu-virt")]
fn active_fb() -> &'static dyn Framebuffer {
    &crate::drivers::qemu_virt::ramfb::FRAMEBUFFER
}
#[cfg(feature = "board-rpi4")]
fn active_fb() -> &'static dyn Framebuffer {
    &crate::drivers::bcm2711::framebuffer::FRAMEBUFFER
}

/// The screen VDI backend (zero-sized; state lives in the framebuffer).
pub struct VdiScreen;

impl VdiScreen {
    #[inline]
    fn pel_to_rgb(pel: Pel) -> u32 {
        PALETTE[(pel as usize) & (NUM_COLORS as usize - 1)]
    }
}

impl RasterBackend for VdiScreen {
    fn width(&self) -> u16 {
        active_fb().width() as u16
    }

    fn height(&self) -> u16 {
        active_fb().height() as u16
    }

    fn num_colors(&self) -> u16 {
        NUM_COLORS
    }

    fn fill_rect(&self, x: i16, y: i16, w: i16, h: i16, pel: Pel) {
        // Clamp the top-left into the visible area, shrinking the extent
        // accordingly; the framebuffer clips the far edges itself.
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let cut_x = (x0 as i32 - x as i32) as i16; // pixels trimmed on the left
        let cut_y = (y0 as i32 - y as i32) as i16;
        let w = (w - cut_x).max(0) as u32;
        let h = (h - cut_y).max(0) as u32;
        if w == 0 || h == 0 {
            return;
        }
        active_fb().fill_rect(x0, y0, w, h, Self::pel_to_rgb(pel));
    }

    fn draw_line(&self, x0: i16, y0: i16, x1: i16, y1: i16, pel: Pel, clip: ClipRect) {
        let fb = active_fb();
        let color = Self::pel_to_rgb(pel);
        // Bresenham; plot only pixels inside the inclusive clip rect (and
        // the framebuffer, which `put_pixel` enforces too).
        let (mut x, mut y) = (x0 as i32, y0 as i32);
        let (xe, ye) = (x1 as i32, y1 as i32);
        let dx = (xe - x).abs();
        let dy = -(ye - y).abs();
        let sx = if x < xe { 1 } else { -1 };
        let sy = if y < ye { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x >= clip.xmn as i32
                && x <= clip.xmx as i32
                && y >= clip.ymn as i32
                && y <= clip.ymx as i32
            {
                fb.put_pixel(x as u32, y as u32, color);
            }
            if x == xe && y == ye {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
}

/// The installed screen backend, handed to `tos-api` via the host.
pub static VDI_SCREEN: VdiScreen = VdiScreen;

/// Whether a usable framebuffer is up (geometry published). The host
/// returns the backend only when this is true.
pub fn available() -> bool {
    active_fb().width() != 0 && active_fb().height() != 0
}

/// Screen dimensions in pixels (for the VDI self-test).
pub fn dims() -> (u16, u16) {
    (active_fb().width() as u16, active_fb().height() as u16)
}
