//! A small VDI demo (Phase 12 D3): the 16 GEM palette colours as filled
//! tiles, delineated and overlaid with a line grid and a screen border —
//! every pixel drawn **through the VDI** (`svc #2`), exercising
//! `v_opnvwk` / `vsf_color` / `vsf_interior` / `vr_recfl` / `vsl_color` /
//! `v_pline`. This replaces the old hard-coded boot test pattern; it is
//! the final on-screen image after boot.

use crate::kprintln;

const BLACK: i16 = 1;
const SOLID: i16 = 1;

/// A VDI caller: the five parameter-block arrays plus the open handle.
/// Each call rebuilds the 5-pointer block from the (stable) array
/// addresses and issues `svc #2`.
struct Vdi {
    contrl: [i16; 12],
    intin: [i16; 16],
    ptsin: [i16; 32],
    intout: [i16; 45],
    ptsout: [i16; 12],
    handle: i16,
}

impl Vdi {
    fn new() -> Self {
        Self {
            contrl: [0; 12],
            intin: [0; 16],
            ptsin: [0; 32],
            intout: [0; 45],
            ptsout: [0; 12],
            handle: 0,
        }
    }

    /// Issue the current parameter block as a VDI call (`svc #2`).
    fn call(&mut self) {
        let block: [u64; 5] = [
            self.contrl.as_mut_ptr() as u64,
            self.intin.as_mut_ptr() as u64,
            self.ptsin.as_mut_ptr() as u64,
            self.intout.as_mut_ptr() as u64,
            self.ptsout.as_mut_ptr() as u64,
        ];
        let pb = block.as_ptr() as u64;
        // SAFETY: svc #2 with a valid VDI parameter block in x0.
        unsafe {
            core::arch::asm!("svc #2", inout("x0") pb => _, options(nostack));
        }
    }

    /// Open a virtual workstation; returns false if no handle/backend.
    fn open(&mut self) -> bool {
        self.contrl.fill(0);
        self.contrl[0] = 100; // v_opnvwk
        self.contrl[3] = 11;
        self.intin[..11].copy_from_slice(&[1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 2]);
        self.call();
        self.handle = self.contrl[6];
        self.handle != 0
    }

    fn close(&mut self) {
        self.contrl.fill(0);
        self.contrl[0] = 101; // v_clsvwk
        self.contrl[6] = self.handle;
        self.call();
    }

    fn clear(&mut self) {
        self.contrl.fill(0);
        self.contrl[0] = 3; // v_clrwk
        self.contrl[6] = self.handle;
        self.call();
    }

    /// Set an attribute opcode taking a single `INTIN[0]` value.
    fn set1(&mut self, opcode: i16, value: i16) {
        self.contrl.fill(0);
        self.contrl[0] = opcode;
        self.contrl[3] = 1;
        self.contrl[6] = self.handle;
        self.intin[0] = value;
        self.call();
    }

    fn set_fill_color(&mut self, color: i16) {
        self.set1(25, color); // vsf_color
    }
    fn set_fill_style(&mut self, style: i16) {
        self.set1(23, style); // vsf_interior
    }
    fn set_fill_index(&mut self, index: i16) {
        self.set1(24, index); // vsf_style
    }

    /// Solid fill in `color`.
    fn set_fill(&mut self, color: i16) {
        self.set_fill_color(color);
        self.set_fill_style(SOLID);
    }

    fn fill_rect(&mut self, x1: i16, y1: i16, x2: i16, y2: i16) {
        self.contrl.fill(0);
        self.contrl[0] = 114; // vr_recfl
        self.contrl[1] = 2;
        self.contrl[6] = self.handle;
        self.ptsin[..4].copy_from_slice(&[x1, y1, x2, y2]);
        self.call();
    }

    fn set_line(&mut self, color: i16) {
        self.contrl.fill(0);
        self.contrl[0] = 17; // vsl_color
        self.contrl[3] = 1;
        self.contrl[6] = self.handle;
        self.intin[0] = color;
        self.call();
    }

    fn line(&mut self, x0: i16, y0: i16, x1: i16, y1: i16) {
        self.contrl.fill(0);
        self.contrl[0] = 6; // v_pline
        self.contrl[1] = 2;
        self.contrl[6] = self.handle;
        self.ptsin[..4].copy_from_slice(&[x0, y0, x1, y1]);
        self.call();
    }
}

/// Draw the VDI demo as the final on-screen image.
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

    // dither patterns 1..=8 in blue
    for i in 0..8i16 {
        let x = MARGIN + i * tile_w;
        v.set_fill_color(4); // blue
        v.set_fill_style(FIS_PATTERN);
        v.set_fill_index(i + 1);
        v.fill_rect(x, dy, x + tile_w - 1, dy + p_h - 1);
        outline(&mut v, x, dy, x + tile_w - 1, dy + p_h - 1);
    }

    // hatch styles 1..=6 in black
    let hatch_w = (w - 2 * MARGIN) / 6;
    for i in 0..6i16 {
        let x = MARGIN + i * hatch_w;
        v.set_fill_color(BLACK);
        v.set_fill_style(FIS_HATCH);
        v.set_fill_index(i + 1);
        v.fill_rect(x, hy, x + hatch_w - 1, hy + p_h - 1);
        outline(&mut v, x, hy, x + hatch_w - 1, hy + p_h - 1);
    }

    // ── outer screen border ──
    v.set_line(BLACK);
    v.line(0, 0, w - 1, 0);
    v.line(w - 1, 0, w - 1, h - 1);
    v.line(w - 1, h - 1, 0, h - 1);
    v.line(0, h - 1, 0, 0);

    v.close();
    kprintln!("[vdi-demo] palette + patterns + hatches drawn through the VDI ({}x{})", w, h);
}

/// Draw a black rectangle outline (four `v_pline` segments).
fn outline(v: &mut Vdi, x0: i16, y0: i16, x1: i16, y1: i16) {
    v.set_line(BLACK);
    v.line(x0, y0, x1, y0);
    v.line(x1, y0, x1, y1);
    v.line(x1, y1, x0, y1);
    v.line(x0, y1, x0, y0);
}
