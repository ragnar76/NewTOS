//! Software VDI rasterizer backend (Phase 12).
//!
//! Implements [`tos_api::vdi::backend::RasterBackend`] over the board's
//! linear [`Framebuffer`]. This is the **bring-up** backend: every
//! primitive is plain scalar CPU code. It is the swappable boundary the
//! VDI layer draws through — a later NEON / DMA / VideoCore-V3D backend
//! replaces this impl without the VDI layer changing (see the
//! `RasterBackend` docs).
//!
//! Colours arrive as PEL (palette index); we resolve each at the write
//! point through the **active** per-workstation pseudo-palette
//! ([`ACTIVE_PALETTE`], EmuTOS `CUR_WORK->ext->palette`, `vdi_line.c:459`),
//! which the VDI dispatcher points at the drawing workstation before every
//! colour-sensitive op. The pseudo-palette holds one RGB555-in-565 word per
//! pen (Falcon truecolor, `vdi_col.c:601-614`); the backend expands each gun
//! 5→8 bits into the 32-bpp XRGB8888 framebuffer (`c8 = (c5<<3)|(c5>>2)`).
//! VDI convention: index 0 = white (background), 1 = black (foreground).
//! Pens 0..15 default to EmuTOS `videl_palette1`, pens 16..255 to
//! `videl_palette2`. The advertised colour count (`DEV_TAB[13]`) is 256,
//! matching a Falcon truecolor workstation, while the framebuffer stays
//! 32-bpp XRGB8888 (an internal detail per `docs/COMPATIBILITY.md`).

use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};

use crate::hal::framebuffer::{rgb, Framebuffer};

/// Number of palette entries the VDI advertises as `DEV_TAB[13]`. The
/// Falcon truecolor profile reports 256 (`vdi_control.c:206`,
/// `numcolors = (v_planes<8) ? (1<<v_planes) : 256`), decoupled from the
/// bit-plane count [`PLANES`].
const NUM_COLORS: u16 = 256;

/// Bit-plane count the VDI advertises as `INQ_TAB[4] = v_planes`
/// (`vdi_control.c:210`). Under Falcon truecolor this is **16**
/// (`bios/videl.c:320-341`, `get_videl_bpp` SPS_HICOLOR → 16), *not*
/// `log2(NUM_COLORS)` — the two only coincide in the palette modes
/// (`vdi/vdi_defs.h:22`, `TRUECOLOR_MODE (v_planes > 8)`).
const PLANES: u16 = 16;

/// EmuTOS' `videl_palette1` (`vdi/vdi_col.c:130-136`): the Falcon default
/// palette for pens 0..15, as VDI `(r, g, b)` triples in the 0..=1000
/// range. This is the device-global default a truecolor workstation seeds
/// its low pens from (`init_colors`, `vdi_col.c:718`
/// `memcpy(REQ_COL, videl_palette1, …)`). Index 0 is white (background),
/// 1 is black (foreground); pens 16..255 come from [`VIDEL_PALETTE2`].
/// Pens 8..15 are the Falcon look (light grey 733, dark grey 533,
/// half-intensity primaries 667) — visibly different from the plain GEM-16
/// greys, and the intended appearance for a 1:1 EmuTOS-Falcon desktop.
#[rustfmt::skip]
const VIDEL_PALETTE1: [(u16, u16, u16); 16] = [
    (1000, 1000, 1000), (0, 0, 0),       (1000, 0, 0),    (0, 1000, 0),    // 0..3
    (0, 0, 1000),       (0, 1000, 1000), (1000, 1000, 0), (1000, 0, 1000), // 4..7
    (733, 733, 733),    (533, 533, 533), (667, 0, 0),     (0, 667, 0),     // 8..11
    (0, 0, 667),        (0, 667, 667),   (667, 667, 0),   (667, 0, 667),   // 12..15
];

/// EmuTOS' extended VIDEL default palette for pens 16..255, as VDI
/// `(r, g, b)` triples in the 0..=1000 range — copied verbatim from
/// `vdi/vdi_col.c:137-199` (`videl_palette2[240][3]`). This is the
/// device-global default a truecolor workstation seeds its extended pens
/// from; the current desktop/AES never use these pens, so their exact
/// values are not observable yet, but they are kept EmuTOS-faithful.
#[rustfmt::skip]
const VIDEL_PALETTE2: [(u16, u16, u16); 240] = [
    (1000, 1000, 1000), (933, 933, 933), (867, 867, 867), (800, 800, 800),
    (733, 733, 733), (667, 667, 667), (600, 600, 600), (533, 533, 533),
    (467, 467, 467), (400, 400, 400), (333, 333, 333), (267, 267, 267),
    (200, 200, 200), (133, 133, 133), (67, 67, 67), (0, 0, 0),
    (1000, 0, 0), (1000, 0, 67), (1000, 0, 133), (1000, 0, 200),
    (1000, 0, 267), (1000, 0, 333), (1000, 0, 400), (1000, 0, 467),
    (1000, 0, 533), (1000, 0, 600), (1000, 0, 667), (1000, 0, 733),
    (1000, 0, 800), (1000, 0, 867), (1000, 0, 933), (1000, 0, 1000),
    (933, 0, 1000), (867, 0, 1000), (800, 0, 1000), (733, 0, 1000),
    (667, 0, 1000), (600, 0, 1000), (533, 0, 1000), (467, 0, 1000),
    (400, 0, 1000), (333, 0, 1000), (267, 0, 1000), (200, 0, 1000),
    (133, 0, 1000), (67, 0, 1000), (0, 0, 1000), (0, 67, 1000),
    (0, 133, 1000), (0, 200, 1000), (0, 267, 1000), (0, 333, 1000),
    (0, 400, 1000), (0, 467, 1000), (0, 533, 1000), (0, 600, 1000),
    (0, 667, 1000), (0, 733, 1000), (0, 800, 1000), (0, 867, 1000),
    (0, 933, 1000), (0, 1000, 1000), (0, 1000, 933), (0, 1000, 867),
    (0, 1000, 800), (0, 1000, 733), (0, 1000, 667), (0, 1000, 600),
    (0, 1000, 533), (0, 1000, 467), (0, 1000, 400), (0, 1000, 333),
    (0, 1000, 267), (0, 1000, 200), (0, 1000, 133), (0, 1000, 67),
    (0, 1000, 0), (67, 1000, 0), (133, 1000, 0), (200, 1000, 0),
    (267, 1000, 0), (333, 1000, 0), (400, 1000, 0), (467, 1000, 0),
    (533, 1000, 0), (600, 1000, 0), (667, 1000, 0), (733, 1000, 0),
    (800, 1000, 0), (867, 1000, 0), (933, 1000, 0), (1000, 1000, 0),
    (1000, 933, 0), (1000, 867, 0), (1000, 800, 0), (1000, 733, 0),
    (1000, 667, 0), (1000, 600, 0), (1000, 533, 0), (1000, 467, 0),
    (1000, 400, 0), (1000, 333, 0), (1000, 267, 0), (1000, 200, 0),
    (1000, 133, 0), (1000, 67, 0), (733, 0, 0), (733, 0, 67),
    (733, 0, 133), (733, 0, 200), (733, 0, 267), (733, 0, 333),
    (733, 0, 400), (733, 0, 467), (733, 0, 533), (733, 0, 600),
    (733, 0, 667), (733, 0, 733), (667, 0, 733), (600, 0, 733),
    (533, 0, 733), (467, 0, 733), (400, 0, 733), (333, 0, 733),
    (267, 0, 733), (200, 0, 733), (133, 0, 733), (67, 0, 733),
    (0, 0, 733), (0, 67, 733), (0, 133, 733), (0, 200, 733),
    (0, 267, 733), (0, 333, 733), (0, 400, 733), (0, 467, 733),
    (0, 533, 733), (0, 600, 733), (0, 667, 733), (0, 733, 733),
    (0, 733, 667), (0, 733, 600), (0, 733, 533), (0, 733, 467),
    (0, 733, 400), (0, 733, 333), (0, 733, 267), (0, 733, 200),
    (0, 733, 133), (0, 733, 67), (0, 733, 0), (67, 733, 0),
    (133, 733, 0), (200, 733, 0), (267, 733, 0), (333, 733, 0),
    (400, 733, 0), (467, 733, 0), (533, 733, 0), (600, 733, 0),
    (667, 733, 0), (733, 733, 0), (733, 667, 0), (733, 600, 0),
    (733, 533, 0), (733, 467, 0), (733, 400, 0), (733, 333, 0),
    (733, 267, 0), (733, 200, 0), (733, 133, 0), (733, 67, 0),
    (467, 0, 0), (467, 0, 67), (467, 0, 133), (467, 0, 200),
    (467, 0, 267), (467, 0, 333), (467, 0, 400), (467, 0, 467),
    (400, 0, 467), (333, 0, 467), (267, 0, 467), (200, 0, 467),
    (133, 0, 467), (67, 0, 467), (0, 0, 467), (0, 67, 467),
    (0, 133, 467), (0, 200, 467), (0, 267, 467), (0, 333, 467),
    (0, 400, 467), (0, 467, 467), (0, 467, 400), (0, 467, 333),
    (0, 467, 267), (0, 467, 200), (0, 467, 133), (0, 467, 67),
    (0, 467, 0), (67, 467, 0), (133, 467, 0), (200, 467, 0),
    (267, 467, 0), (333, 467, 0), (400, 467, 0), (467, 467, 0),
    (467, 400, 0), (467, 333, 0), (467, 267, 0), (467, 200, 0),
    (467, 133, 0), (467, 67, 0), (267, 0, 0), (267, 0, 67),
    (267, 0, 133), (267, 0, 200), (267, 0, 267), (200, 0, 267),
    (133, 0, 267), (67, 0, 267), (0, 0, 267), (0, 67, 267),
    (0, 133, 267), (0, 200, 267), (0, 267, 267), (0, 267, 200),
    (0, 267, 133), (0, 267, 67), (0, 267, 0), (67, 267, 0),
    (133, 267, 0), (200, 267, 0), (267, 267, 0), (267, 200, 0),
    (267, 133, 0), (267, 67, 0), (1000, 1000, 1000), (0, 0, 0),
];

/// The **active** VDI pseudo-palette the draw path resolves each PEL
/// through at the write point (EmuTOS `CUR_WORK->ext->palette`,
/// `vdi_line.c:459-460`). One 16-bit value per pen, laid out
/// `RRRRRGGGGG0BBBBB` (RGB555 packed into a 565 slot, green LSB / bit 5
/// always 0 — EmuTOS `set_color16`, `vdi_col.c:601-614`). The VDI
/// dispatcher points this at the drawing workstation's own per-`Vwk`
/// pseudo-palette before every colour-sensitive op via
/// [`RasterBackend::set_active_palette`]; its lazily-seeded default is the
/// 5-bit quantisation of the device default (`videl_palette1` /
/// `videl_palette2`), identical to what a fresh workstation seeds, so an
/// in-kernel draw before any op resolves the same colours. Accessed only
/// from the single trap path; `AtomicU16` keeps it `Sync` without `unsafe`.
static ACTIVE_PALETTE: [AtomicU16; NUM_COLORS as usize] =
    [const { AtomicU16::new(0) }; NUM_COLORS as usize];

/// The device-global requested RGB (0..=1000) per pen, packed
/// `r<<20 | g<<10 | b`, returned by `vq_color` and used to seed each
/// workstation's private colour table (`init_wk`). Seeded from
/// [`VIDEL_PALETTE1`] (0..15) + [`VIDEL_PALETTE2`] (16..255); `set_color`
/// (XBIOS / the non-truecolor path) overwrites entries.
static REQ_COL: [AtomicU32; NUM_COLORS as usize] =
    [const { AtomicU32::new(0) }; NUM_COLORS as usize];

/// Set once [`REQ_COL`] / [`ACTIVE_PALETTE`] hold their seeded defaults.
static PAL_INIT: AtomicBool = AtomicBool::new(false);

/// Quantise a VDI colour component (0..=1000, caller-clamped) to 5 bits
/// (0..31). EmuTOS `vdi2fivebits` (`vdi_col.c:588-591`): scale 1000 → 31.
const fn vdi2fivebits(col: u16) -> u16 {
    ((col as u32 * 31 + 500) / 1000) as u16
}

/// Pack three VDI colour components (0..=1000) into the RGB555-in-565
/// pseudo-palette word EmuTOS' `set_color16` builds (`vdi_col.c:601-614`):
/// `(r5<<11)|(g5<<6)|b5`. Bit 5 (the green LSB) stays 0 by construction.
const fn pack555(r: u16, g: u16, b: u16) -> u16 {
    (vdi2fivebits(r) << 11) | (vdi2fivebits(g) << 6) | vdi2fivebits(b)
}

/// Expand a 5-bit colour component (0..31) to 8 bits by bit-replication —
/// the Falcon VIDEL truecolor DAC expansion: `c8 = (c5<<3)|(c5>>2)`.
const fn expand5to8(c5: u16) -> u8 {
    ((c5 << 3) | (c5 >> 2)) as u8
}

/// Expand one RGB555-in-565 pseudo-palette word to packed XRGB8888 (the
/// framebuffer pixel format), 5→8-bit per gun.
fn pal555_to_xrgb(p: u16) -> u32 {
    rgb(
        expand5to8((p >> 11) & 0x1f),
        expand5to8((p >> 6) & 0x1f),
        expand5to8(p & 0x1f),
    )
}

/// Squared RGB distance between two packed XRGB colours (for the
/// reverse palette lookup in `read_pixel`).
fn rgb_dist(a: u32, b: u32) -> u32 {
    let dr = ((a >> 16) & 0xFF) as i32 - ((b >> 16) & 0xFF) as i32;
    let dg = ((a >> 8) & 0xFF) as i32 - ((b >> 8) & 0xFF) as i32;
    let db = (a & 0xFF) as i32 - (b & 0xFF) as i32;
    (dr * dr + dg * dg + db * db) as u32
}

/// Seed the device defaults on first use: the requested RGB
/// ([`REQ_COL`], 0..1000) and the 5-bit-quantised active pseudo-palette
/// ([`ACTIVE_PALETTE`]) from EmuTOS `videl_palette1` (pens 0..15) and
/// `videl_palette2` (pens 16..255). The active-palette quantisation
/// mirrors EmuTOS `set_color16` exactly, so it matches what each fresh
/// workstation seeds via `init_wk`.
fn ensure_palette_init() {
    if PAL_INIT.load(Ordering::Relaxed) {
        return;
    }
    let seed = |i: usize, r: u16, g: u16, b: u16| {
        let req = (r as u32) << 20 | (g as u32) << 10 | b as u32;
        REQ_COL[i].store(req, Ordering::Relaxed);
        ACTIVE_PALETTE[i].store(pack555(r, g, b), Ordering::Relaxed);
    };
    for (i, &(r, g, b)) in VIDEL_PALETTE1.iter().enumerate() {
        seed(i, r, g, b);
    }
    for (j, &(r, g, b)) in VIDEL_PALETTE2.iter().enumerate() {
        seed(16 + j, r, g, b);
    }
    PAL_INIT.store(true, Ordering::Relaxed);
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
        // Resolve the PEL through the *active* workstation pseudo-palette at
        // the write point (EmuTOS `CUR_WORK->ext->palette`,
        // `vdi_line.c:459-460`), expanding the 5-bit guns to 8 bits. `pel` is
        // a `u8`, so it always indexes the 256-entry array in bounds.
        pal555_to_xrgb(ACTIVE_PALETTE[pel as usize].load(Ordering::Relaxed))
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
        if tos_api::xbios::screen::direct_screen_active() {
            // A process holds a Normal Non-Cacheable EL0 alias of these very
            // pixels (`Physbase`/`Logbase`, ADR-023). Leaving *any* line of
            // the rectangle in this cacheable alias would break the pair: a
            // later partial write to a stale line writes the whole 64-byte
            // line back and would undo the program's own stores. Clean **and
            // invalidate**, so nothing of the framebuffer survives a
            // primitive (ARM ARM, "Mismatched memory attributes").
            // SAFETY: the span lies within the framebuffer's pixel store.
            unsafe {
                crate::arch::aarch64::mmu::clean_invalidate_dcache_range(start, end - start)
            };
        } else {
            // SAFETY: the span lies within the framebuffer's pixel store.
            unsafe { crate::arch::aarch64::mmu::clean_dcache_range(start, end - start) };
        }
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

    fn planes(&self) -> u16 {
        PLANES
    }

    fn set_color(&self, index: u16, r: u16, g: u16, b: u16) {
        if index >= NUM_COLORS {
            return;
        }
        ensure_palette_init();
        let (r, g, b) = (r.min(1000), g.min(1000), b.min(1000));
        let req = (r as u32) << 20 | (g as u32) << 10 | b as u32;
        REQ_COL[index as usize].store(req, Ordering::Relaxed);
        // Program the active pseudo-palette (XBIOS `Setcolor` / the
        // non-truecolor CLUT path), 5-bit-quantised like the Falcon DAC. The
        // VDI truecolor draw path resets the active palette per-op from the
        // drawing workstation's own table, so this is the immediate colour a
        // non-VDI palette write (XBIOS) leaves until the next VDI op.
        ACTIVE_PALETTE[index as usize].store(pack555(r, g, b), Ordering::Relaxed);
    }

    fn query_color(&self, index: u16) -> (u16, u16, u16) {
        if index >= NUM_COLORS {
            return (0, 0, 0);
        }
        ensure_palette_init();
        let req = REQ_COL[index as usize].load(Ordering::Relaxed);
        (((req >> 20) & 0x3FF) as u16, ((req >> 10) & 0x3FF) as u16, (req & 0x3FF) as u16)
    }

    fn set_active_palette(&self, palette: &[u16; 256]) {
        ensure_palette_init();
        for (slot, &p) in ACTIVE_PALETTE.iter().zip(palette.iter()) {
            slot.store(p, Ordering::Relaxed);
        }
    }

    fn pel_to_device16(&self, pel: Pel) -> u16 {
        ensure_palette_init();
        // The active per-workstation pseudo-palette holds one RGB555-in-565
        // word per pen (EmuTOS `CUR_WORK->ext->palette`); `vrt_cpyfm16` writes
        // exactly this word into a memory destination (`vdi_raster.c:1234`).
        // `pel` is a `u8`, so it always indexes the 256-entry array in bounds.
        ACTIVE_PALETTE[pel as usize].load(Ordering::Relaxed)
    }

    fn device_to_rgb565(&self, argb: u32) -> u16 {
        // XRGB8888 → RGB555-in-565: keep the top 5 bits of each 8-bit gun
        // (bit 5 / green LSB stays 0 by construction). The framebuffer is
        // 5-bit-quantised, so this recovers the exact stored value.
        let r5 = ((argb >> 19) & 0x1f) as u16;
        let g5 = ((argb >> 11) & 0x1f) as u16;
        let b5 = ((argb >> 3) & 0x1f) as u16;
        (r5 << 11) | (g5 << 6) | b5
    }

    fn rgb565_to_device(&self, px: u16) -> u32 {
        // RGB555-in-565 word → XRGB8888, 5→8-bit gun replication — the same
        // expansion the draw path uses, so a restored pixel equals the drawn
        // one bit-for-bit.
        pal555_to_xrgb(px)
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
        // Reverse-map the XRGB value to the nearest active-palette index —
        // the same per-Vwk pseudo-palette the draw path resolves through, so
        // a pixel this backend drew round-trips exactly.
        let mut best = 0u8;
        let mut best_d = u32::MAX;
        for (i, entry) in ACTIVE_PALETTE.iter().enumerate() {
            let d = rgb_dist(packed, pal555_to_xrgb(entry.load(Ordering::Relaxed)) & 0x00FF_FFFF);
            if d < best_d {
                best_d = d;
                best = i as u8;
            }
        }
        best
    }

    fn clear_screen_word(&self, word: u16) {
        // EmuTOS clears the whole screen with a single
        // `memset(v_bas_ad, fill, size)` (`vdi_control.c:599-618`, the memset at
        // :618) — one bulk store over the framebuffer, not a pixel loop. The
        // trait's default reproduces the *result* through `put_device_pixel`,
        // which on this backend is one dynamically-dispatched call, bounds check
        // and stride multiply per pixel — over two million of them at 1920x1080,
        // on the boot path (`v_clrwk` from `initinfo`). The framebuffer HAL
        // already owns a bulk fill, so hand it the whole visible area in one call.
        //
        // Same pixels, same values as the default: `Framebuffer::clear` fills
        // `[0, width) x [0, height)` at `y * stride_px + x` — exactly the
        // addresses `put_device_pixel` would have written — with the identical
        // `rgb565_to_device(word)`, so a cleared screen stays bit-for-bit what it
        // was (both `0xFFFF` and the pseudo-palette's white `0xFFDF` fold to
        // `0xFFFFFF`, five green bits either way). The pitch padding a VideoCore
        // line may carry past `width` is left alone, as the default left it; it
        // is never scanned out.
        let fb = active_fb();
        fb.clear(self.rgb565_to_device(word));
        // Then the flush the default performs, i.e. this backend's normal
        // post-write cache maintenance: clean the visible area to the point of
        // coherency for the display controller (a no-op span if the framebuffer
        // is not up yet).
        let (w, h) = (self.width(), self.height());
        self.flush_rect(0, 0, w.saturating_sub(1) as i16, h.saturating_sub(1) as i16);
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
    fn abline(
        &self,
        x0: i16,
        y0: i16,
        x1: i16,
        y1: i16,
        pel: Pel,
        mode: WriteMode,
        clip: ClipRect,
        mask: u16,
        last: bool,
    ) -> u16 {
        let fb = active_fb();
        let color = Self::pel_to_rgb(pel);
        // Background pen (pen 0) for Replace's dash gaps — EmuTOS
        // `swblit_*16` writes `bgcol = palette[0]` on the dash-off pixels in
        // `WM_REPLACE`.
        let bg = Self::pel_to_rgb(0);

        // Always draw left→right (EmuTOS `abline:2300`): the dash walks from
        // the geometrically-left endpoint. Vertical lines (x equal) keep the
        // given point order, matching EmuTOS' vertical path (draws from y1).
        let (ax, ay, mut bx, mut by) = if x1 < x0 {
            (x1, y1, x0, y0)
        } else {
            (x0, y0, x1, y1)
        };

        // XOR `LSTLIN` kludge (EmuTOS `abline:2319`): on any but the last
        // segment of a polyline, shorten the far endpoint by one pixel so the
        // vertex shared with the next segment is not XOR'd twice.
        if mode == WriteMode::Xor && !last {
            if ax != bx {
                bx -= 1;
            } else if ay != by {
                by -= 1;
            }
        }

        // Horizontal segment: EmuTOS routes it through `draw_rect_common`
        // (`abline:2330`), whose dash is **screen-aligned** — the pattern bit
        // is `mask & (0x8000 >> (x & 0xf))` at absolute x — and `LN_MASK` is
        // *not* advanced. Return the mask unchanged.
        if ay == by {
            let y = ay as i32;
            let hmax = fb.height() as i32 - 1;
            if y >= clip.ymn as i32 && y <= clip.ymx as i32 && y >= 0 && y <= hmax {
                let wmax = fb.width() as i32 - 1;
                let xlo = (ax.min(bx) as i32).max(clip.xmn as i32).max(0);
                let xhi = (ax.max(bx) as i32).min(clip.xmx as i32).min(wmax);
                if xlo <= xhi {
                    for x in xlo..=xhi {
                        let set = mask & (0x8000u16 >> (x & 0xf)) != 0;
                        Self::combine(fb, x as u32, y as u32, set, mode, color, bg);
                    }
                    Self::clean(fb, xlo as u32, y as u32, xhi as u32, y as u32);
                }
            }
            return mask;
        }

        // General (vertical / diagonal) line: Bresenham from the left
        // endpoint, rotating the dash mask one bit per plotted pixel and
        // writing it back (EmuTOS `draw_line16` rolw1 + `LN_MASK = linemask`,
        // `:1645-1770`). `combine` enforces the framebuffer bounds; we clip to
        // the inclusive workstation rectangle here.
        let (mut x, mut y) = (ax as i32, ay as i32);
        let (xe, ye) = (bx as i32, by as i32);
        let dx = (xe - x).abs();
        let dy = -(ye - y).abs();
        let sx = if x < xe { 1 } else { -1 };
        let sy = if y < ye { 1 } else { -1 };
        let mut err = dx + dy;
        let mut linemask = mask;
        loop {
            // rolw1(linemask): rotate left 1, then test bit 0 (MSB-first),
            // exactly as EmuTOS advances the line style before each pixel.
            linemask = linemask.rotate_left(1);
            if x >= clip.xmn as i32
                && x <= clip.xmx as i32
                && y >= clip.ymn as i32
                && y <= clip.ymx as i32
            {
                let dash_on = linemask & 1 != 0;
                Self::combine(fb, x as u32, y as u32, dash_on, mode, color, bg);
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
        let xlo = (ax.min(bx) as i32).clamp(0, wmax) as u32;
        let xhi = (ax.max(bx) as i32).clamp(0, wmax) as u32;
        let ylo = (ay.min(by) as i32).clamp(0, hmax) as u32;
        let yhi = (ay.max(by) as i32).clamp(0, hmax) as u32;
        Self::clean(fb, xlo, ylo, xhi, yhi);
        linemask
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

    #[allow(clippy::too_many_arguments)]
    fn draw_glyph_sized(
        &self,
        x: i16,
        y: i16,
        glyph: &[u8],
        cell_w: i16,
        cell_h: i16,
        fg: Pel,
        mode: WriteMode,
        clip: ClipRect,
    ) {
        let fb = active_fb();
        // The `cell_w`×`cell_h` cell intersected with the clip rectangle and
        // the screen — only pixels inside this inclusive box are touched.
        // (For the 8×16 system font, `cell_w-1 = 7` / `cell_h-1 = 15`, so
        // this reduces exactly to the former fixed-size glyph path.)
        let cell = ClipRect {
            xmn: x.max(clip.xmn),
            ymn: y.max(clip.ymn),
            xmx: ((x as i32 + cell_w as i32 - 1) as i16).min(clip.xmx),
            ymx: ((y as i32 + cell_h as i32 - 1) as i16).min(clip.ymx),
        };
        let Some((x0, y0, x1, y1)) = cell.clamp_to(fb.width() as u16, fb.height() as u16) else {
            return;
        };
        let fg_rgb = Self::pel_to_rgb(fg);
        // Replace's cell-clear background is VDI pen 0 (`bgcol = palette[0]`
        // in EmuTOS `vdi_textblit.c` `WM_REPLACE`).
        let bg_rgb = Self::pel_to_rgb(0);
        // Each row occupies `bytes_per_row = ceil(cell_w/8)` bytes, MSB-first,
        // leftmost byte first (so a cell wider than 8 px — text effects such as
        // outline/skew widen the cell beyond the base 8 px — is a run of bytes
        // per row). For `cell_w <= 8` this is one byte per row: `col/8 == 0` and
        // `col & 7 == col`, so the 8×16 / 6×6 / 8×8 system-font path is byte-for-
        // byte the former single-byte-per-row blit.
        let bpr = (cell_w as usize).div_ceil(8);
        for yy in y0..=y1 {
            let base = (yy - y as i32) as usize * bpr;
            for xx in x0..=x1 {
                let col = (xx - x as i32) as usize; // 0..cell_w within the cell
                let byte = glyph[base + col / 8];
                let set = byte & (0x80u8 >> (col & 7)) != 0;
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

    fn get_device_pixel(&self, x: i16, y: i16) -> u32 {
        let fb = active_fb();
        if x < 0 || y < 0 || x as u32 >= fb.width() || y as u32 >= fb.height() {
            return 0;
        }
        let off = y as usize * fb.stride_px() as usize + x as usize;
        // SAFETY: (x, y) is within the framebuffer bounds checked above.
        unsafe { fb.pixels().add(off).read_volatile() }
    }

    fn put_device_pixel(&self, x: i16, y: i16, argb: u32) {
        let fb = active_fb();
        if x < 0 || y < 0 || x as u32 >= fb.width() || y as u32 >= fb.height() {
            return;
        }
        let off = y as usize * fb.stride_px() as usize + x as usize;
        // SAFETY: (x, y) is within the framebuffer bounds checked above.
        // The caller flushes the region afterwards (see `flush_rect`).
        unsafe { fb.pixels().add(off).write_volatile(argb) };
    }

    fn flush_rect(&self, x0: i16, y0: i16, x1: i16, y1: i16) {
        let fb = active_fb();
        let (w, h) = (fb.width(), fb.height());
        if w == 0 || h == 0 {
            return;
        }
        let cx0 = x0.max(0) as u32;
        let cy0 = y0.max(0) as u32;
        let cx1 = (x1.max(0) as u32).min(w - 1);
        let cy1 = (y1.max(0) as u32).min(h - 1);
        if cx0 > cx1 || cy0 > cy1 {
            return;
        }
        Self::clean(fb, cx0, cy0, cx1, cy1);
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

/// Bytes per scan line of the live framebuffer — the hardware pitch, which
/// may exceed `width * 4` when the display controller pads lines (the
/// VideoCore reports it via the mailbox `get pitch` tag). Feeds
/// [`tos_api::host::ScreenInfo::stride_bytes`].
pub fn stride_bytes() -> u32 {
    active_fb().stride_px() * 4
}

/// Byte size of the CPU-visible pixel store (`stride_bytes * height`).
/// Feeds [`tos_api::host::ScreenInfo::size_bytes`] — the span the XBIOS
/// screen group publishes into a process that draws into the screen itself.
pub fn size_bytes() -> u32 {
    stride_bytes().saturating_mul(active_fb().height())
}
