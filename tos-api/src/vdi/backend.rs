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

/// The VDI writing mode (`wrt_mode`), selecting how a primitive's
/// foreground pixels combine with what is already on screen. Mapped from
/// the **0-based** `wrt_mode` the `Vwk` stores (`vswr_mode` stores
/// `INTIN[0] - 1`; EmuTOS `WM_REPLACE`/`WM_TRANS`/`WM_XOR`/`WM_ERASE` =
/// 0/1/2/3). Spec: `docs/api/vdi/attributes/_mode-clip-color.md`.
///
/// A primitive presents each candidate pixel with a boolean "source bit":
/// for a filled span it is the interior-pattern bit, for a dashed line it
/// is the `LN_MASK` bit, for a glyph it is the font bit. The mode decides
/// what (if anything) is written:
///
/// | mode | source **set** | source **clear** |
/// |------|----------------|------------------|
/// | `Replace`        | foreground | background |
/// | `Transparent`    | foreground | untouched  |
/// | `Xor`            | complement | untouched  |
/// | `RevTransparent` | untouched  | foreground |
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WriteMode {
    /// `WM_REPLACE` (0, the default): foreground on set bits, background
    /// on clear bits — opaque, so it also clears the cell/gap.
    Replace,
    /// `WM_TRANS` (1): foreground on set bits; clear bits untouched.
    Transparent,
    /// `WM_XOR` (2): complement the destination on set bits; clear bits
    /// untouched. Self-inverse (drawing twice restores the original).
    Xor,
    /// `WM_ERASE` (3), reverse-transparent: foreground on **clear** bits;
    /// set bits untouched — the inverse of [`Transparent`](Self::Transparent).
    RevTransparent,
}

impl WriteMode {
    /// Map the **0-based** `wrt_mode` stored in the [`Vwk`](crate::vdi::vwk::Vwk)
    /// to a [`WriteMode`]. Out-of-range values fall back to
    /// [`Replace`](Self::Replace) (`DEF_WRT_MODE`), matching how
    /// `vswr_mode` clamps an out-of-range `INTIN[0]`.
    #[inline]
    pub fn from_wrt_mode(wrt_mode: i16) -> WriteMode {
        match wrt_mode {
            1 => WriteMode::Transparent,
            2 => WriteMode::Xor,
            3 => WriteMode::RevTransparent,
            _ => WriteMode::Replace,
        }
    }
}

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

    /// Number of simultaneously displayable colours (the VDI pen count as
    /// advertised in `DEV_TAB[13]`; EmuTOS `numcolors`). The Falcon
    /// truecolor profile reports 256 (`vdi_control.c:206`).
    fn num_colors(&self) -> u16;

    /// Number of bit-planes the VDI advertises as `INQ_TAB[4] = v_planes`
    /// (`vdi_control.c:210`). **Decoupled from** [`num_colors`](Self::num_colors):
    /// under Falcon truecolor `v_planes == 16` while the pen count
    /// (`DEV_TAB[13]`) is 256 — the two must not be conflated
    /// (`vdi/vdi_defs.h:22`, `TRUECOLOR_MODE (v_planes > 8)`; they only
    /// coincide in the palette modes, 16 colours = 4 planes). This feeds
    /// `DEV_TAB[35]` (`v_planes == 1`), `INQ_TAB[4]`, and — since the AES
    /// reads `ap_planes`/`gl_nplanes` straight from `INQ_TAB[4]`
    /// (`gemgsxif.c:497-501`) — the AES plane count too.
    fn planes(&self) -> u16;

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
    /// and rotated, matching the VDI `LN_MASK`; a set mask bit is the
    /// primitive's "source set". `mode` selects the raster combination:
    /// `Replace` also paints the background on the dash gaps, `Xor`
    /// complements the dash-on pixels, `RevTransparent` paints the gaps.
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
    );

    /// Fill the rectangle `[x, x+w) × [y, y+h)` with the monochrome
    /// pattern in [`PatternFill`], expanded into colour per its
    /// [`WriteMode`](PatternFill::mode). Clipped to `clip` ∩ the visible
    /// area.
    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill, clip: ClipRect);

    /// Blit an 8-wide × 16-tall monochrome glyph with its top-left at
    /// `(x, y)` in colour `fg`. Bit 7 (`0x80`) of each byte is the
    /// leftmost pixel; the font bit is the primitive's "source set".
    /// `mode` selects the raster combination — `Transparent` draws only
    /// set bits (the common text case), `Replace` also clears the cell
    /// background (VDI background pen 0), `Xor` complements, and
    /// `RevTransparent` draws the clear bits. Clipped to `clip` ∩ the
    /// visible area.
    fn draw_glyph(&self, x: i16, y: i16, glyph: &[u8; 16], fg: Pel, mode: WriteMode, clip: ClipRect);

    /// Blit a monochrome source ([`MonoSrc`]) into the screen rectangle
    /// `dst`, expanding it to colour per the VDI [`WriteMode`]
    /// (`vrt_cpyfm`'s `MD_*` codes). A source bit is the primitive's
    /// "source set"; the mode decides what lands on set/clear bits:
    ///
    /// | mode | source **set** (1) | source **clear** (0) |
    /// |------|--------------------|----------------------|
    /// | `Replace`        | `fg` | `bg` |
    /// | `Transparent`    | `fg` | untouched |
    /// | `Xor`            | complement | untouched |
    /// | `RevTransparent` | untouched | `bg` |
    ///
    /// `RevTransparent` maps EmuTOS' `MD_ERASE`: the 0-bits take the
    /// **background** colour (`INTIN[2]`), the 1-bits are left alone.
    /// Clipped to `clip` ∩ the visible area.
    fn blit_mono(&self, dst: Rect, src: MonoSrc, fg: Pel, bg: Pel, mode: WriteMode, clip: ClipRect);

    /// Copy a screen rectangle `src` to the screen position `(dx, dy)`
    /// applying one of the 16 BitBlt logic ops `op` (`D' = op(S, D)`, the
    /// `vro_cpyfm` `INTIN[0]` table with `PAT_FLAG` already stripped): e.g.
    /// `0`=`0`, `3`=`S` (replace), `5`=`D` (no-op), `6`=`S ^ D` (XOR),
    /// `7`=`S | D` (OR), `15`=`~0`. The op acts literally on the packed
    /// pixel word — the screen-to-screen case of `vro_cpyfm`. The
    /// destination write is clipped to `clip` ∩ the visible area.
    fn copy_rect(&self, src: Rect, dx: i16, dy: i16, op: u8, clip: ClipRect);

    /// Read the **raw device pixel word** (the packed value actually stored
    /// in the framebuffer — XRGB8888 for the bring-up truecolor screen) at
    /// device coordinate `(x, y)`. Out-of-bounds → `0`.
    ///
    /// Unlike [`read_pixel`](Self::read_pixel), which reverse-maps to the
    /// nearest CLUT index (lossy), this returns the exact stored colour.
    /// It is the screen side of a memory-MFDB copy (`vro_cpyfm`
    /// Screen→Memory, i.e. AES `bb_save`): the saved bitmap must reproduce
    /// the screen bit-for-bit on restore, which a PEL round-trip cannot
    /// guarantee for non-palette content.
    fn get_device_pixel(&self, x: i16, y: i16) -> u32;

    /// Write the **raw device pixel word** at `(x, y)`; out-of-bounds is
    /// ignored. Writes made this way are not flushed to the point of
    /// coherency — the caller batches a region and calls
    /// [`flush_rect`](Self::flush_rect) once, so the display DMA (a
    /// separate master beyond the CPU caches on real hardware) sees them.
    /// This is the screen side of `vro_cpyfm` Memory→Screen (AES
    /// `bb_restore`).
    fn put_device_pixel(&self, x: i16, y: i16, argb: u32);

    /// Flush (clean to the point of coherency) the inclusive device
    /// rectangle `[x0..=x1] × [y0..=y1]`, so writes made via
    /// [`put_device_pixel`](Self::put_device_pixel) become visible to the
    /// display controller. Coordinates are clamped to the screen; a no-op
    /// on a Non-Cacheable framebuffer.
    fn flush_rect(&self, x0: i16, y0: i16, x1: i16, y1: i16);
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

/// A monochrome fill source: a pattern expanded into colour per a
/// [`WriteMode`]. The `pattern` is a run of 16-bit rows whose length is a
/// power of two; scanline `yy` uses row `pattern[yy & (len-1)]`, pixel
/// `xx` uses bit `0x8000 >> (xx & 0xF)` (patterns are screen-aligned to
/// absolute device coordinates). The pattern bit is the primitive's
/// "source set"; [`mode`](Self::mode) decides what is written for set and
/// clear bits (see [`WriteMode`]).
#[derive(Clone, Copy)]
pub struct PatternFill<'a> {
    /// Colour for the foreground pixels (see [`WriteMode`]).
    pub fg: Pel,
    /// Colour for the background pixels (used only by
    /// [`WriteMode::Replace`]).
    pub bg: Pel,
    /// The pattern rows (length a power of two).
    pub pattern: &'a [u16],
    /// The raster combination applied to the pattern.
    pub mode: WriteMode,
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
