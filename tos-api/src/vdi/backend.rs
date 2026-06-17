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

    /// Fill the rectangle `[x, x+w) × [y, y+h)` (device coordinates,
    /// origin top-left) with palette colour `pel`. Implementations clip
    /// to the visible area; negative / oversized rectangles are clamped.
    fn fill_rect(&self, x: i16, y: i16, w: i16, h: i16, pel: Pel);

    /// Draw a one-pixel-wide line from `(x0, y0)` to `(x1, y1)` in
    /// palette colour `pel`, clipped to `[xmn, ymn]..=[xmx, ymx]`
    /// (inclusive device coordinates — the caller passes the workstation
    /// clip rectangle). Endpoints are inclusive.
    fn draw_line(&self, x0: i16, y0: i16, x1: i16, y1: i16, pel: Pel, clip: ClipRect);
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
