//! Framebuffer HAL trait.
//!
//! A linear 32-bit (XRGB8888) framebuffer, board-independent. Concrete
//! implementations:
//!   * `drivers::qemu_virt::ramfb` — QEMU `ramfb` via fw_cfg (the dev
//!     vehicle; verified headless with QMP `screendump`).
//!   * `drivers::bcm2711` — VideoCore mailbox framebuffer (RPi4; HW
//!     bring-up later).
//!
//! The TOS graphics stack (VDI / Line-A, later phases) and a future
//! on-screen VT52 console sit on top of this; for now it carries a
//! boot test pattern.

/// Pack an opaque (X=0xFF) RGB colour into XRGB8888.
#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// A linear XRGB8888 framebuffer.
///
/// # Safety
///
/// Implementations return a raw mutable pointer to the pixel store;
/// callers must respect [`Framebuffer::width`] / [`Framebuffer::height`]
/// / [`Framebuffer::stride_px`] bounds.
pub trait Framebuffer: Sync {
    /// Visible width in pixels.
    fn width(&self) -> u32;
    /// Visible height in pixels.
    fn height(&self) -> u32;
    /// Row stride in **pixels** (`stride_bytes / 4`).
    fn stride_px(&self) -> u32;
    /// Base pointer to the pixel store (XRGB8888, one `u32` per pixel),
    /// addressed through the kernel's normal (high-half) mapping.
    fn pixels(&self) -> *mut u32;

    /// Set one pixel. Out-of-bounds coordinates are ignored.
    fn put_pixel(&self, x: u32, y: u32, color: u32) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let off = (y * self.stride_px() + x) as usize;
        // SAFETY: bounds checked above; `pixels()` is valid for
        // `height * stride_px` u32 writes.
        unsafe { self.pixels().add(off).write_volatile(color) };
    }

    /// Fill a rectangle (clipped to the visible area).
    fn fill_rect(&self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let x1 = (x + w).min(self.width());
        let y1 = (y + h).min(self.height());
        let stride = self.stride_px();
        let base = self.pixels();
        for yy in y..y1 {
            let row = (yy * stride) as usize;
            for xx in x..x1 {
                // SAFETY: xx<width, yy<height ⇒ in-bounds.
                unsafe { base.add(row + xx as usize).write_volatile(color) };
            }
        }
    }

    /// Fill the whole framebuffer with one colour.
    fn clear(&self, color: u32) {
        self.fill_rect(0, 0, self.width(), self.height(), color);
    }
}
