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
//! pixels, so the screen probe always sees the original (pre-fill) colour
//! — the pattern write can never fool the flood.
//!
//! The bitmap is sized per call to the live clip ∩ device rectangle (no
//! fixed-resolution assumption), so contourfill works at any framebuffer
//! geometry; a small clip needs only a few bytes, a full 1920×1080 screen
//! ~253 KiB, freed on return. Allocation failure aborts the fill early
//! (nothing drawn), mirroring EmuTOS' queue-overflow abort.
//!
//! ## Colour model
//!
//! The comparison follows the device's colour model, as EmuTOS' `end_pts`
//! dispatches on `TRUECOLOR_MODE` (`vdi_fill.c:836-849`):
//!
//! - **Truecolor** (`v_planes > 8`): compare **16-bit screen words** with
//!   `OVERLAY_BIT` masked out of the screen side (`end_pts16`/
//!   `search_to_right16`/`search_to_left16`, `vdi_fill.c:694-767`). The
//!   masking is what makes a `v_clrwk`-cleared background (`0xFFFF`) and a
//!   pen-0 fill (`0xFFDF`, the pseudo-palette's white) the same colour.
//! - **Palette** (`v_planes <= 8`): compare colour indices, as the planar
//!   `end_pts` does (`vdi_fill.c:851-866`).

use core::cell::UnsafeCell;

use alloc::alloc::{alloc_zeroed, dealloc, Layout};

use super::backend::{PatternFill, RasterBackend, WriteMode};
use super::fill_patterns;
use super::vwk::{self, Vwk};
use super::Vdipb;

/// The Falcon 16-bit overlay bit — bit 5 of the `RRRRRGGGGGOBBBBB` screen
/// word, i.e. the green LSB the VDI pseudo-palette never sets
/// (`vdi_fill.c:26`, `vdi_defs.h:156`, `vdi_col.c:599`).
const OVERLAY_BIT: u16 = 0x0020;

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

    // Under `TRUECOLOR_MODE` (`v_planes > 8`, `vdi_defs.h:22`) the seed
    // fill works on raw 16-bit screen words, not palette indices —
    // `end_pts` dispatches to `end_pts16` (`vdi_fill.c:846-849`).
    let truecolor = backend.planes() > 8;

    // The screen colour at `(x, y)` in the units the comparison uses.
    //
    // Truecolor: the 16-bit word with `OVERLAY_BIT` masked off, which is
    // what `end_pts16` reads (`color = *addr & ~OVERLAY_BIT`,
    // `vdi_fill.c:755`) and what `search_to_right16`/`search_to_left16`
    // compare (`:709,733`). NewTOS' framebuffer stores 32-bpp XRGB8888
    // internally (`docs/COMPATIBILITY.md`), so the app-visible word is
    // recovered through `device_to_rgb565`, the same screen→16-bit boundary
    // `v_get_pixel` and a memory MFDB cross — *not* through `read_pixel`,
    // whose reverse map to the nearest CLUT entry is lossy and would fold
    // every non-pen colour onto a pen.
    //
    // Palette: the colour index, as the planar `end_pts` composes it from
    // the bit-planes (`vdi_fill.c:857`); no overlay bit exists there.
    let probe = |x: i16, y: i16| -> u16 {
        if truecolor {
            backend.device_to_rgb565(backend.get_device_pixel(x, y)) & !OVERLAY_BIT
        } else {
            backend.read_pixel(x, y) as u16
        }
    };

    // Determine the search colour + seed mode (`contourfill`,
    // `vdi_fill.c:968-983`).
    let intin0 = pb.intin(0);
    let (search, seed_type) = if intin0 < 0 {
        // Fill the region equal to the seed pixel: `search_color =
        // pixelread(xleft, oldy)` (`vdi_fill.c:970`). Note EmuTOS takes the
        // **unmasked** word here — `pixelread`'s truecolor branch returns
        // `*addr` as stored (`:677-681`) — and `end_pts16` then compares it
        // against a *masked* screen colour (`:763`). So a seed pixel whose
        // overlay bit is set (a `v_clrwk`-cleared background, `0xFFFF`)
        // never equals any masked screen word and the fill draws nothing.
        // The mask is therefore applied to the screen side only, as EmuTOS
        // does — and the asymmetry is observable on NewTOS as on a Falcon
        // since the kernel keeps `OVERLAY_BIT` in the device pixel's X byte
        // (VDI-OVL-001, closed by V11/V12, 2026-09-21): a seed-mode fill on
        // the cleared background reads `0xFFFF` here and draws nothing.
        let c = if truecolor {
            backend.device_to_rgb565(backend.get_device_pixel(sx, sy))
        } else {
            backend.read_pixel(sx, sy) as u16
        };
        (c, true)
    } else {
        // `>= numcolors` guard (silent no-op, `vdi_fill.c:976`).
        if intin0 as u16 >= backend.num_colors() {
            return;
        }
        // Convert the VDI pen to a device colour: `search_color =
        // MAP_COL[search_color]` and, under truecolor, the physical
        // workstation's pseudo-palette entry — `phys_work.ext->palette[...]`
        // (`vdi_fill.c:978-984`), the *physical* workstation's table, not the
        // calling one's. NewTOS folds `MAP_COL` to the identity
        // (`vwk::VwkExt`), so the pen indexes the pseudo-palette directly.
        let c = if truecolor {
            // SAFETY: single trap path, as for the rest of the VDI state.
            let phys = unsafe { vwk::ext(vwk::PHYS_HANDLE) };
            let Some(word) = phys.and_then(|e| e.palette.get(intin0 as usize).copied()) else {
                return; // pen beyond the pseudo-palette → no such colour
            };
            word
        } else {
            intin0 as u16
        };
        (c, false) // fill region bounded by this colour
    };

    // HOLLOW is not a no-op: EmuTOS `st_fl_ptr` points `patptr` at the
    // all-zero HOLLOW word, so each filled span is drawn through the write
    // mode (Replace → pen 0). The seed/visited logic is independent of the
    // drawn colour, so this still terminates.
    let pattern = fill_patterns::select(vwk);
    let fill = PatternFill {
        fg: vwk.fill_color,
        bg: 0,
        pattern,
        mode: WriteMode::from_wrt_mode(vwk.wrt_mode),
    };

    // Seed predicate on a pixel's original colour — `end_pts`' verdict
    // `(color == search_color) ? seed_type : !seed_type` (`vdi_fill.c:763-766`,
    // `:862-865`): fill-while-same for `seed_type`, else fill-to-boundary.
    let matches = |p: u16| if seed_type { p == search } else { p != search };

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
        if visited.is_set(x, y) || !matches(probe(x, y)) {
            continue;
        }
        // Extend the span left and right over unvisited, matching pixels.
        let mut xl = x;
        while xl > xmn && !visited.is_set(xl - 1, y) && matches(probe(xl - 1, y)) {
            xl -= 1;
        }
        let mut xr = x;
        while xr < xmx && !visited.is_set(xr + 1, y) && matches(probe(xr + 1, y)) {
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
                if !visited.is_set(xx, ny) && matches(probe(xx, ny)) {
                    // Seed once, then skip to the end of this fillable run.
                    if top < SEED_MAX {
                        stack[top] = (xx, ny);
                        top += 1;
                    }
                    while xx <= xr && !visited.is_set(xx, ny) && matches(probe(xx, ny)) {
                        xx += 1;
                    }
                } else {
                    xx += 1;
                }
            }
        }
    }
}
