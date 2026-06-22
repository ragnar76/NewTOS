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

use tos_api::aes::object::{Object, TedInfo};
use tos_api::vdi::backend::{ClipRect, PatternFill, RasterBackend};

use crate::kprintln;

const G_IBOX: u16 = 25;
const G_BOX: u16 = 20;
const G_BOXTEXT: u16 = 22;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;
const G_TITLE: u16 = 32;
// ob_flags.
const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const EDITABLE: u16 = 0x0008;
const LASTOB: u16 = 0x0020;
const DISABLED: u16 = 0x0008;
const FL3DACT: u16 = 0x0600;
const FL3DBAK: u16 = 0x0400;

const WHITE: u8 = 0;
const BLACK: u8 = 1;

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

/// Issue a GEMDOS trap (`svc #1`): function id in `x8`, first arg in `x0`,
/// result in `x0`. Used for the desktop's file operations (Dcreate, …).
fn gemdos1(func: u64, a0: u64) -> i64 {
    let r: i64;
    // SAFETY: GEMDOS trap; `func`/`a0` are a valid function id + argument.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") func,
            inout("x0") a0 => r,
            options(nostack),
        );
    }
    r
}

/// Issue a 3-argument GEMDOS trap (`svc #1`): function id in `x8`, args in
/// `x0`/`x1`/`x2`, result in `x0`. Used for Frename.
fn gemdos3(func: u64, a0: u64, a1: u64, a2: u64) -> i64 {
    let r: i64;
    // SAFETY: GEMDOS trap; valid function id + argument registers.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") func,
            inout("x0") a0 => r,
            in("x1") a1,
            in("x2") a2,
            options(nostack),
        );
    }
    r
}

// evnt_multi event classes and AES message ids (stable GEM protocol).
const MU_KEYBD: u16 = 0x0001;
const MU_BUTTON: u16 = 0x0002;
const MU_MESAG: u16 = 0x0010;
const MN_SELECTED: i16 = 10;
const WM_REDRAW: i16 = 20;
const WM_TOPPED: i16 = 21;
const WM_CLOSED: i16 = 22;
const WM_MOVED: i16 = 28;

/// A reusable AES parameter block. `call` zeroes `control`, sets the
/// opcode + counts, and traps; results land in `intout`/`addrout`.
struct Aes {
    control: [i16; 8],
    global: [i16; 32],
    intin: [i16; 16],
    intout: [i16; 16],
    addrin: [i64; 4],
    addrout: [i64; 4],
}

impl Aes {
    fn new() -> Self {
        Self {
            control: [0; 8], global: [0; 32], intin: [0; 16],
            intout: [0; 16], addrin: [0; 4], addrout: [0; 4],
        }
    }

    /// Trap opcode `op` with `n_int` int-ins and `n_addr` addr-ins.
    fn call(&mut self, op: i16, n_int: i16, n_addr: i16) {
        self.control = [0; 8];
        self.control[0] = op;
        self.control[1] = n_int;
        self.control[3] = n_addr;
        let block: [u64; 6] = [
            self.control.as_ptr() as u64, self.global.as_ptr() as u64,
            self.intin.as_ptr() as u64, self.intout.as_ptr() as u64,
            self.addrin.as_ptr() as u64, self.addrout.as_ptr() as u64,
        ];
        aes(block.as_ptr() as u64);
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

    use crate::desktop_icons as ic;

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

    // EmuTOS desktop fixtures (deskapp.c:734-750): the Trash (#T, IG_TRASH)
    // at the bottom-left, and the Printer (#O, IG_PRINT) at the bottom-right.
    let ycnt = ((sh - MENU_H) / ich).max(1);
    let cy_bot = MENU_H + 18 + (ycnt - 1) * ich;
    draw_fixture_icon(backend, 20 + icw / 2, cy_bot, &ic::TRASH_DATA, &ic::TRASH_MASK, b"Trash", clip);
    let printer_cx = 20 + (xcnt - 1) * icw + icw / 2;
    draw_fixture_icon(backend, printer_cx, cy_bot, &ic::PRINTER_DATA, &ic::PRINTER_MASK, b"Printer", clip);
}

/// Draw a non-drive desktop fixture icon (trash/printer): the real ICONBLK
/// bitmap with a name label, no drive letter.
fn draw_fixture_icon(
    backend: &dyn RasterBackend, cx: i16, y: i16,
    data: &[u16; 64], mask: &[u16; 64], label: &[u8], clip: ClipRect,
) {
    blit_icon(backend, cx - 16, y, data, mask, clip);
    draw_icon_label(backend, cx, y, label, clip);
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
            b" Format floppy... \0", b"------------------\0",
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
            // Box width = widest item text + the renderer's 2-cell check
            // column indent (object.rs draws G_STRING at x + 2*CELL_W) +
            // a right margin, so text and separators never touch the edge.
            let maxlen = items.iter().map(|s| cstr_len(s) as i16).max().unwrap_or(8);
            let iw = maxlen * 8 + 2 * 8 + 6;
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
/// window (the response a click on the C drive triggers).
pub fn preview_dir() {
    draw();
    open_dir_window(b'C', (40, MENU_H + 24, 360, 300));
}

/// The desktop main loop (EmuTOS `deskmain`): paint the desktop, then
/// dispatch AES events — menu selections, drive clicks, window messages —
/// until a *Shutdown* selection, when it returns (the caller halts).
/// Mouse-driven; works in QEMU (real-hardware mouse waits on IKBD bring-up).
pub fn run() {
    draw();
    let mut msg = [0i16; 8];
    while step(&mut msg) {}
}

/// One iteration of the desktop loop: wait for an AES event and dispatch
/// it. Returns `false` only on a Shutdown selection (the loop should exit).
fn step(msg: &mut [i16; 8]) -> bool {
    // evnt_multi(KEYBD|BUTTON|MESAG): wait for a left-button press or a
    // menu/window message. No timer — the desktop is event-driven.
    let mut a = Aes::new();
    a.addrin[0] = msg.as_mut_ptr() as i64;
    a.intin[0] = (MU_KEYBD | MU_BUTTON | MU_MESAG) as i16;
    a.intin[1] = 1; // clicks requested
    a.intin[2] = 1; // button mask = left
    a.intin[3] = 1; // fire on press
    a.call(25, 16, 1);
    let fired = a.intout[0] as u16;
    let (mx, my) = (a.intout[1], a.intout[2]);

    if fired & MU_MESAG != 0 {
        match msg[0] {
            MN_SELECTED => {
                let shutdown = handle_menu_select(msg[4]);
                menu_tnormal(msg[3]); // un-highlight the title
                if shutdown {
                    return false;
                }
            }
            WM_CLOSED => close_win(msg[3]),
            WM_TOPPED => {
                set_top(msg[3]);
                redraw_win(msg[3]);
            }
            WM_REDRAW => redraw_win(msg[3]),
            WM_MOVED => move_win(msg[3], (msg[4], msg[5], msg[6], msg[7])),
            _ => {}
        }
    }
    if fired & MU_BUTTON != 0 {
        // A raw click the AES did not claim → desktop surface or a window
        // work area. A drive icon opens it; otherwise select the entry the
        // click lands on. (Click-count isn't wired yet; debounce on release.)
        if let Some(letter) = drive_at(mx, my) {
            open_cascaded(letter);
            wait_release();
        } else if select_at(mx, my) {
            // A file was grabbed: follow the drag; dropping it on the Trash
            // deletes it (EmuTOS drag-to-trash).
            let (dx, dy) = follow_drag();
            if trash_at(dx, dy) {
                delete_selected();
            }
        } else {
            wait_release();
        }
    }
    true
}

/// Verification (no real mouse in QEMU): synthesize a left-click over the
/// C: drive icon, run one `evnt_multi`, dispatch the button, and leave the
/// opened window on screen for a screendump. Exercises the full
/// evnt_multi → button → drive_at → open_dir_window chain. (Skips the
/// `wait_release` debounce, which would block with no live input source.)
pub fn preview_click() {
    draw();
    let backend = &crate::vdi_backend::VDI_SCREEN;
    let (cx, cy) = (60i16, MENU_H + 40); // over the C: icon
    // Initialise the cursor first (feed_relative is relative to the
    // *post-init* position), then move it to the target with the button down.
    tos_api::vdi::feed_relative(0, 0, 0x00, backend);
    let (mx, my) = tos_api::vdi::mouse_position();
    tos_api::vdi::feed_relative(cx - mx, cy - my, 0x01, backend);
    click_once();
    tos_api::vdi::feed_relative(0, 0, 0x00, backend);
}

/// Verification of the *real IKBD input path*: position the cursor and
/// press the left button purely by feeding raw Atari IKBD relative-mouse
/// packets to `crate::ikbd::feed` (exactly what UART2 delivers on
/// hardware), then run one `evnt_multi` + dispatch. Proves the full chain
/// IKBD bytes → decoder → VDI cursor → desktop event loop end-to-end,
/// without a live mouse.
pub fn preview_ikbd_click() {
    draw();
    // Relative-mouse packets: header 0xF8 (no buttons) / 0xFA (left down),
    // then signed dx, dy. The first packet centres the cursor (init), then
    // three moves walk it onto the C: icon (~60, 60), then a button packet.
    let packets: [[u8; 3]; 5] = [
        [0xF8, 0, 0],
        [0xF8, (-100i8) as u8, (-90i8) as u8],
        [0xF8, (-100i8) as u8, (-90i8) as u8],
        [0xF8, (-60i8) as u8, 0],
        [0xFA, 0, 0], // left button down, no move
    ];
    for pkt in packets {
        for b in pkt {
            crate::ikbd::feed(b);
        }
    }
    kprintln!("[desk] IKBD cursor at {:?} buttons={}",
        tos_api::vdi::mouse_position(), tos_api::vdi::mouse_buttons());
    click_once();
    for b in [0xF8u8, 0, 0] {
        crate::ikbd::feed(b); // button up
    }
}

/// One `evnt_multi(BUTTON|MESAG)` + dispatch of a desktop-surface click,
/// without the `wait_release` debounce (which would block with no live
/// input source). Shared by the verification previews.
fn click_once() {
    let mut msg = [0i16; 8];
    let mut a = Aes::new();
    a.addrin[0] = msg.as_mut_ptr() as i64;
    a.intin[0] = (MU_BUTTON | MU_MESAG) as i16;
    a.intin[1] = 1; a.intin[2] = 1; a.intin[3] = 1;
    a.call(25, 16, 1);
    let fired = a.intout[0] as u16;
    if fired & MU_BUTTON != 0 {
        if let Some(letter) = drive_at(a.intout[1], a.intout[2]) {
            kprintln!("[desk] click → open {}:", letter as char);
            open_cascaded(letter);
        }
    }
}

/// Verification of the View-menu sort orders: set `mode` (0=name 1=type
/// 2=size 3=date 4=none) and open a C: window so the listing reflects it.
pub fn preview_sort(mode: u8) {
    draw();
    SORT_MODE.store(mode, core::sync::atomic::Ordering::Release);
    open_dir_window(b'C', (40, MENU_H + 24, 420, 340));
}

/// Verification of the "Show as icons" view: open a C: window in icon mode.
pub fn preview_icons() {
    draw();
    VIEW_MODE.store(1, core::sync::atomic::Ordering::Release);
    open_dir_window(b'C', (40, MENU_H + 24, 460, 360));
}

/// Verification of "Size to fit": open a deliberately short window, then
/// grow it to fit the listing.
pub fn preview_fit() {
    draw();
    open_dir_window(b'C', (40, MENU_H + 24, 300, 120));
    size_to_fit();
}

/// Verification of "New folder": open a C: window, create a folder, and
/// repaint so the new NEWFLDR entry shows in the listing.
pub fn preview_newfolder() {
    draw();
    open_dir_window(b'C', (40, MENU_H + 24, 420, 340));
    new_folder();
}

/// Verification of selection: open a C: window and select entry 4 so its
/// row shows inverted.
pub fn preview_select() {
    draw();
    let h = open_dir_window(b'C', (40, MENU_H + 24, 420, 340));
    SEL_WIN.store(h, core::sync::atomic::Ordering::Release);
    SEL_IDX.store(4, core::sync::atomic::Ordering::Release);
    redraw_win(h);
}

/// Verification of the delete-confirm dialog: select an entry, then draw
/// the confirmation alert (blocks for the screendump).
pub fn preview_confirm() {
    draw();
    let h = open_dir_window(b'C', (40, MENU_H + 24, 420, 340));
    SEL_WIN.store(h, core::sync::atomic::Ordering::Release);
    SEL_IDX.store(4, core::sync::atomic::Ordering::Release);
    redraw_win(h);
    if let Some((_, _, name, _)) = resolve_selection() {
        confirm_delete(&name);
    }
}

/// Verification of the Info/Rename dialog: select an entry and open the
/// rename dialog (pre-filled with its name; blocks for the screendump).
pub fn preview_rename() {
    draw();
    let h = open_dir_window(b'C', (40, MENU_H + 24, 420, 340));
    SEL_WIN.store(h, core::sync::atomic::Ordering::Release);
    SEL_IDX.store(4, core::sync::atomic::Ordering::Release);
    redraw_win(h);
    info_rename();
}

/// Verification of the rename itself (bypassing the dialog): rename
/// FILE15.TXT → RENAMED.TXT and repaint.
pub fn preview_dorename() {
    draw();
    let h = open_dir_window(b'C', (40, MENU_H + 24, 420, 340));
    do_rename(b'C', b"FILE15.TXT", b"RENAMED.TXT");
    redraw_win(h);
}

/// Verification of the Trash hit-test: move the cursor onto the Trash icon
/// and onto a blank spot, logging `trash_at` for each.
pub fn preview_trash() {
    draw();
    open_dir_window(b'C', (40, MENU_H + 24, 300, 120));
    let backend = &crate::vdi_backend::VDI_SCREEN;
    let ich = 64i16;
    let ycnt = ((480 - MENU_H) / ich).max(1);
    let cy = MENU_H + 18 + (ycnt - 1) * ich + 16; // over the trash body
    tos_api::vdi::feed_relative(0, 0, 0, backend);
    let to = |x: i16, y: i16| {
        let (mx, my) = tos_api::vdi::mouse_position();
        tos_api::vdi::feed_relative(x - mx, y - my, 0, backend);
        tos_api::vdi::mouse_position()
    };
    let p = to(60, cy);
    kprintln!("[desk] cursor {:?} trash_at = {}", p, trash_at(p.0, p.1));
    let q = to(300, 200);
    kprintln!("[desk] cursor {:?} trash_at = {}", q, trash_at(q.0, q.1));
}

/// Verification of the actual deletion (bypassing the interactive confirm):
/// delete FILE15.TXT from C:\ and repaint so it disappears.
pub fn preview_delete() {
    draw();
    let h = open_dir_window(b'C', (40, MENU_H + 24, 420, 340));
    let mut name = [0u8; 14];
    name[..10].copy_from_slice(b"FILE15.TXT");
    do_delete(b'C', &name, false);
    redraw_win(h);
}

/// Verification of the File-menu window actions: open three cascaded
/// windows, then "Cycle windows" to bring the bottom one to the top.
pub fn preview_windows(cycle: bool) {
    draw();
    open_cascaded(b'C');
    open_cascaded(b'D');
    open_cascaded(b'C');
    if cycle {
        cycle_windows();
    }
}

/// Dispatch an `MN_SELECTED` item by its menu text. Returns `true` on the
/// Shutdown item (the loop should exit). Mirrors EmuTOS `do_*menu`.
fn handle_menu_select(item: i16) -> bool {
    if item_has(item, b"Shutdown") {
        kprintln!("[desk] menu: Shutdown");
        return true;
    }
    if item_has(item, b"Desktop info") {
        show_info_alert();
    } else if item_has(item, b"Open") {
        // Open the first present hard drive (a selection model lands later).
        let bits = tos_api::bios::drive::drvbits();
        for d in 2..26u32 {
            if bits & (1 << d) != 0 {
                open_cascaded(b'A' + d as u8);
                break;
            }
        }
    } else if item_has(item, b"Close top window") || item_has(item, b"Close folder") {
        close_top_window();
    } else if item_has(item, b"Cycle windows") {
        cycle_windows();
    } else if item_has(item, b"New folder") {
        new_folder();
    } else if item_has(item, b"Delete") {
        delete_selected();
    } else if item_has(item, b"Info/Rename") {
        info_rename();
    } else if item_has(item, b"Show as icons") {
        set_view(1);
    } else if item_has(item, b"Show as text") {
        set_view(0);
    } else if item_has(item, b"Sort by name") {
        set_sort(0);
    } else if item_has(item, b"Sort by type") {
        set_sort(1);
    } else if item_has(item, b"Sort by size") {
        set_sort(2);
    } else if item_has(item, b"Sort by date") {
        set_sort(3);
    } else if item_has(item, b"No sort") {
        set_sort(4);
    } else if item_has(item, b"Size to fit") {
        size_to_fit();
    } else {
        // Remaining items are logged until their handlers land.
        kprintln!("[desk] menu item {} selected", item);
    }
    false
}

/// True if the menu item `item`'s text contains `needle`.
fn item_has(item: i16, needle: &[u8]) -> bool {
    if item < 0 {
        return false;
    }
    // SAFETY: `item` indexes the persistent menu tree; ob_spec is a C string.
    let spec = unsafe { (*build_menu().add(item as usize)).ob_spec };
    if spec == 0 {
        return false;
    }
    let s = spec as *const u8;
    // Scan the (short, NUL-terminated) item text for `needle`.
    let mut buf = [0u8; 32];
    let mut n = 0usize;
    while n < buf.len() {
        // SAFETY: NUL-terminated within the resource string.
        let c = unsafe { *s.add(n) };
        if c == 0 {
            break;
        }
        buf[n] = c;
        n += 1;
    }
    buf[..n].windows(needle.len()).any(|w| w == needle)
}

/// `menu_tnormal(tree, title, 1)` — un-highlight a menu title after acting
/// on its selection (EmuTOS `deskmain.c:1035`).
fn menu_tnormal(title: i16) {
    let mut a = Aes::new();
    a.addrin[0] = build_menu() as i64;
    a.intin[0] = title;
    a.intin[1] = 1; // normal
    a.call(33, 2, 1);
}

/// Open a directory window for `letter`, cascading each new window down-right.
fn open_cascaded(letter: u8) {
    use core::sync::atomic::{AtomicI16, Ordering};
    static NEXT: AtomicI16 = AtomicI16::new(0);
    let k = NEXT.fetch_add(1, Ordering::Relaxed).rem_euclid(5);
    let off = k * 24;
    open_dir_window(letter, (40 + off, MENU_H + 24 + off, 360, 280));
}

/// Handle `WM_CLOSED`: close the window and free its slot. `wind_close`
/// triggers `redraw_all`, which repaints the desktop via the painter hook.
fn close_win(handle: i16) {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.call(102, 1, 0); // wind_close
    if let Some(s) = win_slot(handle) {
        s.drive = 0;
    }
    // If the top window just closed, promote another open one.
    if DESK_TOP.load(core::sync::atomic::Ordering::Acquire) == handle {
        set_top(any_open_handle());
    }
}

/// Handle `WM_REDRAW`/`WM_TOPPED`: repaint the directory listing from the
/// tracked rect (avoids the flaky `wind_get(WF_WXYWH)`).
fn redraw_win(handle: i16) {
    if let Some(s) = win_slot(handle) {
        if s.drive != 0 {
            draw_dir_listing(handle, s.drive, work_of(s.rect));
        }
    }
}

/// Handle `WM_MOVED`: confirm the drag via `wind_set(WF_CXYWH, rect)`,
/// update the tracked rect, and repaint the listing.
fn move_win(handle: i16, rect: (i16, i16, i16, i16)) {
    let mut a = Aes::new();
    a.intin[0] = handle; a.intin[1] = 5; // WF_CXYWH
    a.intin[2] = rect.0; a.intin[3] = rect.1; a.intin[4] = rect.2; a.intin[5] = rect.3;
    a.call(105, 6, 0); // wind_set
    if let Some(s) = win_slot(handle) {
        s.rect = rect;
        if s.drive != 0 {
            draw_dir_listing(handle, s.drive, work_of(rect));
        }
    }
}

/// The desktop's notion of the top window (kept in sync with the AES on
/// open and on `WM_TOPPED`), so the File-menu window actions have a target
/// without the circular `wind_get(WF_TOP)` (which needs a window handle).
static DESK_TOP: core::sync::atomic::AtomicI16 = core::sync::atomic::AtomicI16::new(0);

/// Record `handle` as the top window.
fn set_top(handle: i16) {
    DESK_TOP.store(handle, core::sync::atomic::Ordering::Release);
}

/// First open window handle (or 0 if none), for picking a new top.
fn any_open_handle() -> i16 {
    for h in 1..8i16 {
        if win_slot(h).map(|s| s.drive != 0).unwrap_or(false) {
            return h;
        }
    }
    0
}

/// The open window after `h` in round-robin handle order (or `h` itself if
/// it is the only one; 0 if none open).
fn next_open_after(h: i16) -> i16 {
    for k in 1..=7i16 {
        let cand = (h - 1 + k).rem_euclid(7) + 1; // 1..=7
        if win_slot(cand).map(|s| s.drive != 0).unwrap_or(false) {
            return cand;
        }
    }
    0
}

/// `wind_set(handle, WF_TOP)` — bring `handle` to the top.
fn wind_top(handle: i16) {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = 10; // WF_TOP
    a.call(105, 2, 0);
}

/// File → "Close top window" / "Close folder": close the top window and
/// promote another open one to the top (EmuTOS `do_filemenu` CLSWITEM).
fn close_top_window() {
    let h = DESK_TOP.load(core::sync::atomic::Ordering::Acquire);
    if h < 1 {
        return;
    }
    close_win(h);
    let nh = any_open_handle();
    set_top(nh);
    if nh >= 1 {
        wind_top(nh);
    }
}

/// File → "New folder": create `NEWFLDR[n]` in the top window's drive root
/// via `Dcreate` (GEMDOS 0x39), then repaint the window. (A name-entry
/// dialog is a refinement; this picks the first free default name.)
fn new_folder() {
    let h = DESK_TOP.load(core::sync::atomic::Ordering::Acquire);
    let letter = match win_slot(h) {
        Some(s) if s.drive != 0 => s.drive,
        _ => return,
    };
    for n in 0..10u8 {
        // "X:\NEWFLDR" + optional digit + NUL.
        let mut path = [letter, b':', b'\\', b'N', b'E', b'W', b'F', b'L', b'D', b'R', 0, 0, 0];
        let mut len = 10usize;
        if n > 0 {
            path[len] = b'0' + n;
            len += 1;
        }
        path[len] = 0;
        if gemdos1(0x39, path.as_ptr() as u64) == 0 {
            kprintln!("[desk] New folder: created {}:\\NEWFLDR", letter as char);
            redraw_win(h);
            return;
        }
    }
    kprintln!("[desk] New folder: failed (no free name / not writable)");
}

/// Resolve the current selection to `(window, drive, name, is_dir)` by
/// re-listing+sorting the selected window and indexing the sorted list.
fn resolve_selection() -> Option<(i16, u8, [u8; 14], bool)> {
    let h = SEL_WIN.load(core::sync::atomic::Ordering::Acquire);
    if h < 1 {
        return None;
    }
    let letter = match win_slot(h) {
        Some(s) if s.drive != 0 => s.drive,
        _ => return None,
    };
    let idx = SEL_IDX.load(core::sync::atomic::Ordering::Acquire) as usize;
    let path = [letter, b':', b'\\', b'*', b'.', b'*'];
    let path_str = core::str::from_utf8(&path).unwrap_or("C:\\*.*");
    let mut ents = [tos_api::bdos::fs::find::DirInfo::default(); 64];
    let n = tos_api::bdos::fs::find::list_dir_ex(path_str, &mut ents);
    sort_entries(&mut ents[..n]);
    if idx >= n {
        return None;
    }
    Some((h, letter, ents[idx].name, ents[idx].attrib & FA_SUBDIR != 0))
}

/// Delete `X:\NAME` via `Fdelete` (0x41, files) or `Ddelete` (0x3A, empty
/// dirs). Returns true on success.
fn do_delete(letter: u8, name: &[u8; 14], is_dir: bool) -> bool {
    let mut full = [0u8; 20];
    full[0] = letter;
    full[1] = b':';
    full[2] = b'\\';
    let nend = name.iter().position(|&b| b == 0).unwrap_or(14);
    full[3..3 + nend].copy_from_slice(&name[..nend]);
    full[3 + nend] = 0;
    let func = if is_dir { 0x3A } else { 0x41 };
    let r = gemdos1(func, full.as_ptr() as u64);
    if r == 0 {
        kprintln!("[desk] deleted {}:\\{}", letter as char,
            core::str::from_utf8(&name[..nend]).unwrap_or("?"));
        true
    } else {
        kprintln!("[desk] delete failed ({})", r);
        false
    }
}

/// Confirm a deletion with `form_alert`. Default button = Cancel.
fn confirm_delete(name: &[u8; 14]) -> bool {
    let mut buf = [0u8; 80];
    let mut k = 0;
    for &b in b"[2][ Delete | " {
        buf[k] = b;
        k += 1;
    }
    let nend = name.iter().position(|&b| b == 0).unwrap_or(14);
    for &c in &name[..nend] {
        buf[k] = c;
        k += 1;
    }
    for &b in b" ? ][ OK | Cancel ]\0" {
        buf[k] = b;
        k += 1;
    }
    let mut a = Aes::new();
    a.addrin[0] = buf.as_ptr() as i64;
    a.intin[0] = 2; // default = Cancel (the safe choice)
    a.call(52, 1, 1); // form_alert
    a.intout[0] == 1 // button 1 = OK
}

/// File → "Delete": delete the selected entry after confirmation, then
/// clear the selection and repaint (EmuTOS `do_filemenu` DELTITEM).
fn delete_selected() {
    if let Some((h, letter, name, is_dir)) = resolve_selection() {
        if confirm_delete(&name) && do_delete(letter, &name, is_dir) {
            SEL_WIN.store(0, core::sync::atomic::Ordering::Release);
            redraw_win(h);
        }
    }
}

/// Build `X:\NAME\0` into `buf`; returns the length before the NUL.
fn drive_path(buf: &mut [u8; 20], letter: u8, name: &[u8]) -> usize {
    buf[0] = letter;
    buf[1] = b':';
    buf[2] = b'\\';
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    buf[3..3 + end].copy_from_slice(&name[..end]);
    buf[3 + end] = 0;
    3 + end
}

/// Rename `X:\old` → `X:\new` via `Frename` (0x56). Returns true on success.
fn do_rename(letter: u8, old: &[u8], new: &[u8]) -> bool {
    let mut op = [0u8; 20];
    let mut np = [0u8; 20];
    drive_path(&mut op, letter, old);
    drive_path(&mut np, letter, new);
    let r = gemdos3(0x56, 0, op.as_ptr() as u64, np.as_ptr() as u64);
    if r == 0 {
        kprintln!("[desk] renamed {}:\\ entry", letter as char);
        true
    } else {
        kprintln!("[desk] rename failed ({})", r);
        false
    }
}

/// File → "Info/Rename": show a dialog with the selected file's name in an
/// editable field; OK renames it via `Frename` (EmuTOS `do_filemenu`
/// SHOWITEM, reduced to the rename function).
fn info_rename() {
    let (h, letter, name, _is_dir) = match resolve_selection() {
        Some(x) => x,
        None => return,
    };
    let nend = name.iter().position(|&b| b == 0).unwrap_or(14);
    let mut textbuf = [0u8; 16];
    textbuf[..nend].copy_from_slice(&name[..nend]);

    static LABEL: &[u8] = b"Rename file to:\0";
    static OK: &[u8] = b" OK \0";
    static CANCEL: &[u8] = b" Cancel \0";
    static EMPTY: &[u8] = b"\0";
    let ted = TedInfo {
        te_ptext: textbuf.as_mut_ptr() as u64, te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64, te_font: 3, te_junk1: 0, te_just: 0,
        te_color: 1 << 8, te_junk2: 0, te_thickness: 1, te_txtlen: 13, te_tmplen: 0,
    };
    let tree = [
        mk(G_BOX, FL3DBAK, 1 << 16, 180, 150, 290, 150, -1, 1, 4),
        mk(G_STRING, 0, LABEL.as_ptr() as u64, 24, 18, 240, 16, 2, -1, -1),
        mk(G_BOXTEXT, EDITABLE | FL3DACT, &ted as *const _ as u64, 24, 46, 200, 24, 3, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT | FL3DACT, OK.as_ptr() as u64, 28, 104, 90, 28, 4, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, 168, 104, 90, 28, 0, -1, -1),
    ];

    let mut a = Aes::new();
    a.addrin[0] = tree.as_ptr() as i64;
    a.intin[0] = 0; a.intin[1] = 8; a.intin[2] = 0; a.intin[3] = 0; a.intin[4] = 640; a.intin[5] = 480;
    a.call(42, 6, 1); // objc_draw
    a.intin[0] = 0;
    a.addrin[0] = tree.as_ptr() as i64;
    a.call(50, 1, 1); // form_do
    let clicked = a.intout[0];
    if clicked == 3 {
        // OK: rename if the field changed and is non-empty.
        let m = textbuf.iter().position(|&b| b == 0).unwrap_or(textbuf.len());
        if m > 0 && textbuf[..m] != name[..nend] {
            do_rename(letter, &name[..nend], &textbuf[..m]);
            SEL_WIN.store(0, core::sync::atomic::Ordering::Release);
            redraw_win(h);
        }
    }
}

/// View → "Size to fit": resize the top window so its work area just fits
/// the directory listing, capped at the desktop bottom (EmuTOS `do_viewmenu`
/// FITITEM, simplified to a vertical fit).
fn size_to_fit() {
    let h = DESK_TOP.load(core::sync::atomic::Ordering::Acquire);
    if h < 1 {
        return;
    }
    let (letter, (x, y, w, _)) = match win_slot(h) {
        Some(s) if s.drive != 0 => (s.drive, s.rect),
        _ => return,
    };
    // Count entries to size the work area.
    let path = [letter, b':', b'\\', b'*', b'.', b'*'];
    let path_str = core::str::from_utf8(&path).unwrap_or("C:\\*.*");
    let mut ents = [tos_api::bdos::fs::find::DirInfo::default(); 64];
    let n = tos_api::bdos::fs::find::list_dir_ex(path_str, &mut ents) as i16;
    // Work height ≈ n rows × 16 + pad; add the frame overhead (title/info +
    // scrollbar ≈ 56) and cap to the desktop bottom.
    let new_h = (n * 16 + 12 + 56).clamp(120, 480 - y);
    move_win(h, (x, y, w, new_h));
}

/// File → "Cycle windows": bring the next open window to the top (EmuTOS
/// `do_filemenu` CYCLITEM).
fn cycle_windows() {
    let h = DESK_TOP.load(core::sync::atomic::Ordering::Acquire);
    let nh = next_open_after(h);
    if nh >= 1 && nh != h {
        wind_top(nh);
        set_top(nh);
        redraw_win(nh);
    }
}

/// `form_alert` for the Desk → "Desktop info..." item.
fn show_info_alert() {
    let alert = b"[1][ NewTOS Desktop | TOS-compatible OS | for AArch64 ][ OK ]\0";
    let mut a = Aes::new();
    a.addrin[0] = alert.as_ptr() as i64;
    a.intin[0] = 1; // default button
    a.call(52, 1, 1); // form_alert
}

/// Hit-test the desktop drive-icon grid (same layout as `paint_desktop_bg`).
/// Returns the drive letter whose icon contains `(mx, my)`.
fn drive_at(mx: i16, my: i16) -> Option<u8> {
    let backend = tos_api::host::host().and_then(|h| h.raster())?;
    let sw = backend.width() as i16;
    let bits = tos_api::bios::drive::drvbits();
    let icw = 80i16;
    let ich = 64i16;
    let xcnt = ((sw - 20) / icw).max(1);
    let mut idx = 0i16;
    for d in 0..26u32 {
        if bits & (1 << d) != 0 {
            let cx = 20 + (idx % xcnt) * icw + icw / 2;
            let cy = MENU_H + 18 + (idx / xcnt) * ich;
            // Icon + label box ~ 56 wide, 56 tall, centred on cx, top at cy.
            if (mx - cx).abs() <= 28 && my >= cy && my <= cy + 56 {
                return Some(b'A' + d as u8);
            }
            idx += 1;
        }
    }
    None
}

/// Spin until the left mouse button is released (debounce a raw click).
fn wait_release() {
    if let Some(h) = tos_api::host::host() {
        while tos_api::vdi::mouse_buttons() & 0x01 != 0 {
            h.pump_input();
        }
    }
}

/// Follow the mouse until the button is released; return the release
/// position (the drop point of a drag).
fn follow_drag() -> (i16, i16) {
    let mut last = tos_api::vdi::mouse_position();
    if let Some(h) = tos_api::host::host() {
        while tos_api::vdi::mouse_buttons() & 0x01 != 0 {
            last = tos_api::vdi::mouse_position();
            h.pump_input();
        }
    }
    last
}

/// True if `(mx, my)` is over the desktop Trash icon (bottom-left fixture,
/// same layout as `paint_desktop_bg`).
fn trash_at(mx: i16, my: i16) -> bool {
    let Some(backend) = tos_api::host::host().and_then(|h| h.raster()) else {
        return false;
    };
    let ich = 64i16;
    let ycnt = ((backend.height() as i16 - MENU_H) / ich).max(1);
    let cy_bot = MENU_H + 18 + (ycnt - 1) * ich;
    let cx = 20 + 80 / 2; // trash centre x (icw/2)
    (mx - cx).abs() <= 28 && my >= cy_bot && my <= cy_bot + 56
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

/// Blit a 32×32 1bpp EmuTOS ICONBLK with mask, top-left at `(ix, y)`:
/// opaque where the mask is set (body white, data pixels black),
/// transparent elsewhere (the desktop shows through).
fn blit_icon(backend: &dyn RasterBackend, ix: i16, y: i16, data: &[u16; 64], mask: &[u16; 64], clip: ClipRect) {
    for r in 0..32i16 {
        let dw = ((data[r as usize * 2] as u32) << 16) | data[r as usize * 2 + 1] as u32;
        let mw = ((mask[r as usize * 2] as u32) << 16) | mask[r as usize * 2 + 1] as u32;
        for c in 0..32i16 {
            let bit = 1u32 << (31 - c);
            if mw & bit != 0 {
                let px = if dw & bit != 0 { BLACK } else { WHITE };
                backend.fill_rect(ix + c, y + r, 1, 1, px, clip);
            }
        }
    }
}

/// Draw a centred name label below an icon (`ib_ptext`), at `(cx, y+34)`.
fn draw_icon_label(backend: &dyn RasterBackend, cx: i16, y: i16, label: &[u8], clip: ClipRect) {
    let lx = cx - (label.len() as i16 * 8) / 2;
    for (i, &c) in label.iter().enumerate() {
        backend.draw_glyph(lx + i as i16 * 8, y + 34, tos_api::vdi::font::glyph(c as u16), BLACK, clip);
    }
}

/// Draw a drive icon using the real EmuTOS ICONBLK bitmap (hard disk for
/// `hard`, else floppy), the drive letter OR-ed into the icon
/// (`ib_char` cell), and the "Disk" name below (`ib_ptext`/STDISK).
fn draw_drive_icon(backend: &dyn RasterBackend, cx: i16, y: i16, letter: u8, hard: bool, clip: ClipRect) {
    use crate::desktop_icons as ic;
    let (data, mask, ch) = if hard {
        (&ic::HARD_DATA, &ic::HARD_MASK, ic::HARD_CHAR)
    } else {
        (&ic::FLOPPY_DATA, &ic::FLOPPY_MASK, ic::FLOPPY_CHAR)
    };
    let ix = cx - 16; // 32-px icon centred on cx
    blit_icon(backend, ix, y, data, mask, clip);
    // Drive letter in the icon's char cell (EmuTOS `ib_char |= a_letter`).
    backend.draw_glyph(ix + ch.0, y + ch.1, tos_api::vdi::font::glyph(letter as u16), BLACK, clip);
    draw_icon_label(backend, cx, y, b"Disk", clip);
}

/// Window border `kind` bits (aesdefs.h:519).
const W_KIND: i16 =
    (0x0001 | 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0040 | 0x0080 | 0x0100) as i16;
// NAME | CLOSER | MOVER | INFO | SIZER | UPARROW | DNARROW | VSLIDE

/// Window → drive + current border rect, tracked by the desktop so window
/// messages (redraw/move) can repaint the listing without the flaky
/// `wind_get(WF_WXYWH)` lookup. Index = window handle (1..8); `drive == 0`
/// means the slot is free.
#[derive(Clone, Copy)]
struct WinInfo {
    drive: u8,
    rect: (i16, i16, i16, i16),
}
struct WinTable {
    t: UnsafeCell<[WinInfo; 8]>,
}
unsafe impl Sync for WinTable {}
static WINS: WinTable = WinTable {
    t: UnsafeCell::new([WinInfo { drive: 0, rect: (0, 0, 0, 0) }; 8]),
};

fn win_slot(handle: i16) -> Option<&'static mut WinInfo> {
    if handle < 1 || handle as usize >= 8 {
        return None;
    }
    // SAFETY: single-CPU desktop; one borrow live at a time.
    Some(unsafe { &mut (*WINS.t.get())[handle as usize] })
}

/// `wind_calc(WC_WORK, kind, border-rect)` → the work area. A *pure*
/// conversion (no window-table lookup), so it sidesteps the `&'static mut`
/// aliasing flakiness `wind_get(WF_WXYWH)` showed in release builds.
fn work_of(rect: (i16, i16, i16, i16)) -> (i16, i16, i16, i16) {
    let mut a = Aes::new();
    a.intin[0] = 1; // WC_WORK
    a.intin[1] = W_KIND;
    a.intin[2] = rect.0; a.intin[3] = rect.1; a.intin[4] = rect.2; a.intin[5] = rect.3;
    a.call(108, 6, 0);
    (a.intout[1], a.intout[2], a.intout[3], a.intout[4])
}

/// Open a directory window for drive `letter` at `rect` and draw its file
/// listing — the desktop's response to opening a drive (EmuTOS
/// `do_diropen`). Returns the window handle, or 0 on failure.
fn open_dir_window(letter: u8, rect: (i16, i16, i16, i16)) -> i16 {
    let mut a = Aes::new();

    // wind_create(kind, full extent = whole desktop below the menu).
    a.intin[0] = W_KIND; a.intin[1] = 0; a.intin[2] = MENU_H; a.intin[3] = 640; a.intin[4] = 480 - MENU_H;
    a.call(100, 5, 0);
    let handle = a.intout[0];
    if handle < 1 {
        return 0;
    }

    // wind_set(WF_NAME, "X:\").
    let mut name = [letter, b':', b'\\', 0, 0, 0, 0, 0];
    a.addrin[0] = name.as_mut_ptr() as i64;
    a.intin[0] = handle; a.intin[1] = 2;
    a.call(105, 2, 1);

    // wind_open(handle, rect).
    a.intin[0] = handle; a.intin[1] = rect.0; a.intin[2] = rect.1; a.intin[3] = rect.2; a.intin[4] = rect.3;
    a.call(101, 5, 0);

    if let Some(s) = win_slot(handle) {
        s.drive = letter;
        s.rect = rect;
    }
    set_top(handle);
    draw_dir_listing(handle, letter, work_of(rect));
    handle
}

/// Directory-view sort order (View menu): 0=name 1=type 2=size 3=date
/// 4=none. The desktop applies one order to all directory windows, like
/// the classic GEM desktop.
static SORT_MODE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

const FA_SUBDIR: u8 = 0x10;

/// Set the sort order (View menu) and repaint every open directory window.
fn set_sort(mode: u8) {
    SORT_MODE.store(mode, core::sync::atomic::Ordering::Release);
    redraw_all_windows();
}

/// Repaint the listing of every open directory window.
fn redraw_all_windows() {
    for h in 1..8i16 {
        redraw_win(h);
    }
}

/// The extension bytes of a `NAME.EXT\0` name (after the dot; empty if none).
fn name_ext(name: &[u8; 14]) -> &[u8] {
    let end = name.iter().position(|&b| b == 0).unwrap_or(14);
    match name[..end].iter().position(|&b| b == b'.') {
        Some(p) => &name[p + 1..end],
        None => &name[end..end],
    }
}

/// Sort `ents` in place per [`SORT_MODE`] — directories first, then by the
/// chosen key (EmuTOS `do_viewmenu` sort orders).
fn sort_entries(ents: &mut [tos_api::bdos::fs::find::DirInfo]) {
    use core::cmp::Ordering as O;
    let mode = SORT_MODE.load(core::sync::atomic::Ordering::Acquire);
    if mode == 4 {
        return; // No sort — keep on-disk order.
    }
    ents.sort_unstable_by(|a, b| {
        let (ad, bd) = (a.attrib & FA_SUBDIR != 0, b.attrib & FA_SUBDIR != 0);
        if ad != bd {
            return if ad { O::Less } else { O::Greater };
        }
        match mode {
            1 => name_ext(&a.name).cmp(name_ext(&b.name)).then_with(|| a.name.cmp(&b.name)),
            2 => a.size.cmp(&b.size),
            3 => (a.date, a.time).cmp(&(b.date, b.time)),
            _ => a.name.cmp(&b.name),
        }
    });
}

/// Draw `v` as a right-aligned decimal ending at `right_x`, in `fg`.
fn draw_u32_right_fg(backend: &dyn RasterBackend, right_x: i16, y: i16, mut v: u32, fg: u8, clip: ClipRect) {
    let mut digits = [0u8; 10];
    let mut k = 0;
    if v == 0 {
        digits[0] = b'0';
        k = 1;
    }
    while v > 0 && k < digits.len() {
        digits[k] = b'0' + (v % 10) as u8;
        v /= 10;
        k += 1;
    }
    let mut x = right_x - 8;
    for &d in &digits[..k] {
        backend.draw_glyph(x, y, tos_api::vdi::font::glyph(d as u16), fg, clip);
        x -= 8;
    }
}

/// Draw `v` as a right-aligned black decimal ending at `right_x`.
fn draw_u32_right(backend: &dyn RasterBackend, right_x: i16, y: i16, v: u32, clip: ClipRect) {
    draw_u32_right_fg(backend, right_x, y, v, BLACK, clip);
}

/// Directory view mode (View menu): 0 = text rows, 1 = icon grid.
static VIEW_MODE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

/// The currently selected entry: window handle (0 = none) and its index in
/// the *sorted* listing. Drives Delete / Info-Rename and the highlight.
static SEL_WIN: core::sync::atomic::AtomicI16 = core::sync::atomic::AtomicI16::new(0);
static SEL_IDX: core::sync::atomic::AtomicI16 = core::sync::atomic::AtomicI16::new(0);

/// Select the entry under `(mx, my)` if it lands in the top window's work
/// area. Returns true if a window click was consumed.
fn select_at(mx: i16, my: i16) -> bool {
    let h = DESK_TOP.load(core::sync::atomic::Ordering::Acquire);
    let rect = match win_slot(h) {
        Some(s) if s.drive != 0 => s.rect,
        _ => return false,
    };
    let (wx, wy, ww, wh) = work_of(rect);
    if mx < wx || mx >= wx + ww || my < wy || my >= wy + wh {
        return false;
    }
    let idx = if VIEW_MODE.load(core::sync::atomic::Ordering::Acquire) == 1 {
        let cols = (ww / 76).max(1);
        let col = ((mx - wx) / 76).clamp(0, cols - 1);
        let row = ((my - wy - 6) / 52).max(0);
        row * cols + col
    } else {
        ((my - wy - 4) / 16).max(0)
    };
    SEL_WIN.store(h, core::sync::atomic::Ordering::Release);
    SEL_IDX.store(idx, core::sync::atomic::Ordering::Release);
    redraw_win(h);
    true
}

/// Is entry `idx` of window `handle` the selected one?
fn is_selected(handle: i16, idx: usize) -> bool {
    SEL_WIN.load(core::sync::atomic::Ordering::Acquire) == handle
        && SEL_IDX.load(core::sync::atomic::Ordering::Acquire) == idx as i16
}

/// Set the view mode (View menu) and repaint every open directory window.
fn set_view(mode: u8) {
    VIEW_MODE.store(mode, core::sync::atomic::Ordering::Release);
    redraw_all_windows();
}

/// List `X:\*.*`, sort it, and draw it into the window's `work` rect as
/// either a text listing or an icon grid per [`VIEW_MODE`].
fn draw_dir_listing(handle: i16, letter: u8, work: (i16, i16, i16, i16)) {
    let Some(backend) = tos_api::host::host().and_then(|h| h.raster()) else {
        return;
    };
    let clip = ClipRect::full(backend.width(), backend.height());
    let (wx, wy, ww, wh) = work;
    backend.fill_rect(wx, wy, ww, wh, WHITE, clip);

    let path = [letter, b':', b'\\', b'*', b'.', b'*'];
    let path_str = core::str::from_utf8(&path).unwrap_or("C:\\*.*");
    let mut ents = [tos_api::bdos::fs::find::DirInfo::default(); 64];
    let n = tos_api::bdos::fs::find::list_dir_ex(path_str, &mut ents);
    sort_entries(&mut ents[..n]);

    if VIEW_MODE.load(core::sync::atomic::Ordering::Acquire) == 1 {
        draw_dir_icons(backend, handle, work, &ents[..n], clip);
    } else {
        draw_dir_text(backend, handle, work, &ents[..n], clip);
    }
    kprintln!("[desk] opened {}:\\ ({} entries)", letter as char, n);
}

/// Text view: one row per entry — name on the left, size (or `<DIR>`)
/// right-aligned; the selected row is inverted.
fn draw_dir_text(
    backend: &dyn RasterBackend, handle: i16, work: (i16, i16, i16, i16),
    ents: &[tos_api::bdos::fs::find::DirInfo], clip: ClipRect,
) {
    let (wx, wy, ww, wh) = work;
    let rows = (wh / 16).max(1) as usize;
    for (i, e) in ents.iter().take(rows).enumerate() {
        let ny = wy + 4 + i as i16 * 16;
        let sel = is_selected(handle, i);
        let ink = if sel {
            backend.fill_rect(wx, ny - 2, ww, 16, BLACK, clip);
            WHITE
        } else {
            BLACK
        };
        let end = e.name.iter().position(|&b| b == 0).unwrap_or(14);
        let mut cx = wx + 8;
        for &c in &e.name[..end] {
            backend.draw_glyph(cx, ny, tos_api::vdi::font::glyph(c as u16), ink, clip);
            cx += 8;
        }
        if e.attrib & FA_SUBDIR != 0 {
            let mut x = wx + ww - 8 - 5 * 8;
            for &c in b"<DIR>" {
                backend.draw_glyph(x, ny, tos_api::vdi::font::glyph(c as u16), ink, clip);
                x += 8;
            }
        } else if sel {
            draw_u32_right_fg(backend, wx + ww - 8, ny, e.size, WHITE, clip);
        } else {
            draw_u32_right(backend, wx + ww - 8, ny, e.size, clip);
        }
    }
}

/// Icon view: a grid of folder / document icons with the name underneath
/// (EmuTOS "Show as icons"); the selected name is inverted.
fn draw_dir_icons(
    backend: &dyn RasterBackend, handle: i16, work: (i16, i16, i16, i16),
    ents: &[tos_api::bdos::fs::find::DirInfo], clip: ClipRect,
) {
    use crate::desktop_icons as ic;
    let (wx, wy, ww, wh) = work;
    let cw = 76i16;
    let ch = 52i16;
    let cols = (ww / cw).max(1);
    for (i, e) in ents.iter().enumerate() {
        let col = i as i16 % cols;
        let row = i as i16 / cols;
        let cx = wx + col * cw + cw / 2;
        let cy = wy + 6 + row * ch;
        if cy + ch > wy + wh {
            break;
        }
        let (data, mask) = if e.attrib & FA_SUBDIR != 0 {
            (&ic::FOLDER_DATA, &ic::FOLDER_MASK)
        } else {
            (&ic::DOC_DATA, &ic::DOC_MASK)
        };
        blit_icon(backend, cx - 16, cy, data, mask, clip);
        // Centred name below, truncated to the cell width; selected inverts.
        let end = e.name.iter().position(|&b| b == 0).unwrap_or(14);
        let shown = end.min(9);
        let lx = cx - (shown as i16 * 8) / 2;
        let ink = if is_selected(handle, i) {
            backend.fill_rect(lx - 1, cy + 32, shown as i16 * 8 + 2, 16, BLACK, clip);
            WHITE
        } else {
            BLACK
        };
        for (j, &c) in e.name[..shown].iter().enumerate() {
            backend.draw_glyph(lx + j as i16 * 8, cy + 34, tos_api::vdi::font::glyph(c as u16), ink, clip);
        }
    }
}
