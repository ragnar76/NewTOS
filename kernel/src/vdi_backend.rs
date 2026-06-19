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

use core::sync::atomic::{AtomicU32, Ordering};

use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect};

use crate::hal::framebuffer::{rgb, Framebuffer};

/// Number of bring-up palette entries (advertised as `DEV_TAB[13]`).
const NUM_COLORS: u16 = 16;

/// The standard GEM 16-colour palette as 8-bit `(r, g, b)` triples.
/// Index 0 is white and 1 is black, matching VDI's background/foreground
/// default. `vs_color` overwrites entries at runtime (see [`PALETTE`]).
const DEFAULT_PALETTE: [(u8, u8, u8); NUM_COLORS as usize] = [
    (255, 255, 255), // 0  white (background)
    (0, 0, 0),       // 1  black (foreground)
    (255, 0, 0),     // 2  red
    (0, 255, 0),     // 3  green
    (0, 0, 255),     // 4  blue
    (0, 255, 255),   // 5  cyan
    (255, 255, 0),   // 6  yellow
    (255, 0, 255),   // 7  magenta
    (192, 192, 192), // 8  light grey
    (128, 128, 128), // 9  dark grey
    (128, 0, 0),     // 10 dark red
    (0, 128, 0),     // 11 dark green
    (0, 0, 128),     // 12 dark blue
    (0, 128, 128),   // 13 dark cyan
    (128, 128, 0),   // 14 dark yellow
    (128, 0, 128),   // 15 dark magenta
];

/// The live palette: PEL → packed XRGB, mutated by `vs_color`. Lazily
/// initialised from [`DEFAULT_PALETTE`] on first read (entry 0 carrying
/// the init sentinel in its high byte). Accessed only from the single
/// trap path, but `AtomicU32` keeps it `Sync` without `unsafe`.
static PALETTE: [AtomicU32; NUM_COLORS as usize] =
    [const { AtomicU32::new(PAL_UNINIT) }; NUM_COLORS as usize];

/// The requested RGB (0..=1000) per entry, packed `r<<20 | g<<10 | b`,
/// returned by `vq_color`. Lazily initialised alongside [`PALETTE`].
static REQ_COL: [AtomicU32; NUM_COLORS as usize] =
    [const { AtomicU32::new(0) }; NUM_COLORS as usize];

/// Sentinel marking an uninitialised [`PALETTE`] slot (alpha byte set,
/// which a real XRGB colour never carries).
const PAL_UNINIT: u32 = 0xFF00_0000;

/// Scale an 8-bit channel (0..=255) to the VDI 0..=1000 range.
const fn to_vdi(c: u8) -> u16 {
    ((c as u32 * 1000 + 127) / 255) as u16
}

/// Scale a VDI channel (0..=1000) to 8-bit (0..=255).
fn to_rgb8(c: u16) -> u8 {
    ((c.min(1000) as u32 * 255 + 500) / 1000) as u8
}

/// Initialise the palette from the GEM defaults on first use.
fn ensure_palette_init() {
    if PALETTE[0].load(Ordering::Relaxed) != PAL_UNINIT {
        return;
    }
    for (i, &(r, g, b)) in DEFAULT_PALETTE.iter().enumerate() {
        PALETTE[i].store(rgb(r, g, b), Ordering::Relaxed);
        let req = (to_vdi(r) as u32) << 20 | (to_vdi(g) as u32) << 10 | to_vdi(b) as u32;
        REQ_COL[i].store(req, Ordering::Relaxed);
    }
}

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
        ensure_palette_init();
        PALETTE[(pel as usize) & (NUM_COLORS as usize - 1)].load(Ordering::Relaxed)
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

    fn set_color(&self, index: u16, r: u16, g: u16, b: u16) {
        if index >= NUM_COLORS {
            return;
        }
        ensure_palette_init();
        let (r, g, b) = (r.min(1000), g.min(1000), b.min(1000));
        let req = (r as u32) << 20 | (g as u32) << 10 | b as u32;
        REQ_COL[index as usize].store(req, Ordering::Relaxed);
        PALETTE[index as usize].store(rgb(to_rgb8(r), to_rgb8(g), to_rgb8(b)), Ordering::Relaxed);
    }

    fn query_color(&self, index: u16) -> (u16, u16, u16) {
        if index >= NUM_COLORS {
            return (0, 0, 0);
        }
        ensure_palette_init();
        let req = REQ_COL[index as usize].load(Ordering::Relaxed);
        (((req >> 20) & 0x3FF) as u16, ((req >> 10) & 0x3FF) as u16, (req & 0x3FF) as u16)
    }

    fn fill_rect(&self, x: i16, y: i16, w: i16, h: i16, pel: Pel, clip: ClipRect) {
        let fb = active_fb();
        // Intersect the requested rectangle, the clip rectangle and the
        // screen into one inclusive device rectangle.
        let rect = ClipRect {
            xmn: x.max(clip.xmn),
            ymn: y.max(clip.ymn),
            xmx: ((x as i32 + w as i32 - 1) as i16).min(clip.xmx),
            ymx: ((y as i32 + h as i32 - 1) as i16).min(clip.ymx),
        };
        let Some((x0, y0, x1, y1)) = rect.clamp_to(fb.width() as u16, fb.height() as u16) else {
            return;
        };
        fb.fill_rect(x0 as u32, y0 as u32, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32, Self::pel_to_rgb(pel));
        Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
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

    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill, clip: ClipRect) {
        if fill.pattern.is_empty() {
            return;
        }
        let fb = active_fb();
        // Intersect the requested rectangle, the clip rectangle and the
        // screen into one inclusive device rectangle.
        let rect = ClipRect {
            xmn: x.max(clip.xmn),
            ymn: y.max(clip.ymn),
            xmx: ((x as i32 + w as i32 - 1) as i16).min(clip.xmx),
            ymx: ((y as i32 + h as i32 - 1) as i16).min(clip.ymx),
        };
        let Some((x0, y0, x1, y1)) = rect.clamp_to(fb.width() as u16, fb.height() as u16) else {
            return;
        };
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

    fn draw_glyph(&self, x: i16, y: i16, glyph: &[u8; 16], fg: Pel, clip: ClipRect) {
        let fb = active_fb();
        // The 8×16 cell intersected with the clip rectangle and the
        // screen — only pixels inside this inclusive box are touched.
        let cell = ClipRect {
            xmn: x.max(clip.xmn),
            ymn: y.max(clip.ymn),
            xmx: ((x as i32 + 7) as i16).min(clip.xmx),
            ymx: ((y as i32 + 15) as i16).min(clip.ymx),
        };
        let Some((x0, y0, x1, y1)) = cell.clamp_to(fb.width() as u16, fb.height() as u16) else {
            return;
        };
        let fg_rgb = Self::pel_to_rgb(fg);
        for yy in y0..=y1 {
            let bits = glyph[(yy - y as i32) as usize];
            for xx in x0..=x1 {
                let col = xx - x as i32; // 0..=7 within the cell
                if bits & (0x80u8 >> col) != 0 {
                    fb.put_pixel(xx as u32, yy as u32, fg_rgb);
                }
            }
        }
        Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    }

    fn blit_mono(&self, dst: Rect, src: MonoSrc, fg: Pel, bg: Pel, opaque: bool, clip: ClipRect) {
        let fb = active_fb();
        // Destination box ∩ clip ∩ screen.
        let box_ = ClipRect {
            xmn: dst.x.max(clip.xmn),
            ymn: dst.y.max(clip.ymn),
            xmx: ((dst.x as i32 + dst.w as i32 - 1) as i16).min(clip.xmx),
            ymx: ((dst.y as i32 + dst.h as i32 - 1) as i16).min(clip.ymx),
        };
        let Some((x0, y0, x1, y1)) = box_.clamp_to(fb.width() as u16, fb.height() as u16) else {
            return;
        };
        let fg_rgb = Self::pel_to_rgb(fg);
        let bg_rgb = Self::pel_to_rgb(bg);
        let stride = src.stride_words as usize;
        for dyc in y0..=y1 {
            let sy = src.y as i32 + (dyc - dst.y as i32);
            for dxc in x0..=x1 {
                let sx = src.x as i32 + (dxc - dst.x as i32);
                let widx = sy as usize * stride + (sx as usize) / 16;
                let set = src
                    .bits
                    .get(widx)
                    .is_some_and(|w| w & (0x8000u16 >> (sx & 15)) != 0);
                if set {
                    fb.put_pixel(dxc as u32, dyc as u32, fg_rgb);
                } else if opaque {
                    fb.put_pixel(dxc as u32, dyc as u32, bg_rgb);
                }
            }
        }
        Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    }

    fn copy_rect(&self, src: Rect, dx: i16, dy: i16, op: u8, clip: ClipRect) {
        let fb = active_fb();
        let scr_w = fb.width() as i32;
        let scr_h = fb.height() as i32;
        let stride = fb.stride_px() as usize;
        let base = fb.pixels();
        let (w, h) = (src.w as i32, src.h as i32);
        // The destination write is clipped; the source offset follows it.
        let dbox = ClipRect {
            xmn: dx.max(clip.xmn),
            ymn: dy.max(clip.ymn),
            xmx: ((dx as i32 + w - 1) as i16).min(clip.xmx),
            ymx: ((dy as i32 + h - 1) as i16).min(clip.ymx),
        };
        let Some((tx0, ty0, tx1, ty1)) = dbox.clamp_to(scr_w as u16, scr_h as u16) else {
            return;
        };
        // Choose copy direction so overlapping moves don't corrupt.
        let down = dy as i32 > src.y as i32;
        let right = dx as i32 > src.x as i32;
        for ri in 0..=(ty1 - ty0) {
            let ty = if down { ty1 - ri } else { ty0 + ri };
            let sy = src.y as i32 + (ty - dy as i32);
            if sy < 0 || sy >= scr_h {
                continue;
            }
            for ci in 0..=(tx1 - tx0) {
                let tx = if right { tx1 - ci } else { tx0 + ci };
                let sx = src.x as i32 + (tx - dx as i32);
                if sx < 0 || sx >= scr_w {
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
        Self::clean(fb, tx0 as u32, ty0 as u32, tx1 as u32, ty1 as u32);
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
