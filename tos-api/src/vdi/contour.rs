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

use core::cell::UnsafeCell;

use super::backend::{PatternFill, RasterBackend};
use super::fill_patterns;
use super::vwk::Vwk;
use super::Vdipb;

const WM_REPLACE: i16 = 0;

/// Bounded seed stack. Sized for screen-area fills; an overflow simply
/// truncates the fill rather than recursing unboundedly.
const SEED_MAX: usize = 4096;

struct SeedStack(UnsafeCell<[(i16, i16); SEED_MAX]>);
// SAFETY: accessed only from the single-CPU, IRQ-off VDI trap path, like
// the rest of the VDI mutable state (see `vwk::TABLE`).
unsafe impl Sync for SeedStack {}
static SEEDS: SeedStack = SeedStack(UnsafeCell::new([(0, 0); SEED_MAX]));

/// Opcode 103 — `v_contourfill`.
pub fn v_contourfill(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let sx = pb.ptsin(0);
    let sy = pb.ptsin(1);
    let clip = vwk.clip_rect();
    if sx < clip.xmn || sx > clip.xmx || sy < clip.ymn || sy > clip.ymx {
        return;
    }

    // Determine the search colour + seed mode.
    let intin0 = pb.intin(0);
    let (search, seed_type) = if intin0 < 0 {
        (backend.read_pixel(sx, sy), true) // fill region equal to seed
    } else {
        let ncol = backend.num_colors();
        if intin0 as u16 >= ncol {
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

    // A pixel is fillable if it matches the seed predicate and has not
    // already been painted (`!= fill_color`, which also bounds the
    // boundary mode against the just-filled span).
    let fillable = |x: i16, y: i16| -> bool {
        let p = backend.read_pixel(x, y);
        if p == vwk.fill_color {
            return false;
        }
        if seed_type {
            p == search
        } else {
            p != search
        }
    };

    // SAFETY: single trap path (see `SeedStack`).
    let stack = unsafe { &mut *SEEDS.0.get() };
    let mut top = 0usize;
    stack[top] = (sx, sy);
    top += 1;

    while top > 0 {
        top -= 1;
        let (x, y) = stack[top];
        if !fillable(x, y) {
            continue;
        }
        // Extend the span left and right within the clip rectangle.
        let mut xl = x;
        while xl > clip.xmn && fillable(xl - 1, y) {
            xl -= 1;
        }
        let mut xr = x;
        while xr < clip.xmx && fillable(xr + 1, y) {
            xr += 1;
        }
        backend.fill_pattern(xl, y, xr - xl + 1, 1, fill, clip);

        // Seed the runs on the scanlines above and below.
        for &ny in &[y - 1, y + 1] {
            if ny < clip.ymn || ny > clip.ymx {
                continue;
            }
            let mut xx = xl;
            while xx <= xr {
                if fillable(xx, ny) {
                    // Skip to the end of this fillable run, seeding once.
                    if top < SEED_MAX {
                        stack[top] = (xx, ny);
                        top += 1;
                    }
                    while xx <= xr && fillable(xx, ny) {
                        xx += 1;
                    }
                } else {
                    xx += 1;
                }
            }
        }
    }
}
