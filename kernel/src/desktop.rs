//! NewTOS Desktop — Phase 14, Batch 1: the desktop *screen*.
//!
//! Draws the GEM desktop over the AES: a grey desktop background pattern,
//! the menu bar (Desk / File / View / Options), and a drive icon per
//! present drive (from `Drvmap`). Static for now — the interactive parts
//! (drop-down menus, opening a directory window on double-click, launching
//! a program) land in Batch 2+. The desktop is a kernel module that drives
//! the AES directly; mouse interaction works in QEMU (real-hardware mouse
//! waits on the IKBD/UART2 bring-up).

use tos_api::aes::object::Object;
use tos_api::vdi::backend::{ClipRect, PatternFill, RasterBackend};

use crate::kprintln;

const G_IBOX: u16 = 25;
const G_BOX: u16 = 20;
const G_TITLE: u16 = 32;
const LASTOB: u16 = 0x0020;

const WHITE: u8 = 0;
const BLACK: u8 = 1;
const GRAY_LT: u8 = 8;
const GRAY_DK: u8 = 9;

const MENU_H: i16 = 19;

/// Issue a GEM trap (selector 200, AESPB in `x1`).
fn aes(aespb: u64) {
    // SAFETY: GEM trap; results go into the parameter block.
    unsafe {
        core::arch::asm!(
            "svc #2",
            inout("x0") 200u64 => _,
            in("x1") aespb,
            options(nostack),
        );
    }
}

/// Draw the full desktop screen.
pub fn draw() {
    let Some(backend) = tos_api::host::host().and_then(|h| h.raster()) else {
        return;
    };
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;
    let clip = ClipRect::full(backend.width(), backend.height());

    // 1. Desktop background: a 50 % grey dither below the menu bar.
    let pat: [u16; 2] = [0xAAAA, 0x5555];
    let bg = PatternFill { fg: BLACK, bg: WHITE, pattern: &pat, opaque: true };
    backend.fill_pattern(0, MENU_H, sw, sh - MENU_H, bg, clip);

    // 2. Menu bar.
    draw_menu_bar();

    // 3. A drive icon per present drive.
    let bits = tos_api::bios::drive::drvbits();
    let mut iy = 48;
    for d in 0..26u32 {
        if bits & (1 << d) != 0 {
            draw_drive_icon(backend, sw - 72, iy, b'A' + d as u8, clip);
            iy += 72;
        }
    }

    kprintln!("[desk] desktop drawn ({} drives)", bits.count_ones());
}

/// Build a minimal canonical menu tree (ROOT/THEBAR/THEACTIVE + titles) and
/// install it with `menu_bar`. Drop-downs arrive in Batch 2.
fn draw_menu_bar() {
    static DESK: &[u8] = b"Desk\0";
    static FILE: &[u8] = b"File\0";
    static VIEW: &[u8] = b"View\0";
    static OPTS: &[u8] = b"Options\0";

    let t = |ty, fl, spec: u64, x, y, w, h, next, head, tail| Object {
        ob_next: next, ob_head: head, ob_tail: tail, ob_type: ty, ob_flags: fl,
        ob_state: 0, ob_spec: spec, ob_x: x, ob_y: y, ob_width: w, ob_height: h,
    };
    let title = |s: &[u8], x, w, next| t(G_TITLE, 0, s.as_ptr() as u64, x, 0, w, MENU_H, next, -1, -1);

    let tree = [
        /* 0 ROOT     */ t(G_IBOX, 0, 0, 0, 0, 640, 480, -1, 1, 1),
        /* 1 THEBAR   */ t(G_BOX, 0, 0, 0, 0, 640, MENU_H, 0, 2, 2),
        /* 2 THEACTIVE*/ t(G_IBOX, 0, 0, 0, 0, 640, MENU_H, 1, 3, 6),
        /* 3 Desk     */ title(DESK, 0, 56, 4),
        /* 4 File     */ title(FILE, 56, 56, 5),
        /* 5 View     */ title(VIEW, 112, 56, 6),
        /* 6 Options  */ t(G_TITLE, LASTOB, OPTS.as_ptr() as u64, 168, 0, 80, MENU_H, 2, -1, -1),
    ];

    let mut control = [0i16; 8];
    let mut global = [0i16; 32];
    let mut intin = [0i16; 16];
    let mut intout = [0i16; 16];
    let mut addrin = [0i64; 4];
    let mut addrout = [0i64; 4];
    let block: [u64; 6] = [
        control.as_mut_ptr() as u64, global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64, intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64, addrout.as_mut_ptr() as u64,
    ];
    control[0] = 30; control[1] = 1; control[3] = 1; // menu_bar
    intin[0] = 1; addrin[0] = tree.as_ptr() as i64;
    aes(block.as_ptr() as u64);
    let _ = (&mut global, &mut intout, &mut addrout);
}

/// Draw a hard-disk drive icon (a 3D body + drive letter) with a label
/// below it, centred at `(cx, y)`.
fn draw_drive_icon(backend: &dyn RasterBackend, cx: i16, y: i16, letter: u8, clip: ClipRect) {
    // Disk body: a 40×30 raised grey box with a darker face + a bay slot.
    let (bw, bh) = (40i16, 30i16);
    let bx = cx - bw / 2;
    backend.fill_rect(bx, y, bw, bh, GRAY_LT, clip);
    bevel(backend, bx, y, bw, bh, true, clip);
    // drive bay: a recessed inner rectangle
    backend.fill_rect(bx + 5, y + 6, bw - 10, bh - 12, WHITE, clip);
    bevel(backend, bx + 5, y + 6, bw - 10, bh - 12, false, clip);
    // a slot line + the drive letter
    backend.fill_rect(bx + 8, y + 10, bw - 16, 2, GRAY_DK, clip);
    backend.draw_glyph(cx - 4, y + 14, tos_api::vdi::font::glyph(letter as u16), BLACK, clip);

    // Label "X:" centred below the icon.
    let label = [letter, b':'];
    let lx = cx - (label.len() as i16 * 8) / 2;
    for (i, &c) in label.iter().enumerate() {
        backend.draw_glyph(lx + i as i16 * 8, y + bh + 3, tos_api::vdi::font::glyph(c as u16), BLACK, clip);
    }
}

fn bevel(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, raised: bool, clip: ClipRect) {
    if w < 2 || h < 2 {
        return;
    }
    let (tl, br) = if raised { (WHITE, GRAY_DK) } else { (GRAY_DK, WHITE) };
    backend.fill_rect(x, y, w, 1, tl, clip);
    backend.fill_rect(x, y, 1, h, tl, clip);
    backend.fill_rect(x, y + h - 1, w, 1, br, clip);
    backend.fill_rect(x + w - 1, y, 1, h, br, clip);
}
