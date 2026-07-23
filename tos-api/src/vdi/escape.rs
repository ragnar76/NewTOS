//! VDI `v_escape` (opcode 5) — the VT52-style **alpha** (text-mode)
//! cursor controls and a few inquiries. Spec: `docs/api/vdi/_escape.md`.
//! The sub-function is in `CONTRL[5]`; `> 19` is a no-op.
//!
//! NewTOS drives a **self-contained** alpha layer directly over the
//! [`RasterBackend`] using the 8×16 system [`font`], with its own cursor
//! row/column and reverse-video state.
//!
//! This is a deliberate architecture deviation from EmuTOS (AUD-V3-011):
//! EmuTOS routes every alpha escape through the *single shared* BIOS VT-52
//! console (`vdi_esc.c`, `bios_conws`/`crawio`), so its alpha cursor,
//! reverse-video flag and scroll are the same state GEMDOS/BIOS console
//! output uses. NewTOS keeps the two apart — the VDI alpha layer here and
//! the kernel's framebuffer console for GEMDOS/BIOS. The alpha cursor is
//! **logical only** (position tracking); no visible block cursor is drawn,
//! so EmuTOS' VT-52 cursor hide/show sequences (`ESC f`/`ESC e`, emitted by
//! `v_exit_cur`/`v_enter_cur`) have no rendered counterpart here. Building
//! the shared BIOS-console redirect is a large, separate change and is left
//! as this documented deviation; the escape fixes below all operate cleanly
//! on the private [`RasterBackend`] layer.

use core::sync::atomic::{AtomicBool, AtomicI16, Ordering};

use super::backend::{ClipRect, RasterBackend, Rect, WriteMode};
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
    // The cell background is painted above, so the glyph blits transparently.
    backend.draw_glyph(x, y, font::glyph(code), fg, WriteMode::Transparent, clip);
}

/// Clear the whole alpha screen and home the cursor — the effect shared by
/// EmuTOS' `v_exit_cur` (`\033H\033f\033E`) and `v_enter_cur`
/// (`\033E\033e\015`), both of which clear-and-home via VT-52 `ESC E`
/// (`vdi_esc.c:81,93`). Reverse video is **not** reset: neither sequence
/// sends `ESC q`.
fn clear_and_home(backend: &dyn RasterBackend) {
    backend.fill_rect(
        0,
        0,
        backend.width() as i16,
        backend.height() as i16,
        BG,
        full_clip(backend),
    );
    CUR_X.store(0, Ordering::Relaxed);
    CUR_Y.store(0, Ordering::Relaxed);
}

/// Scroll the whole alpha text area up by one cell row — the EmuTOS VT-52
/// console behaviour when `v_curtext` output runs past the bottom of the
/// screen (`vdi_esc.c:196`, `crawio` → BIOS conout, which scrolls). The top
/// row is lost; the freed bottom row is cleared to the background. `mx`/`my`
/// are the maximum (0-based) column/row.
fn scroll_up(backend: &dyn RasterBackend, mx: i16, my: i16) {
    let clip = full_clip(backend);
    // Move rows 1..=my up onto rows 0..=my-1. op 3 = `S` (replace/copy);
    // `copy_rect` picks a non-corrupting direction for the overlap.
    let src = Rect {
        x: 0,
        y: font::CELL_H,
        w: (mx + 1) * font::CELL_W,
        h: my * font::CELL_H,
    };
    backend.copy_rect(src, 0, 0, 3, clip);
    // Clear the freed bottom row.
    erase_cells(backend, my, 0, mx);
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
        2 => {
            // v_exit_cur: EmuTOS `escfn2` sends `\033H\033f\033E` = home, hide
            // alpha cursor, clear-and-home (`vdi_esc.c:81`). Net rendered
            // effect: clear the text area + home the cursor. The cursor-hide
            // (`ESC f`) has no counterpart — the private alpha layer draws no
            // visible block cursor (see module docs, AUD-V3-011).
            clear_and_home(backend);
        }
        3 => {
            // v_enter_cur: EmuTOS `escfn3` sends `\033E\033e\015` = clear-and-
            // home, show alpha cursor, CR (`vdi_esc.c:93`). There is **no**
            // `ESC q`, so reverse video is preserved (AUD-V3-012). Net rendered
            // effect: clear the text area + home; the cursor-show (`ESC e`) and
            // the CR (redundant after home) have no counterpart here.
            clear_and_home(backend);
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
            // v_curtext: output CONTRL[3] chars at the cursor. EmuTOS
            // `escfn12` feeds each char to `crawio` → the VT-52 console, which
            // scrolls the screen up once output passes the bottom line
            // (`vdi_esc.c:196`). We reproduce that scroll here (AUD-V3-005).
            let n = pb.contrl(3).max(0);
            for i in 0..n as usize {
                let mut x = CUR_X.load(Ordering::Relaxed);
                let mut y = CUR_Y.load(Ordering::Relaxed);
                put_cell(backend, x, y, pb.intin(i) as u16);
                x += 1;
                if x > mx {
                    x = 0;
                    y += 1;
                    if y > my {
                        // Past the bottom line: scroll up and stay on the last
                        // (now cleared) row instead of overwriting it.
                        scroll_up(backend, mx, my);
                        y = my;
                    }
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
            // vq_tabstatus: a mouse/tablet device IS available — NewTOS has a
            // USB-HID mouse. EmuTOS escfn16 unconditionally returns 1
            // (`vdi_esc.c:248-252`); the function name is deceptive.
            pb.set_intout(0, 1);
            pb.set_contrl(4, 1);
        }
        17 => {} // v_hardcopy: no printer device → documented no-op
        // v_dspcur: EmuTOS escfn18 ignores the PTSIN coordinates and force-shows
        // the cursor at its current position (v_show_c with INTIN[0]=0).
        18 => mouse::force_show(backend),
        // v_rmcur: EmuTOS escfn19 = vdi_v_hide_c → increments the hide count
        // (not an absolute hide), so it balances against a matching show.
        19 => mouse::v_hide_c(backend),
        _ => {} // > 19 → no-op
    }
}

/// Read-modify-write a cursor coordinate.
fn fetch_update(cell: &AtomicI16, f: impl Fn(i16) -> i16) {
    let v = cell.load(Ordering::Relaxed);
    cell.store(f(v), Ordering::Relaxed);
}
