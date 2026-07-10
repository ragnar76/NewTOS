//! VDI seed flood-fill — `v_contourfill` (opcode 103). Spec:
//! `docs/api/vdi/output/_fill.md`.
//!
//! Scanline flood fill from a seed pixel, bounded to the workstation clip
//! rectangle, using the current interior fill pattern. Two modes
//! (EmuTOS `seed_type`):
//! - `INTIN[0] < 0`: fill the connected region **equal** to the seed
//!   colour (`seed_type = 1`).
//! - `INTIN[0] >= 0`: fill the connected region **not equal** to that
//!   boundary colour (`seed_type = 0`), i.e. flood until the boundary.
//!
//! ## Termination (why this can never hang, for any pattern)
//!
//! EmuTOS' `contourfill` (`vdi_fill.c:948`) is a Smith span seed-fill: it
//! terminates through a **seed-collision queue**, *not* through the pixel
//! colour — a solid fill in boundary mode leaves the filled pixels still
//! "fillable" by `end_pts`, yet it still terminates because each span is
//! enqueued at most once (opposite-direction seeds annihilate).
//!
//! The previous NewTOS port instead used `pixel == fill_color` as the
//! "already visited" marker. That works for a **solid** pattern (every
//! filled pixel becomes `fill_color`), but for a non-solid pattern
//! (`FIS_PATTERN`/`FIS_HATCH`/`FIS_USER`) the clear-bit pixels never take
//! `fill_color`, so they stay fillable forever and neighbouring rows
//! re-seed them endlessly — an unbreakable hang (the VDI trap runs
//! IRQ-off, so the whole OS freezes).
//!
//! This implementation makes the "visited" state **independent of the
//! fill pattern**: a [`VisitedBitmap`] over the clipped fill region
//! records exactly the pixels already filled. A pixel is fillable only if
//! it is *unvisited* and matches the seed predicate; every filled pixel is
//! marked visited in the same step. Because the visited set grows
//! monotonically and is bounded by the (finite) clipped device area, the
//! fill visits each pixel at most once and is **guaranteed to terminate
//! for every pattern**. The predicate is only ever evaluated on unvisited
//! pixels, so `read_pixel` always sees the original (pre-fill) colour —
//! the pattern write can never fool the flood.
//!
//! The bitmap is sized per call to the live clip ∩ device rectangle (no
//! fixed-resolution assumption), so contourfill works at any framebuffer
//! geometry; a small clip needs only a few bytes, a full 1920×1080 screen
//! ~253 KiB, freed on return. Allocation failure aborts the fill early
//! (nothing drawn), mirroring EmuTOS' queue-overflow abort.

use core::cell::UnsafeCell;

use alloc::alloc::{alloc_zeroed, dealloc, Layout};

use super::backend::{PatternFill, RasterBackend};
use super::fill_patterns;
use super::vwk::Vwk;
use super::Vdipb;

const WM_REPLACE: i16 = 0;

/// Bounded seed stack. Sized for screen-area fills; an overflow simply
/// truncates the fill rather than recursing unboundedly (like EmuTOS'
/// queue-overflow abort). Duplicate seeds for the same span are harmless:
/// once a span is filled it is marked visited, so a later duplicate is
/// popped and skipped. This is a recursion-depth cap, independent of the
/// framebuffer resolution.
const SEED_MAX: usize = 4096;

struct SeedStack(UnsafeCell<[(i16, i16); SEED_MAX]>);
// SAFETY: accessed only from the single-CPU, IRQ-off VDI trap path, like
// the rest of the VDI mutable state (see `vwk::TABLE`).
unsafe impl Sync for SeedStack {}
static SEEDS: SeedStack = SeedStack(UnsafeCell::new([(0, 0); SEED_MAX]));

/// A per-call "already filled" bitmap over the clipped fill rectangle
/// `[x0, x0+w) × [y0, y0+h)`, allocated from the kernel heap. One bit per
/// device pixel, addressed **relative to the clip origin** — so the size
/// tracks the live clip ∩ device geometry, with no fixed-resolution
/// assumption. Dropped (freed) when the fill returns.
struct VisitedBitmap {
    words: *mut u64,
    nwords: usize,
    /// Row stride in bits (= clip width).
    stride: usize,
    x0: i16,
    y0: i16,
}

impl VisitedBitmap {
    /// Allocate a zeroed bitmap covering `w × h` pixels with origin
    /// `(x0, y0)`, or `None` on overflow / allocation failure.
    fn new(x0: i16, y0: i16, w: usize, h: usize) -> Option<Self> {
        let nbits = w.checked_mul(h)?;
        let nwords = nbits.div_ceil(64).max(1);
        let layout = Layout::array::<u64>(nwords).ok()?;
        // SAFETY: `nwords >= 1`, so the layout has non-zero size, as the
        // global allocator requires.
        let words = unsafe { alloc_zeroed(layout) } as *mut u64;
        if words.is_null() {
            return None; // heap exhausted → abort the fill (nothing drawn)
        }
        Some(Self { words, nwords, stride: w, x0, y0 })
    }

    /// Bit `(word_index, mask)` for device pixel `(x, y)`. Callers only
    /// pass coordinates inside the bitmap's rectangle, so the word index
    /// is `< nwords`.
    #[inline]
    fn locate(&self, x: i16, y: i16) -> (usize, u64) {
        let bit = (y - self.y0) as usize * self.stride + (x - self.x0) as usize;
        (bit / 64, 1u64 << (bit & 63))
    }

    #[inline]
    fn is_set(&self, x: i16, y: i16) -> bool {
        let (w, m) = self.locate(x, y);
        // SAFETY: `w < nwords` for in-rectangle `(x, y)` (see `new`).
        unsafe { *self.words.add(w) & m != 0 }
    }

    #[inline]
    fn set(&self, x: i16, y: i16) {
        let (w, m) = self.locate(x, y);
        // SAFETY: `w < nwords` for in-rectangle `(x, y)`; single trap path,
        // so this interior write is not concurrent.
        unsafe { *self.words.add(w) |= m }
    }
}

impl Drop for VisitedBitmap {
    fn drop(&mut self) {
        // `Layout::array` succeeded in `new` with the same `nwords`.
        if let Ok(layout) = Layout::array::<u64>(self.nwords) {
            // SAFETY: `words` came from `alloc_zeroed` with this layout and
            // is freed exactly once (owned by `self`).
            unsafe { dealloc(self.words as *mut u8, layout) }
        }
    }
}

/// Opcode 103 — `v_contourfill`.
pub fn v_contourfill(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let sx = pb.ptsin(0);
    let sy = pb.ptsin(1);

    // Effective flood bounds: the workstation clip rectangle intersected
    // with the **live** device geometry (`backend.width()/height()`) — no
    // fixed-resolution assumption. Both must hold for a pixel to be
    // touched, which keeps every visited-bit access inside the bitmap and
    // guarantees termination over the finite clipped area.
    let clip = vwk.clip_rect();
    let xmn = clip.xmn.max(0);
    let ymn = clip.ymn.max(0);
    let xmx = clip.xmx.min(backend.width().saturating_sub(1) as i16);
    let ymx = clip.ymx.min(backend.height().saturating_sub(1) as i16);
    if xmn > xmx || ymn > ymx {
        return;
    }
    // Seed-in-clip guard (silent no-op, `vdi_fill.c:965`).
    if sx < xmn || sx > xmx || sy < ymn || sy > ymx {
        return;
    }

    // Determine the search colour + seed mode.
    let intin0 = pb.intin(0);
    let (search, seed_type) = if intin0 < 0 {
        (backend.read_pixel(sx, sy), true) // fill region equal to seed
    } else {
        // `>= numcolors` guard (silent no-op, `vdi_fill.c:976`).
        if intin0 as u16 >= backend.num_colors() {
            return;
        }
        (intin0 as u8, false) // fill region bounded by this colour
    };

    let pattern = match fill_patterns::select(vwk) {
        Some(p) => p,
        None => return, // hollow → nothing to fill
    };
    let fill = PatternFill {
        fg: vwk.fill_color,
        bg: 0,
        pattern,
        opaque: vwk.wrt_mode == WM_REPLACE,
    };

    // Seed predicate on a pixel's original colour: fill-while-same for
    // `seed_type`, else fill-to-boundary.
    let matches = |p: u8| if seed_type { p == search } else { p != search };

    // Visited bitmap sized to the clipped fill rectangle. Allocation
    // failure aborts the fill (nothing drawn), like EmuTOS' queue overflow.
    let w = (xmx - xmn + 1) as usize;
    let h = (ymx - ymn + 1) as usize;
    let visited = match VisitedBitmap::new(xmn, ymn, w, h) {
        Some(v) => v,
        None => return,
    };

    // SAFETY: single trap path (see `SeedStack`).
    let stack = unsafe { &mut *SEEDS.0.get() };
    let mut top = 0usize;
    stack[top] = (sx, sy);
    top += 1;

    while top > 0 {
        top -= 1;
        let (x, y) = stack[top];
        // Skip already-filled or non-matching seeds (harmless duplicates).
        if visited.is_set(x, y) || !matches(backend.read_pixel(x, y)) {
            continue;
        }
        // Extend the span left and right over unvisited, matching pixels.
        let mut xl = x;
        while xl > xmn && !visited.is_set(xl - 1, y) && matches(backend.read_pixel(xl - 1, y)) {
            xl -= 1;
        }
        let mut xr = x;
        while xr < xmx && !visited.is_set(xr + 1, y) && matches(backend.read_pixel(xr + 1, y)) {
            xr += 1;
        }
        // Paint the span with the interior pattern, then record it visited
        // so nothing re-seeds into it (this is the pattern-independent
        // "visited" marker that guarantees termination).
        backend.fill_pattern(xl, y, xr - xl + 1, 1, fill, clip);
        for xx in xl..=xr {
            visited.set(xx, y);
        }

        // Seed the runs on the scanlines above and below.
        for ny in [y - 1, y + 1] {
            if ny < ymn || ny > ymx {
                continue;
            }
            let mut xx = xl;
            while xx <= xr {
                if !visited.is_set(xx, ny) && matches(backend.read_pixel(xx, ny)) {
                    // Seed once, then skip to the end of this fillable run.
                    if top < SEED_MAX {
                        stack[top] = (xx, ny);
                        top += 1;
                    }
                    while xx <= xr && !visited.is_set(xx, ny) && matches(backend.read_pixel(xx, ny)) {
                        xx += 1;
                    }
                } else {
                    xx += 1;
                }
            }
        }
    }
}
