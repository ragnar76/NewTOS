//! A VDI primitive showcase: the 16 GEM palette colours, the dither and
//! hatch fill styles, the 8×16 system font, and a mono sprite blit +
//! screen copy — every pixel drawn **through the VDI** (`svc #2`). Kept
//! as a manual smoke test of the Phase-12 primitives; the boot screen is
//! [`crate::initinfo`].

use crate::kprintln;
use crate::vdi_client::{Mfdb, Vdi};

const BLACK: i16 = 1;

/// Draw the VDI primitive showcase.
pub fn run() {
    let (w, h) = crate::vdi_backend::dims();
    if w == 0 || h == 0 {
        return; // no framebuffer
    }
    let (w, h) = (w as i16, h as i16);

    let mut v = Vdi::new();
    if !v.open() {
        kprintln!("[vdi-demo] no workstation — skipping");
        return;
    }

    v.clear(); // white background

    // ── 16 palette tiles: 8 columns × 2 rows ──
    const MARGIN: i16 = 8;
    const COLS: i16 = 8;
    const ROWS: i16 = 2;
    const TILE_H: i16 = 48;
    let tile_w = (w - 2 * MARGIN) / COLS;
    let block_w = tile_w * COLS;
    let block_h = TILE_H * ROWS;

    for row in 0..ROWS {
        for col in 0..COLS {
            let color = row * COLS + col;
            let x = MARGIN + col * tile_w;
            let y = MARGIN + row * TILE_H;
            v.set_fill(color);
            v.fill_rect(x, y, x + tile_w - 1, y + TILE_H - 1);
        }
    }

    // ── black grid lines delineating the tiles (so the white tile shows) ──
    v.set_line(BLACK);
    for col in 0..=COLS {
        let x = MARGIN + col * tile_w;
        v.line(x, MARGIN, x, MARGIN + block_h);
    }
    for row in 0..=ROWS {
        let y = MARGIN + row * TILE_H;
        v.line(MARGIN, y, MARGIN + block_w, y);
    }

    // ── fill-style showcase (D4): a row of the 8 dither patterns and a
    //    row of the 6 hatch styles, each outlined so its bounds show. ──
    const FIS_PATTERN: i16 = 2;
    const FIS_HATCH: i16 = 3;
    let p_h: i16 = 64;
    let dy = MARGIN + block_h + 20;
    let hy = dy + p_h + 12;

    for i in 0..8i16 {
        let x = MARGIN + i * tile_w;
        v.set_fill_color(4); // blue
        v.set_fill_style(FIS_PATTERN);
        v.set_fill_index(i + 1);
        v.fill_rect(x, dy, x + tile_w - 1, dy + p_h - 1);
        outline(&mut v, x, dy, x + tile_w - 1, dy + p_h - 1);
    }

    let hatch_w = (w - 2 * MARGIN) / 6;
    for i in 0..6i16 {
        let x = MARGIN + i * hatch_w;
        v.set_fill_color(BLACK);
        v.set_fill_style(FIS_HATCH);
        v.set_fill_index(i + 1);
        v.fill_rect(x, hy, x + hatch_w - 1, hy + p_h - 1);
        outline(&mut v, x, hy, x + hatch_w - 1, hy + p_h - 1);
    }

    // ── text (D5): title + the system-font character set ──
    v.set_text_align(0, 5); // left, top
    let ty = hy + p_h + 24;
    v.set_text_color(BLACK);
    v.gtext(MARGIN, ty, b"NewTOS VDI - Phase 12 (D1-D6) - 8x16 ST system font");
    v.set_text_color(4); // blue
    v.gtext(MARGIN, ty + 24, b"ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz");
    v.set_text_color(2); // red
    v.gtext(MARGIN, ty + 42, b"0123456789  !\"#$%&'()*+,-./:;<=>?@[\\]^_{|}~");

    // ── raster blit (D6): a mono "ball" sprite in colours via vrt_cpyfm,
    //    then the row duplicated by a vro_cpyfm screen-to-screen copy. ──
    let ball: [u16; 16] = [
        0x03C0, 0x0FF0, 0x1FF8, 0x3FFC, 0x7FFE, 0x7FFE, 0xFFFF, 0xFFFF,
        0xFFFF, 0xFFFF, 0x7FFE, 0x7FFE, 0x3FFC, 0x1FF8, 0x0FF0, 0x03C0,
    ];
    let mfdb = Mfdb {
        addr: ball.as_ptr() as u64,
        w: 16,
        h: 16,
        wdwidth: 1,
        stand: 0,
        nplanes: 1,
        r1: 0,
        r2: 0,
        r3: 0,
    };
    let mfdb_ptr = &mfdb as *const Mfdb as u64;
    let by = ty + 64;
    for i in 0..8i16 {
        let bx = MARGIN + i * 40;
        v.blit_mono(mfdb_ptr, 16, 16, bx, by, (i % 7) + 2, 0, 1); // colours 2..8, transparent
    }
    v.copy_screen(MARGIN, by, 8 * 40, 16, MARGIN, by + 24, 3);

    // ── outer screen border ──
    v.set_line(BLACK);
    v.line(0, 0, w - 1, 0);
    v.line(w - 1, 0, w - 1, h - 1);
    v.line(w - 1, h - 1, 0, h - 1);
    v.line(0, h - 1, 0, 0);

    v.close();
    kprintln!("[vdi-demo] full primitive showcase drawn through the VDI ({}x{})", w, h);
}

/// Draw a black rectangle outline (four `v_pline` segments).
fn outline(v: &mut Vdi, x0: i16, y0: i16, x1: i16, y1: i16) {
    v.set_line(BLACK);
    v.line(x0, y0, x1, y0);
    v.line(x1, y0, x1, y1);
    v.line(x1, y1, x0, y1);
    v.line(x0, y1, x0, y0);
}
