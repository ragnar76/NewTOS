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

    // 1. Desktop background: a light dither below the menu bar (stand-in
    //    for EmuTOS' g_patcol desktop pattern+colour; the exact VDI fill
    //    pattern for ob_spec 0x43 lands with the DROOT G_IBOX in Batch 2).
    let pat: [u16; 4] = [0x8888, 0x0000, 0x2222, 0x0000];
    let bg = PatternFill { fg: BLACK, bg: WHITE, pattern: &pat, opaque: true };
    backend.fill_pattern(0, MENU_H, sw, sh - MENU_H, bg, clip);

    // 2. Menu bar.
    draw_menu_bar();

    // 3. A drive icon per present drive, laid out in a grid from the
    //    TOP-LEFT (EmuTOS app_blddesk / the #M loop: drive_x = idx % xcnt,
    //    drive_y = idx / xcnt). C and up are hard disks.
    let bits = tos_api::bios::drive::drvbits();
    let icw = 80i16;
    let ich = 64i16;
    let xcnt = ((sw - 20) / icw).max(1);
    let mut idx = 0i16;
    for d in 0..26u32 {
        if bits & (1 << d) != 0 {
            let cx = 20 + (idx % xcnt) * icw + icw / 2;
            let cy = MENU_H + 18 + (idx / xcnt) * ich;
            draw_drive_icon(backend, cx, cy, b'A' + d as u8, clip);
            idx += 1;
        }
    }

    kprintln!("[desk] desktop drawn ({} drives)", bits.count_ones());
}

/// Build a minimal canonical menu tree (ROOT/THEBAR/THEACTIVE + titles) and
/// install it with `menu_bar`. Drop-downs arrive in Batch 2.
fn draw_menu_bar() {
    // Title text matches EmuTOS desktop.rsc: surrounding spaces.
    static DESK: &[u8] = b" Desk \0";
    static FILE: &[u8] = b" File \0";
    static VIEW: &[u8] = b" View \0";
    static OPTS: &[u8] = b" Options \0";

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

/// Draw a hard-disk drive icon (EmuTOS `IG_HARD`: a drive enclosure with
/// the drive letter on it) and the "Disk" name below, centred at `(cx,
/// y)`. A faithful ICONBLK from icon.rsc is Batch 2; this is a
/// recognizable stand-in built from the same elements (enclosure +
/// `ib_char` letter + `ib_ptext` label).
fn draw_drive_icon(backend: &dyn RasterBackend, cx: i16, y: i16, letter: u8, clip: ClipRect) {
    // Enclosure: a 44×26 raised grey box (the drive unit).
    let (bw, bh) = (44i16, 26i16);
    let bx = cx - bw / 2;
    backend.fill_rect(bx, y, bw, bh, GRAY_LT, clip);
    bevel(backend, bx, y, bw, bh, true, clip);
    // Front panel: a recessed bay, with a slot and an activity LED.
    backend.fill_rect(bx + 4, y + 4, bw - 8, bh - 9, WHITE, clip);
    bevel(backend, bx + 4, y + 4, bw - 8, bh - 9, false, clip);
    backend.fill_rect(bx + 7, y + 8, bw - 22, 2, GRAY_DK, clip); // slot
    backend.fill_rect(bx + bw - 9, y + 8, 3, 3, BLACK, clip); // LED
    // The drive letter on the panel.
    backend.draw_glyph(bx + 8, y + 6, tos_api::vdi::font::glyph(letter as u16), BLACK, clip);

    // Name label ("Disk", EmuTOS STDISK) centred below the icon.
    let label = b"Disk";
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
