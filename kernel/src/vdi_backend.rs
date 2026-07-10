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

use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};

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

/// Squared RGB distance between two packed XRGB colours (for the
/// reverse palette lookup in `read_pixel`).
fn rgb_dist(a: u32, b: u32) -> u32 {
    let dr = ((a >> 16) & 0xFF) as i32 - ((b >> 16) & 0xFF) as i32;
    let dg = ((a >> 8) & 0xFF) as i32 - ((b >> 8) & 0xFF) as i32;
    let db = (a & 0xFF) as i32 - (b & 0xFF) as i32;
    (dr * dr + dg * dg + db * db) as u32
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

    /// Combine one candidate pixel into the framebuffer per the VDI
    /// [`WriteMode`], where `set` is the primitive's source bit
    /// (pattern / dash / glyph) at this pixel. This is the single point
    /// that realises the four modes from EmuTOS' truecolor VDI driver
    /// (`swblit_vertical_line16`/`draw_rect_common`, `vdi_textblit.c`).
    /// Bounds are enforced by [`Framebuffer::put_pixel`] and
    /// [`Self::xor_pixel`].
    #[inline]
    fn combine(fb: &dyn Framebuffer, x: u32, y: u32, set: bool, mode: WriteMode, fg: u32, bg: u32) {
        match mode {
            // fg on set bits, bg on clear bits (opaque).
            WriteMode::Replace => fb.put_pixel(x, y, if set { fg } else { bg }),
            // fg on set bits; clear bits untouched.
            WriteMode::Transparent => {
                if set {
                    fb.put_pixel(x, y, fg);
                }
            }
            // Complement the destination on set bits; clear untouched.
            WriteMode::Xor => {
                if set {
                    Self::xor_pixel(fb, x, y);
                }
            }
            // Reverse-transparent: fg on clear bits; set bits untouched.
            WriteMode::RevTransparent => {
                if !set {
                    fb.put_pixel(x, y, fg);
                }
            }
        }
    }

    /// Complement the RGB of one pixel in place (VDI `WM_XOR`).
    ///
    /// **XOR design decision (chunky-framebuffer reimplementation).** Our
    /// device is a chunky 32-bpp XRGB framebuffer with no stored palette
    /// index, so the planar Atari's index-XOR does not apply directly. We
    /// reimplement the *observable contract* of `WM_XOR`: EmuTOS' own
    /// truecolor VDI driver XORs the **stored colour value** — it writes
    /// `*work = ~temp` (`vdi_line.c:494`, `draw_rect_common`) and
    /// `*q = ~*q` (`vdi_textblit.c`, glyph blit), i.e. it complements the
    /// destination and *ignores the pen*. We do exactly that: invert the
    /// low 24 RGB bits (keeping the top X/alpha byte so the pixel stays
    /// opaque).
    ///
    /// This is chosen over a `dest ^= fg_rgb` scheme deliberately:
    /// (1) it is **self-inverse** — two applications restore the original
    /// pixel, the property GEM relies on for rubber-band / drag feedback;
    /// (2) it matches EmuTOS' truecolor driver bit-for-bit on real palette
    /// pixels; and (3) unlike `dest ^ pen`, it is **always visible** for
    /// any pen — a `dest ^ pen` with the common black pen (RGB 0) would be
    /// a no-op and the feedback would vanish.
    #[inline]
    fn xor_pixel(fb: &dyn Framebuffer, x: u32, y: u32) {
        if x >= fb.width() || y >= fb.height() {
            return;
        }
        let off = (y * fb.stride_px() + x) as usize;
        // SAFETY: bounds checked above; one aligned u32 read-modify-write
        // inside the pixel store, on the single IRQ-off VDI trap path.
        unsafe {
            let p = fb.pixels().add(off);
            let v = p.read_volatile();
            p.write_volatile(v ^ 0x00FF_FFFF);
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

    fn read_pixel(&self, x: i16, y: i16) -> Pel {
        let fb = active_fb();
        if x < 0 || y < 0 || x as u32 >= fb.width() || y as u32 >= fb.height() {
            return 0;
        }
        ensure_palette_init();
        // SAFETY: (x,y) is within the framebuffer bounds checked above.
        let packed = unsafe {
            fb.pixels()
                .add(y as usize * fb.stride_px() as usize + x as usize)
                .read_volatile()
        } & 0x00FF_FFFF;
        // Reverse-map the XRGB value to the nearest palette index.
        let mut best = 0u8;
        let mut best_d = u32::MAX;
        for (i, entry) in PALETTE.iter().enumerate() {
            let d = rgb_dist(packed, entry.load(Ordering::Relaxed) & 0x00FF_FFFF);
            if d < best_d {
                best_d = d;
                best = i as u8;
            }
        }
        best
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

    #[allow(clippy::too_many_arguments)]
    fn draw_line(
        &self,
        x0: i16,
        y0: i16,
        x1: i16,
        y1: i16,
        pel: Pel,
        mode: WriteMode,
        clip: ClipRect,
        mask: u16,
    ) {
        let fb = active_fb();
        let color = Self::pel_to_rgb(pel);
        // Background pen (pen 0) for Replace's dash gaps — EmuTOS
        // `swblit_vertical_line16` writes `bgcol = palette[0]` on the
        // dash-off pixels in `WM_REPLACE`.
        let bg = Self::pel_to_rgb(0);
        // Bresenham; touch only pixels inside the inclusive clip rect (and
        // the framebuffer, which `combine` enforces too). `bit` walks the
        // 16-bit dash mask from the MSB, one step per pixel; a set mask bit
        // is the line's "source set" for the write mode.
        let (mut x, mut y) = (x0 as i32, y0 as i32);
        let (xe, ye) = (x1 as i32, y1 as i32);
        let dx = (xe - x).abs();
        let dy = -(ye - y).abs();
        let sx = if x < xe { 1 } else { -1 };
        let sy = if y < ye { 1 } else { -1 };
        let mut err = dx + dy;
        let mut bit: u32 = 0x8000;
        loop {
            if x >= clip.xmn as i32
                && x <= clip.xmx as i32
                && y >= clip.ymn as i32
                && y <= clip.ymx as i32
            {
                let dash_on = mask as u32 & bit != 0;
                Self::combine(fb, x as u32, y as u32, dash_on, mode, color, bg);
            }
            bit = if bit == 1 { 0x8000 } else { bit >> 1 };
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
                let set = row & (0x8000u16 >> (xx & 0xF)) != 0;
                Self::combine(fb, xx as u32, yy as u32, set, fill.mode, fg_rgb, bg_rgb);
            }
        }
        Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    }

    fn draw_glyph(&self, x: i16, y: i16, glyph: &[u8; 16], fg: Pel, mode: WriteMode, clip: ClipRect) {
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
        // Replace's cell-clear background is VDI pen 0 (`bgcol = palette[0]`
        // in EmuTOS `vdi_textblit.c` `WM_REPLACE`).
        let bg_rgb = Self::pel_to_rgb(0);
        for yy in y0..=y1 {
            let bits = glyph[(yy - y as i32) as usize];
            for xx in x0..=x1 {
                let col = xx - x as i32; // 0..=7 within the cell
                let set = bits & (0x80u8 >> col) != 0;
                Self::combine(fb, xx as u32, yy as u32, set, mode, fg_rgb, bg_rgb);
            }
        }
        Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    }

    fn blit_mono(&self, dst: Rect, src: MonoSrc, fg: Pel, bg: Pel, mode: WriteMode, clip: ClipRect) {
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
        // Reuse the shared four-mode `combine` (DRY with glyph/pattern/line).
        // `combine`'s `RevTransparent` paints its `fg` slot on the *clear*
        // bits; `vrt_cpyfm`'s `MD_ERASE` paints the **background** colour
        // (INTIN[2]) there, so hand `combine` `bg_rgb` as its fg slot for
        // that one mode. All other modes forward (fg, bg) directly:
        //   Replace: set→fg, clear→bg; Transparent: set→fg; Xor: set→~D.
        let (c_fg, c_bg) = match mode {
            WriteMode::RevTransparent => (bg_rgb, fg_rgb),
            _ => (fg_rgb, bg_rgb),
        };
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
                Self::combine(fb, dxc as u32, dyc as u32, set, mode, c_fg, c_bg);
            }
        }
        Self::clean(fb, x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    }

    fn copy_rect(&self, src: Rect, dx: i16, dy: i16, op: u8, clip: ClipRect) {
        // BM_D_ONLY (op 5): `D' = D` — the destination is untouched, so we
        // do nothing at all (EmuTOS `vro_cpyfm16` "nothing to do"; avoids
        // needlessly re-writing/cleaning the rectangle and any corruption).
        if op & 0x0f == 5 {
            return;
        }
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
                    let val = raster_op(op, sp, dp.read_volatile());
                    dp.write_volatile(val);
                }
            }
        }
        Self::clean(fb, tx0 as u32, ty0 as u32, tx1 as u32, ty1 as u32);
    }
}

/// The 16-entry BitBlt logic-op table, `D' = op(S, D)`, acting **literally
/// on the packed pixel word** (`vro_cpyfm`, EmuTOS `vdi_raster.c:495-544` /
/// `vro_cpyfm16:1008-1220`). Only the low nibble of `op` selects the
/// operation (`PAT_FLAG` is stripped by the caller); the same chunky word
/// semantics EmuTOS' truecolor `vro_cpyfm16` uses. Note the Atari planar
/// names invert against a truecolor screen: `ALL_WHITE` (0) writes `0`
/// (black), `ALL_BLACK` (15) writes `!0` (white) — the op acts on bits,
/// not palette indices.
#[inline]
pub fn raster_op(op: u8, s: u32, d: u32) -> u32 {
    match op & 0x0f {
        0 => 0,             // BM_ALL_WHITE   D' = 0
        1 => s & d,         // BM_S_AND_D     D' = S & D
        2 => s & !d,        // BM_S_AND_NOTD  D' = S & ~D
        3 => s,             // BM_S_ONLY      D' = S            (replace)
        4 => !s & d,        // BM_NOTS_AND_D  D' = ~S & D
        5 => d,             // BM_D_ONLY      D' = D            (no-op)
        6 => s ^ d,         // BM_S_XOR_D     D' = S ^ D        (XOR)
        7 => s | d,         // BM_S_OR_D      D' = S | D        (OR)
        8 => !(s | d),      // BM_NOT_SORD    D' = ~(S | D)
        9 => !(s ^ d),      // BM_NOT_SXORD   D' = ~(S ^ D)
        10 => !d,           // BM_NOT_D       D' = ~D
        11 => s | !d,       // BM_S_OR_NOTD   D' = S | ~D
        12 => !s,           // BM_NOT_S       D' = ~S
        13 => !s | d,       // BM_NOTS_OR_D   D' = ~S | D
        14 => !(s & d),     // BM_NOT_SANDD   D' = ~(S & D)
        _ => !0,            // BM_ALL_BLACK   D' = ~0
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
