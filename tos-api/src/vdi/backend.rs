//! VDI rasterizer backend — the swappable hardware boundary.
//!
//! The VDI layer above (dispatcher, `Vwk` attribute state, validation,
//! clipping, coordinate handling) is **hardware-independent**. Only the
//! actual pixel-touching primitives live behind this trait, so a later
//! NEON / DMA / VideoCore-V3D accelerated implementation can replace the
//! backend without the VDI layer changing at all.
//!
//! Bring-up provides a single software implementation (in the kernel)
//! that draws into the [`hal::framebuffer`] linear buffer. The kernel
//! hands the active backend to `tos-api` through
//! [`crate::host::Host::raster`].
//!
//! Colours cross this boundary as **PEL** (palette indices, see
//! [`Pel`]); the backend owns the palette → device-colour mapping, just
//! as the hardware CLUT would. This matches how the VDI stores colours
//! in the [`Vwk`](crate::vdi::vwk::Vwk).

/// A palette colour index (PEL), as stored in the `Vwk` and passed to
/// the backend. The backend resolves it to a concrete device colour
/// (RGB for the bring-up truecolor framebuffer).
pub type Pel = u8;

/// The set of raster primitives the VDI layer needs from the device.
///
/// Intentionally minimal for the workstation-lifecycle milestone
/// (D1/D2): geometry queries plus a rectangle fill (used by `v_clrwk`).
/// Output primitives (`line`, `span_fill`, `blit`, `glyph`) are added as
/// the corresponding VDI opcodes are implemented (D3+), so the trait
/// never carries an unimplemented method.
pub trait RasterBackend: Sync {
    /// Visible width in device pixels.
    fn width(&self) -> u16;

    /// Visible height in device pixels.
    fn height(&self) -> u16;

    /// Number of simultaneously displayable colours (CLUT size as the
    /// VDI advertises it in `DEV_TAB[13]`). The bring-up palette is
    /// 16 colours regardless of the truecolor framebuffer underneath.
    fn num_colors(&self) -> u16;

    /// Set palette entry `index` to the requested RGB, each component in
    /// the VDI's 0..=1000 range (`vs_color`). The backend remembers the
    /// request (for [`query_color`](Self::query_color)) and programs the
    /// device CLUT. Out-of-range indices are ignored.
    fn set_color(&self, index: u16, r: u16, g: u16, b: u16);

    /// Read back the last requested RGB (0..=1000) of palette entry
    /// `index` (`vq_color` "requested" mode). Out-of-range → `(0,0,0)`.
    fn query_color(&self, index: u16) -> (u16, u16, u16);

    /// Read the palette index (PEL) at device pixel `(x, y)`, reverse-
    /// mapping the stored colour to the nearest CLUT entry (`v_get_pixel`,
    /// `v_contourfill`). Out-of-bounds → `0`.
    fn read_pixel(&self, x: i16, y: i16) -> Pel;

    /// Fill the rectangle `[x, x+w) × [y, y+h)` (device coordinates,
    /// origin top-left) with palette colour `pel`. The draw is clipped to
    /// `clip` ∩ the visible area; negative / oversized rectangles are
    /// clamped.
    fn fill_rect(&self, x: i16, y: i16, w: i16, h: i16, pel: Pel, clip: ClipRect);

    /// Draw a one-pixel-wide line from `(x0, y0)` to `(x1, y1)` in
    /// palette colour `pel`, clipped to `[xmn, ymn]..=[xmx, ymx]`
    /// (inclusive device coordinates — the caller passes the workstation
    /// clip rectangle). Endpoints are inclusive. `mask` is the 16-bit
    /// dash pattern (`0xFFFF` = solid), consumed one bit per plotted step
    /// and rotated, matching the VDI `LN_MASK`.
    #[allow(clippy::too_many_arguments)]
    fn draw_line(&self, x0: i16, y0: i16, x1: i16, y1: i16, pel: Pel, clip: ClipRect, mask: u16);

    /// Fill the rectangle `[x, x+w) × [y, y+h)` with the monochrome
    /// pattern in [`PatternFill`], expanded into colour. Clipped to
    /// `clip` ∩ the visible area.
    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill, clip: ClipRect);

    /// Blit an 8-wide × 16-tall monochrome glyph with its top-left at
    /// `(x, y)` in colour `fg`. Bit 7 (`0x80`) of each byte is the
    /// leftmost pixel; only set bits are drawn (transparent). Clipped to
    /// `clip` ∩ the visible area.
    fn draw_glyph(&self, x: i16, y: i16, glyph: &[u8; 16], fg: Pel, clip: ClipRect);

    /// Blit a monochrome source ([`MonoSrc`]) into the screen rectangle
    /// `dst`, expanding set bits to `fg` and (when `opaque`) clear bits
    /// to `bg` (`vrt_cpyfm`). Clipped to `clip` ∩ the visible area.
    fn blit_mono(&self, dst: Rect, src: MonoSrc, fg: Pel, bg: Pel, opaque: bool, clip: ClipRect);

    /// Copy a screen rectangle `src` to the screen position `(dx, dy)`
    /// using BitBlt logic op `op` (3 = replace, 6 = XOR) — the
    /// screen-to-screen case of `vro_cpyfm`. The destination write is
    /// clipped to `clip` ∩ the visible area.
    fn copy_rect(&self, src: Rect, dx: i16, dy: i16, op: u8, clip: ClipRect);
}

/// A rectangle in device coordinates (top-left + size).
#[derive(Clone, Copy)]
pub struct Rect {
    /// Left edge.
    pub x: i16,
    /// Top edge.
    pub y: i16,
    /// Width in pixels.
    pub w: i16,
    /// Height in pixels.
    pub h: i16,
}

/// A monochrome blit source (one plane). `bits` is the raster as 16-bit
/// big-endian-packed words, `stride_words` words per row; `(x, y)` is
/// the top-left of the sub-rectangle to read. Bit `0x8000 >> (col & 15)`
/// of word `col/16` is the leftmost pixel.
#[derive(Clone, Copy)]
pub struct MonoSrc<'a> {
    /// The source raster words.
    pub bits: &'a [u16],
    /// Words per raster row.
    pub stride_words: u16,
    /// Source sub-rectangle left.
    pub x: i16,
    /// Source sub-rectangle top.
    pub y: i16,
}

/// A monochrome fill source: a pattern expanded into a foreground /
/// background colour. The `pattern` is a run of 16-bit rows whose length
/// is a power of two; scanline `yy` uses row `pattern[yy & (len-1)]`,
/// pixel `xx` uses bit `0x8000 >> (xx & 0xF)` (patterns are
/// screen-aligned to absolute device coordinates). A set bit writes
/// `fg`; a clear bit writes `bg` when `opaque`, else leaves the pixel
/// unchanged.
#[derive(Clone, Copy)]
pub struct PatternFill<'a> {
    /// Colour for set bits.
    pub fg: Pel,
    /// Colour for clear bits (used only when `opaque`).
    pub bg: Pel,
    /// The pattern rows (length a power of two).
    pub pattern: &'a [u16],
    /// Write `bg` on clear bits (replace) vs. leave them (transparent).
    pub opaque: bool,
}

/// Inclusive clipping rectangle in device coordinates, as carried by the
/// workstation. Passed to line drawing so the backend clips without
/// reaching back into the `Vwk`.
#[derive(Clone, Copy)]
pub struct ClipRect {
    /// Left edge (inclusive).
    pub xmn: i16,
    /// Top edge (inclusive).
    pub ymn: i16,
    /// Right edge (inclusive).
    pub xmx: i16,
    /// Bottom edge (inclusive).
    pub ymx: i16,
}

impl ClipRect {
    /// The clip rectangle covering the whole `w × h` device.
    pub fn full(w: u16, h: u16) -> ClipRect {
        ClipRect {
            xmn: 0,
            ymn: 0,
            xmx: w.saturating_sub(1) as i16,
            ymx: h.saturating_sub(1) as i16,
        }
    }

    /// Intersect with the device bounds `w × h`, returning the inclusive
    /// pixel rectangle `(x0, y0, x1, y1)` to draw into, or `None` if the
    /// result is empty. The backend calls this to honour both the
    /// workstation clip and the screen edges in one step.
    pub fn clamp_to(&self, w: u16, h: u16) -> Option<(i32, i32, i32, i32)> {
        let x0 = (self.xmn as i32).max(0);
        let y0 = (self.ymn as i32).max(0);
        let x1 = (self.xmx as i32).min(w as i32 - 1);
        let y1 = (self.ymx as i32).min(h as i32 - 1);
        if x0 <= x1 && y0 <= y1 {
            Some((x0, y0, x1, y1))
        } else {
            None
        }
    }
}
