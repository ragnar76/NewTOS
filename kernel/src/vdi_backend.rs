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

use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect};

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

    /// Clean the cache lines covering the inclusive pixel rectangle
    /// `[x0..=x1] × [y0..=y1]` to the point of coherency, so the display
    /// controller (a separate DMA master beyond the ARM caches) sees the
    /// pixels just written. A no-op if the framebuffer is Non-Cacheable.
    /// We clean the contiguous address span enclosing the rectangle —
    /// cleaning the few row-end bytes in between is harmless.
    fn clean(fb: &dyn Framebuffer, x0: u32, y0: u32, x1: u32, y1: u32) {
        let stride = fb.stride_px() as usize;
        let base = fb.pixels() as usize;
        let start = base + (y0 as usize * stride + x0 as usize) * 4;
        let end = base + (y1 as usize * stride + x1 as usize) * 4 + 4;
        // SAFETY: the span lies within the framebuffer's pixel store.
        unsafe { crate::arch::aarch64::mmu::clean_dcache_range(start, end - start) };
    }

    /// Clean the rectangle `(x, y, w, h)` clamped to the visible area.
    fn clean_clamped(fb: &dyn Framebuffer, x: i16, y: i16, w: i16, h: i16) {
        let x0 = (x as i32).max(0);
        let y0 = (y as i32).max(0);
        let x1 = ((x as i32) + (w as i32) - 1).min(fb.width() as i32 - 1);
        let y1 = ((y as i32) + (h as i32) - 1).min(fb.height() as i32 - 1);
        if x0 <= x1 && y0 <= y1 {
            Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
        }
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
        let fb = active_fb();
        fb.fill_rect(x0, y0, w, h, Self::pel_to_rgb(pel));
        // Clamp the cleaned rectangle to the visible area (fill_rect
        // already clipped the draw).
        let x1 = (x0 + w - 1).min(fb.width().saturating_sub(1));
        let y1 = (y0 + h - 1).min(fb.height().saturating_sub(1));
        Self::clean(fb, x0, y0, x1, y1);
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
        // Clean the line's bounding box (clamped to the screen) to PoC.
        let wmax = fb.width().saturating_sub(1) as i32;
        let hmax = fb.height().saturating_sub(1) as i32;
        let xlo = (x0.min(x1) as i32).clamp(0, wmax) as u32;
        let xhi = (x0.max(x1) as i32).clamp(0, wmax) as u32;
        let ylo = (y0.min(y1) as i32).clamp(0, hmax) as u32;
        let yhi = (y0.max(y1) as i32).clamp(0, hmax) as u32;
        Self::clean(fb, xlo, ylo, xhi, yhi);
    }

    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill) {
        if fill.pattern.is_empty() {
            return;
        }
        let fb = active_fb();
        let scr_w = fb.width() as i32;
        let scr_h = fb.height() as i32;
        // Inclusive device rectangle, clamped to the visible area.
        let x0 = (x as i32).max(0);
        let y0 = (y as i32).max(0);
        let x1 = ((x as i32) + (w as i32) - 1).min(scr_w - 1);
        let y1 = ((y as i32) + (h as i32) - 1).min(scr_h - 1);
        if x0 > x1 || y0 > y1 {
            return;
        }
        let pmask = fill.pattern.len() - 1; // pattern len is a power of two
        let fg_rgb = Self::pel_to_rgb(fill.fg);
        let bg_rgb = Self::pel_to_rgb(fill.bg);
        for yy in y0..=y1 {
            // Pattern rows are screen-aligned to the absolute y coordinate.
            let row = fill.pattern[(yy as usize) & pmask];
            for xx in x0..=x1 {
                if row & (0x8000u16 >> (xx & 0xF)) != 0 {
                    fb.put_pixel(xx as u32, yy as u32, fg_rgb);
                } else if fill.opaque {
                    fb.put_pixel(xx as u32, yy as u32, bg_rgb);
                }
            }
        }
        Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    }

    fn draw_glyph(&self, x: i16, y: i16, glyph: &[u8; 16], fg: Pel) {
        let fb = active_fb();
        let scr_w = fb.width() as i32;
        let scr_h = fb.height() as i32;
        let fg_rgb = Self::pel_to_rgb(fg);
        for (row, &bits) in glyph.iter().enumerate() {
            let yy = y as i32 + row as i32;
            if yy < 0 || yy >= scr_h {
                continue;
            }
            for col in 0..8i32 {
                if bits & (0x80u8 >> col) != 0 {
                    let xx = x as i32 + col;
                    if xx >= 0 && xx < scr_w {
                        fb.put_pixel(xx as u32, yy as u32, fg_rgb);
                    }
                }
            }
        }
        // Clean the 8×16 cell (clamped to the screen) to PoC.
        let x0 = (x as i32).max(0);
        let y0 = (y as i32).max(0);
        let x1 = ((x as i32) + 7).min(scr_w - 1);
        let y1 = ((y as i32) + 15).min(scr_h - 1);
        if x0 <= x1 && y0 <= y1 {
            Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
        }
    }

    fn blit_mono(&self, dst: Rect, src: MonoSrc, fg: Pel, bg: Pel, opaque: bool) {
        let fb = active_fb();
        let scr_w = fb.width() as i32;
        let scr_h = fb.height() as i32;
        let fg_rgb = Self::pel_to_rgb(fg);
        let bg_rgb = Self::pel_to_rgb(bg);
        let stride = src.stride_words as usize;
        for r in 0..dst.h as i32 {
            let dy = dst.y as i32 + r;
            if dy < 0 || dy >= scr_h {
                continue;
            }
            let sy = src.y as i32 + r;
            for c in 0..dst.w as i32 {
                let dx = dst.x as i32 + c;
                if dx < 0 || dx >= scr_w {
                    continue;
                }
                let sx = src.x as i32 + c;
                let widx = sy as usize * stride + (sx as usize) / 16;
                let set = src
                    .bits
                    .get(widx)
                    .is_some_and(|w| w & (0x8000u16 >> (sx & 15)) != 0);
                if set {
                    fb.put_pixel(dx as u32, dy as u32, fg_rgb);
                } else if opaque {
                    fb.put_pixel(dx as u32, dy as u32, bg_rgb);
                }
            }
        }
        Self::clean_clamped(fb, dst.x, dst.y, dst.w, dst.h);
    }

    fn copy_rect(&self, src: Rect, dx: i16, dy: i16, op: u8) {
        let fb = active_fb();
        let scr_w = fb.width() as i32;
        let scr_h = fb.height() as i32;
        let stride = fb.stride_px() as usize;
        let base = fb.pixels();
        let (w, h) = (src.w as i32, src.h as i32);
        // Choose copy direction so overlapping moves don't corrupt.
        let down = dy as i32 > src.y as i32;
        let right = dx as i32 > src.x as i32;
        for ri in 0..h {
            let r = if down { h - 1 - ri } else { ri };
            let sy = src.y as i32 + r;
            let ty = dy as i32 + r;
            if sy < 0 || sy >= scr_h || ty < 0 || ty >= scr_h {
                continue;
            }
            for ci in 0..w {
                let c = if right { w - 1 - ci } else { ci };
                let sx = src.x as i32 + c;
                let tx = dx as i32 + c;
                if sx < 0 || sx >= scr_w || tx < 0 || tx >= scr_w {
                    continue;
                }
                // SAFETY: indices are within the framebuffer bounds.
                unsafe {
                    let sp = base.add(sy as usize * stride + sx as usize).read_volatile();
                    let dp = base.add(ty as usize * stride + tx as usize);
                    let val = if op == 6 { sp ^ dp.read_volatile() } else { sp };
                    dp.write_volatile(val);
                }
            }
        }
        Self::clean_clamped(fb, dx, dy, src.w, src.h);
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

/// CPU-visible framebuffer base address (for the XBIOS screen group).
pub fn base_addr() -> u64 {
    active_fb().pixels() as u64
}
