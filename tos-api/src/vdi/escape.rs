//! VDI `v_escape` (opcode 5) — the VT52-style **alpha** (text-mode)
//! cursor controls and a few inquiries. Spec: `docs/api/vdi/_escape.md`.
//! The sub-function is in `CONTRL[5]`; `> 19` is a no-op.
//!
//! NewTOS drives a self-contained alpha layer directly over the
//! [`RasterBackend`] using the 8×16 system [`font`], with its own cursor
//! row/column and reverse-video state. (This is the VDI's own text layer;
//! GEMDOS console output uses the kernel's framebuffer console.)

use core::sync::atomic::{AtomicBool, AtomicI16, Ordering};

use super::backend::{ClipRect, RasterBackend};
use super::{font, mouse, Vdipb};

// Alpha colours (Atari style: black text on white).
const FG: u8 = 1;
const BG: u8 = 0;

static CUR_X: AtomicI16 = AtomicI16::new(0); // column (0-based)
static CUR_Y: AtomicI16 = AtomicI16::new(0); // row (0-based)
static REVERSE: AtomicBool = AtomicBool::new(false);

fn cols(backend: &dyn RasterBackend) -> i16 {
    backend.width() as i16 / font::CELL_W
}
fn rows(backend: &dyn RasterBackend) -> i16 {
    backend.height() as i16 / font::CELL_H
}
fn full_clip(backend: &dyn RasterBackend) -> ClipRect {
    ClipRect::full(backend.width(), backend.height())
}

/// Draw one glyph in the current (possibly reversed) colours at the cell.
fn put_cell(backend: &dyn RasterBackend, col: i16, row: i16, code: u16) {
    let (x, y) = (col * font::CELL_W, row * font::CELL_H);
    let clip = full_clip(backend);
    let (fg, bg) = if REVERSE.load(Ordering::Relaxed) { (BG, FG) } else { (FG, BG) };
    backend.fill_rect(x, y, font::CELL_W, font::CELL_H, bg, clip);
    backend.draw_glyph(x, y, font::glyph(code), fg, clip);
}

/// Erase a run of cells on `row` from `col0` to `col1` (inclusive).
fn erase_cells(backend: &dyn RasterBackend, row: i16, col0: i16, col1: i16) {
    if col0 > col1 {
        return;
    }
    let clip = full_clip(backend);
    backend.fill_rect(
        col0 * font::CELL_W,
        row * font::CELL_H,
        (col1 - col0 + 1) * font::CELL_W,
        font::CELL_H,
        BG,
        clip,
    );
}

/// Opcode 5 — `v_escape`: dispatch on `CONTRL[5]`.
pub fn v_escape(pb: &Vdipb, backend: &dyn RasterBackend) {
    let (mx, my) = (cols(backend) - 1, rows(backend) - 1); // max col/row
    match pb.contrl(5) {
        1 => {
            // vq_chcells: rows, columns
            pb.set_intout(0, my + 1);
            pb.set_intout(1, mx + 1);
            pb.set_contrl(4, 2);
        }
        2 => {} // v_exit_cur: enter graphics mode (nothing to tear down)
        3 => {
            // v_enter_cur: enter alpha mode — clear screen, home cursor.
            backend.fill_rect(0, 0, backend.width() as i16, backend.height() as i16, BG, full_clip(backend));
            CUR_X.store(0, Ordering::Relaxed);
            CUR_Y.store(0, Ordering::Relaxed);
            REVERSE.store(false, Ordering::Relaxed);
        }
        4 => fetch_update(&CUR_Y, |y| (y - 1).max(0)),          // up
        5 => fetch_update(&CUR_Y, |y| (y + 1).min(my)),         // down
        6 => fetch_update(&CUR_X, |x| (x + 1).min(mx)),         // right
        7 => fetch_update(&CUR_X, |x| (x - 1).max(0)),          // left
        8 => {
            CUR_X.store(0, Ordering::Relaxed); // home
            CUR_Y.store(0, Ordering::Relaxed);
        }
        9 => {
            // v_eeos: erase cursor → end of screen.
            let (cx, cy) = (CUR_X.load(Ordering::Relaxed), CUR_Y.load(Ordering::Relaxed));
            erase_cells(backend, cy, cx, mx);
            for r in (cy + 1)..=my {
                erase_cells(backend, r, 0, mx);
            }
        }
        10 => {
            // v_eeol: erase cursor → end of line.
            let (cx, cy) = (CUR_X.load(Ordering::Relaxed), CUR_Y.load(Ordering::Relaxed));
            erase_cells(backend, cy, cx, mx);
        }
        11 => {
            // vs_curaddress: INTIN[0]=row, INTIN[1]=col (1-based).
            CUR_Y.store((pb.intin(0) - 1).clamp(0, my), Ordering::Relaxed);
            CUR_X.store((pb.intin(1) - 1).clamp(0, mx), Ordering::Relaxed);
        }
        12 => {
            // v_curtext: output CONTRL[3] chars at the cursor.
            let n = pb.contrl(3).max(0);
            for i in 0..n as usize {
                let mut x = CUR_X.load(Ordering::Relaxed);
                let mut y = CUR_Y.load(Ordering::Relaxed);
                put_cell(backend, x, y, pb.intin(i) as u16);
                x += 1;
                if x > mx {
                    x = 0;
                    y = (y + 1).min(my);
                }
                CUR_X.store(x, Ordering::Relaxed);
                CUR_Y.store(y, Ordering::Relaxed);
            }
        }
        13 => REVERSE.store(true, Ordering::Relaxed),  // v_rvon
        14 => REVERSE.store(false, Ordering::Relaxed), // v_rvoff
        15 => {
            // vq_curaddress: row, column (1-based).
            pb.set_intout(0, CUR_Y.load(Ordering::Relaxed) + 1);
            pb.set_intout(1, CUR_X.load(Ordering::Relaxed) + 1);
            pb.set_contrl(4, 2);
        }
        16 => {
            // vq_tabstatus: no tablet device.
            pb.set_intout(0, 0);
            pb.set_contrl(4, 1);
        }
        17 => {} // v_hardcopy: no printer device → documented no-op
        18 => mouse::display_at(pb.ptsin(0), pb.ptsin(1), backend), // v_dspcur
        19 => mouse::remove(backend),                               // v_rmcur
        _ => {} // > 19 → no-op
    }
}

/// Read-modify-write a cursor coordinate.
fn fetch_update(cell: &AtomicI16, f: impl Fn(i16) -> i16) {
    let v = cell.load(Ordering::Relaxed);
    cell.store(f(v), Ordering::Relaxed);
}
