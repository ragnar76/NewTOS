//! NewTOS Desktop — Phase 14, Batch 1: the desktop *screen*.
//!
//! Draws the GEM desktop over the AES: a grey desktop background pattern,
//! the menu bar (Desk / File / View / Options), and a drive icon per
//! present drive (from `Drvmap`). Static for now — the interactive parts
//! (drop-down menus, opening a directory window on double-click, launching
//! a program) land in Batch 2+. The desktop is a kernel module that drives
//! the AES directly; mouse interaction works in QEMU (real-hardware mouse
//! waits on the IKBD/UART2 bring-up).

use core::cell::UnsafeCell;

use tos_api::aes::object::Object;
use tos_api::vdi::backend::{ClipRect, PatternFill, RasterBackend};

use crate::kprintln;

const G_IBOX: u16 = 25;
const G_BOX: u16 = 20;
const G_STRING: u16 = 28;
const G_TITLE: u16 = 32;
const LASTOB: u16 = 0x0020;
const DISABLED: u16 = 0x0008;

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

    // The desktop background (pattern + drive icons) is painted by a hook
    // the window manager calls on every redraw_all, so it persists under
    // windows instead of being whited out (Bug 1). Register it, then paint
    // the first frame + the menu bar.
    tos_api::aes::window::set_desktop_painter(paint_desktop_bg);
    paint_desktop_bg(backend);
    draw_menu_bar();

    let bits = tos_api::bios::drive::drvbits();
    kprintln!("[desk] desktop drawn ({} drives)", bits.count_ones());
}

/// Paint the desktop background — the pattern fill below the menu bar plus
/// the drive icons — straight through the backend. **No AES trap**: the
/// window manager calls this from inside `redraw_all` (the AES SVC
/// handler), so it must not re-enter the AES. The menu bar (y < MENU_H) is
/// deliberately left untouched: windows never cover it, so it survives.
fn paint_desktop_bg(backend: &dyn RasterBackend) {
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;
    let clip = ClipRect::full(backend.width(), backend.height());

    // Background: a light dither (stand-in for EmuTOS' g_patcol desktop
    // pattern+colour; the exact VDI fill for ob_spec 0x43 lands with the
    // DROOT G_IBOX later).
    let pat: [u16; 4] = [0x8888, 0x0000, 0x2222, 0x0000];
    let bg = PatternFill { fg: BLACK, bg: WHITE, pattern: &pat, opaque: true };
    backend.fill_pattern(0, MENU_H, sw, sh - MENU_H, bg, clip);

    // A drive icon per present drive, in a grid from the TOP-LEFT (EmuTOS
    // app_blddesk / #M loop: drive_x = idx % xcnt, drive_y = idx / xcnt).
    let bits = tos_api::bios::drive::drvbits();
    let icw = 80i16;
    let ich = 64i16;
    let xcnt = ((sw - 20) / icw).max(1);
    let mut idx = 0i16;
    for d in 0..26u32 {
        if bits & (1 << d) != 0 {
            let cx = 20 + (idx % xcnt) * icw + icw / 2;
            let cy = MENU_H + 18 + (idx / xcnt) * ich;
            // EmuTOS deskapp.c:721 — A/B (drive < 2) are floppies, C+ hard.
            draw_drive_icon(backend, cx, cy, b'A' + d as u8, d > 1, clip);
            idx += 1;
        }
    }
}

// The desktop menu (EmuTOS desktop.rsc ADMENU). NUL-terminated; a string
// of dashes is a disabled separator.
const MENUS: [(&[u8], &[&[u8]]); 4] = [
    (b" Desk \0", &[b" Desktop info... \0", b"-----------------\0"]),
    (
        b" File \0",
        &[
            b" Open             \0", b" Info/Rename...   \0", b" Search...        \0",
            b"------------------\0", b" New folder...    \0", b" Close folder     \0",
            b" Close top window \0", b" Cycle windows    \0", b" Select all items \0",
            b" Set file mask... \0", b"------------------\0", b" Delete...        \0",
            b" Format floppy... \0", b" Eject floppy     \0", b"------------------\0",
            b" Execute EmuCON   \0", b" Shutdown         \0",
        ],
    ),
    (
        b" View \0",
        &[
            b" Show as icons \0", b" Show as text  \0", b"---------------\0",
            b" Sort by name  \0", b" Sort by type  \0", b" Sort by size  \0",
            b" Sort by date  \0", b" No sort       \0", b"---------------\0",
            b" Size to fit   \0",
        ],
    ),
    (
        b" Options \0",
        &[
            b" Set background...       \0", b" Install icon...         \0",
            b" Install application...  \0", b" Install devices         \0",
            b" Remove desktop icon     \0", b"-------------------------\0",
            b" Set preferences...      \0", b" Desktop configuration...\0",
            b" Change resolution...    \0", b"-------------------------\0",
            b" Read .INF file...       \0", b" Save desktop...         \0",
            b"-------------------------\0", b" Blitter                 \0",
            b" Cache                   \0",
        ],
    ),
];

const EMPTY_OBJ: Object = Object {
    ob_next: -1, ob_head: -1, ob_tail: -1, ob_type: 0, ob_flags: 0,
    ob_state: 0, ob_spec: 0, ob_x: 0, ob_y: 0, ob_width: 0, ob_height: 0,
};

/// The persistent menu tree (the AES keeps a pointer to it for interactive
/// drop-down tracking). Single boot CPU only.
struct MenuTree {
    obj: UnsafeCell<[Object; 96]>,
    built: UnsafeCell<bool>,
}
unsafe impl Sync for MenuTree {}
static MENU: MenuTree = MenuTree {
    obj: UnsafeCell::new([EMPTY_OBJ; 96]),
    built: UnsafeCell::new(false),
};

/// Build the canonical ADMENU tree (ROOT / THEBAR / THEACTIVE / titles +
/// THESCREEN box of drop-downs) into the persistent static. Returns the
/// root pointer.
fn build_menu() -> *mut Object {
    // SAFETY: single-CPU AES; built once.
    unsafe {
        let o = &mut *MENU.obj.get();
        if *MENU.built.get() {
            return o.as_mut_ptr();
        }

        let nt = MENUS.len();
        let first_title = 3usize;
        let themenus = first_title + nt; // = 7
        // ROOT / THEBAR / THEACTIVE
        o[0] = mk(G_IBOX, 0, 0, 0, 0, 640, 480, -1, 1, themenus as i16);
        o[1] = mk(G_BOX, 0, 0, 0, 0, 640, MENU_H, themenus as i16, 2, 2);
        o[2] = mk(G_IBOX, 0, 0, 0, 0, 640, MENU_H, 1, first_title as i16, (themenus - 1) as i16);

        // Titles (left to right) and, after `themenus`, one box per title.
        let mut tx = 0i16;
        let mut k = themenus + 1; // first box index
        // first pass needs box indices; lay out boxes+items sequentially.
        let mut box_idx = [0usize; 8];
        // titles
        for (ti, (name, _)) in MENUS.iter().enumerate() {
            let tw = (cstr_len(name) as i16) * 8 + 4;
            let last = ti == nt - 1;
            o[first_title + ti] = mk(
                G_TITLE, if last { LASTOB } else { 0 }, name.as_ptr() as u64,
                tx, 0, tw, MENU_H,
                if last { 2 } else { (first_title + ti + 1) as i16 }, -1, -1,
            );
            tx += tw;
        }
        // themenus box
        let themenus_first_child = k;
        // boxes + their items
        for (ti, (_name, items)) in MENUS.iter().enumerate() {
            box_idx[ti] = k;
            let bx_x = title_x(ti);
            let iw = items.iter().map(|s| cstr_len(s) as i16).max().unwrap_or(8) * 8 + 4;
            let bh = items.len() as i16 * MENU_H;
            let bi = k;
            k += 1;
            let first_item = k;
            for (ii, it) in items.iter().enumerate() {
                let is_sep = it.first() == Some(&b'-');
                let last_item = ii == items.len() - 1;
                let last_overall = (ti == nt - 1) && last_item;
                let mut fl = if last_overall { LASTOB } else { 0 };
                if is_sep {
                    fl |= DISABLED;
                }
                o[k] = mk(
                    G_STRING, fl, it.as_ptr() as u64, 0, ii as i16 * MENU_H, iw, MENU_H,
                    if last_item { bi as i16 } else { (k + 1) as i16 }, -1, -1,
                );
                k += 1;
            }
            // the box itself
            let next_box = if ti == nt - 1 { themenus as i16 } else { (k) as i16 };
            o[bi] = mk(
                G_BOX, 0, 1 << 16, bx_x, MENU_H, iw, bh, next_box, first_item as i16, (k - 1) as i16,
            );
        }
        let last_box = box_idx[nt - 1];
        o[themenus] = mk(G_IBOX, 0, 0, 0, 0, 640, 480, 0, themenus_first_child as i16, last_box as i16);

        *MENU.built.get() = true;
        o.as_mut_ptr()
    }
}

/// X position of title `ti` (accumulated title widths).
fn title_x(ti: usize) -> i16 {
    let mut x = 0i16;
    for (name, _) in MENUS.iter().take(ti) {
        x += (cstr_len(name) as i16) * 8 + 4;
    }
    x
}

fn cstr_len(s: &[u8]) -> usize {
    s.iter().position(|&b| b == 0).unwrap_or(s.len())
}

#[allow(clippy::too_many_arguments)]
fn mk(ty: u16, fl: u16, spec: u64, x: i16, y: i16, w: i16, h: i16, next: i16, head: i16, tail: i16) -> Object {
    Object {
        ob_next: next, ob_head: head, ob_tail: tail, ob_type: ty, ob_flags: fl,
        ob_state: 0, ob_spec: spec, ob_x: x, ob_y: y, ob_width: w, ob_height: h,
    }
}

/// Verification preview: draw the desktop and open a `C:\` directory
/// window (the response a double-click on the C drive will trigger).
pub fn preview_dir() {
    draw();
    open_dir_window(b'C');
}

/// The built menu tree root (for tests / the interactive loop).
///
/// # Safety
/// The returned pointer aliases the persistent menu static.
pub unsafe fn menu_tree_ptr() -> *mut Object {
    build_menu()
}

/// Install the desktop menu bar via `menu_bar`.
fn draw_menu_bar() {
    let tree = build_menu();
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
    intin[0] = 1; addrin[0] = tree as i64;
    aes(block.as_ptr() as u64);
}

/// Draw a drive icon (EmuTOS `IG_HARD` for `hard`, else `IG_FLOPPY`) with
/// the drive letter on it (`ib_char`) and the "Disk" name below
/// (`ib_ptext`/STDISK), centred at `(cx, y)`. A faithful ICONBLK from
/// icon.rsc is Batch 2; this is a recognizable stand-in from the same
/// elements.
fn draw_drive_icon(backend: &dyn RasterBackend, cx: i16, y: i16, letter: u8, hard: bool, clip: ClipRect) {
    let (bw, bh) = if hard { (44i16, 26i16) } else { (30i16, 32i16) };
    let bx = cx - bw / 2;
    backend.fill_rect(bx, y, bw, bh, GRAY_LT, clip);
    bevel(backend, bx, y, bw, bh, true, clip);
    if hard {
        // Hard disk: a wide enclosure with a recessed bay, slot + LED.
        backend.fill_rect(bx + 4, y + 4, bw - 8, bh - 9, WHITE, clip);
        bevel(backend, bx + 4, y + 4, bw - 8, bh - 9, false, clip);
        backend.fill_rect(bx + 7, y + 8, bw - 22, 2, GRAY_DK, clip); // slot
        backend.fill_rect(bx + bw - 9, y + 8, 3, 3, BLACK, clip); // LED
        backend.draw_glyph(bx + 8, y + 6, tos_api::vdi::font::glyph(letter as u16), BLACK, clip);
    } else {
        // Floppy: a square with the metal shutter at top + a label area.
        backend.fill_rect(bx + 9, y + 2, 12, 6, GRAY_DK, clip); // shutter
        backend.fill_rect(bx + 4, y + 12, bw - 8, bh - 16, WHITE, clip); // label
        bevel(backend, bx + 4, y + 12, bw - 8, bh - 16, false, clip);
        backend.draw_glyph(bx + (bw - 8) / 2, y + 13, tos_api::vdi::font::glyph(letter as u16), BLACK, clip);
    }

    // Name label ("Disk", EmuTOS STDISK) centred below the icon.
    let label = b"Disk";
    let lx = cx - (label.len() as i16 * 8) / 2;
    for (i, &c) in label.iter().enumerate() {
        backend.draw_glyph(lx + i as i16 * 8, y + bh + 3, tos_api::vdi::font::glyph(c as u16), BLACK, clip);
    }
}

/// Window border `kind` bits (aesdefs.h:519).
const W_KIND: i16 =
    (0x0001 | 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0040 | 0x0080 | 0x0100) as i16;
// NAME | CLOSER | MOVER | INFO | SIZER | UPARROW | DNARROW | VSLIDE

/// Open a directory window for drive `letter` and draw its file listing —
/// the desktop's response to opening a drive (EmuTOS `do_diropen`). Returns
/// the window handle, or 0 on failure.
fn open_dir_window(letter: u8) -> i16 {
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

    // wind_create(kind, full extent = whole desktop below the menu).
    control = [0; 8]; control[0] = 100; control[1] = 5;
    intin[0] = W_KIND; intin[1] = 0; intin[2] = MENU_H; intin[3] = 640; intin[4] = 480 - MENU_H;
    aes(block.as_ptr() as u64);
    let handle = intout[0];
    if handle < 1 {
        return 0;
    }

    // wind_set(WF_NAME, "X:\").
    let mut name = [letter, b':', b'\\', 0, 0, 0, 0, 0];
    control = [0; 8]; control[0] = 105; control[1] = 2; control[3] = 1;
    intin[0] = handle; intin[1] = 2; addrin[0] = name.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);

    // wind_open(handle, rect) — a window covering the left half.
    let (ox, oy, ow, oh) = (40i16, MENU_H + 24, 360i16, 300i16);
    control = [0; 8]; control[0] = 101; control[1] = 5;
    intin[0] = handle; intin[1] = ox; intin[2] = oy; intin[3] = ow; intin[4] = oh;
    aes(block.as_ptr() as u64);

    // wind_calc(WC_WORK, kind, border-rect) → the work area. `wind_calc` is
    // a *pure* conversion — no window-table lookup — so it sidesteps the
    // `&'static mut` aliasing flakiness that `wind_get(WF_WXYWH)` exhibited
    // in release builds (the window table hands out aliasing `&mut`).
    control = [0; 8]; control[0] = 108; control[1] = 6;
    intin[0] = 1; intin[1] = W_KIND; intin[2] = ox; intin[3] = oy; intin[4] = ow; intin[5] = oh;
    aes(block.as_ptr() as u64);
    let work = (intout[1], intout[2], intout[3], intout[4]);

    draw_dir_listing(handle, letter, work);
    handle
}

/// List `X:\*.*` and draw the file names into the window's `work` rect.
fn draw_dir_listing(_handle: i16, letter: u8, work: (i16, i16, i16, i16)) {
    let Some(backend) = tos_api::host::host().and_then(|h| h.raster()) else {
        return;
    };
    let clip = ClipRect::full(backend.width(), backend.height());
    let (wx, wy, ww, wh) = work;
    backend.fill_rect(wx, wy, ww, wh, WHITE, clip);

    // list_dir(X:\*.*)
    let path = [letter, b':', b'\\', b'*', b'.', b'*'];
    let path_str = core::str::from_utf8(&path).unwrap_or("C:\\*.*");
    let mut names = [[0u8; 14]; 64];
    let mut attrs = [0u8; 64];
    let n = tos_api::bdos::fs::find::list_dir(path_str, &mut names, &mut attrs);

    let rows = (wh / 16).max(1) as usize;
    for (i, nm) in names.iter().enumerate().take(n.min(rows)) {
        let end = nm.iter().position(|&b| b == 0).unwrap_or(nm.len());
        let mut cx = wx + 8;
        for &c in &nm[..end] {
            backend.draw_glyph(cx, wy + 4 + i as i16 * 16, tos_api::vdi::font::glyph(c as u16), BLACK, clip);
            cx += 8;
        }
    }
    kprintln!("[desk] opened {}:\\ ({} entries)", letter as char, n);
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
