//! AES Object Manager demo: build a small dialog tree and render it
//! through the real GEM trap (`objc_draw`, opcode 42) so it is visible in
//! `cargo xtask run --window`.

use tos_api::aes::object::{BitBlk, IconBlk, Object, TedInfo};

use crate::kprintln;

const G_BOX: u16 = 20;
const G_TEXT: u16 = 21;
const G_BOXTEXT: u16 = 22;
const G_IMAGE: u16 = 23;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;
const G_ICON: u16 = 31;

const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const LASTOB: u16 = 0x0020;
const FL3DBAK: u16 = 0x0400; // 3D grey panel
const FL3DACT: u16 = 0x0600; // 3D raised button

/// Issue a GEM trap (selector 200 in `x0`, AESPB in `x1`).
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

/// Build a dialog tree and draw it with `objc_draw`.
pub fn run() {
    static TITLE: &[u8] = b"NewTOS AES Object Manager\0";
    static OK: &[u8] = b"OK\0";
    static CANCEL: &[u8] = b"Cancel\0";

    let box_spec: u64 = 1 << 16; // border thickness 1, colour word 0

    let tree = [
        // 0: dialog frame (root)
        Object {
            ob_next: -1, ob_head: 1, ob_tail: 3,
            ob_type: G_BOX, ob_flags: FL3DBAK, ob_state: 0, ob_spec: box_spec,
            ob_x: 160, ob_y: 130, ob_width: 320, ob_height: 160,
        },
        // 1: title string
        Object {
            ob_next: 2, ob_head: -1, ob_tail: -1,
            ob_type: G_STRING, ob_flags: 0, ob_state: 0, ob_spec: TITLE.as_ptr() as u64,
            ob_x: 24, ob_y: 24, ob_width: 272, ob_height: 16,
        },
        // 2: OK button (SELECTED → inverted look: black box, white text)
        Object {
            ob_next: 3, ob_head: -1, ob_tail: -1,
            ob_type: G_BUTTON, ob_flags: SELECTABLE | EXIT | FL3DACT | DEFAULT, ob_state: 0,
            ob_spec: OK.as_ptr() as u64,
            ob_x: 40, ob_y: 110, ob_width: 100, ob_height: 28,
        },
        // 3: Cancel button (last child → ob_next = parent 0; LASTOB)
        Object {
            ob_next: 0, ob_head: -1, ob_tail: -1,
            ob_type: G_BUTTON, ob_flags: SELECTABLE | EXIT | LASTOB | FL3DACT, ob_state: 0,
            ob_spec: CANCEL.as_ptr() as u64,
            ob_x: 180, ob_y: 110, ob_width: 100, ob_height: 28,
        },
    ];

    let mut control = [0i16; 8];
    let mut global = [0i16; 32];
    let mut intin = [0i16; 16];
    let mut intout = [0i16; 16];
    let mut addrin = [0i64; 4];
    let mut addrout = [0i64; 4];
    let block: [u64; 6] = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];

    control[0] = 42; // objc_draw
    control[3] = 1; // one addrin
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0; // drawob = root
    intin[1] = 8; // depth
    intin[2] = 0;
    intin[3] = 0;
    intin[4] = 640;
    intin[5] = 480; // clip = full screen
    aes(block.as_ptr() as u64);

    kprintln!(
        "[objc] dialog rendered via objc_draw, ret={} — view with 'cargo xtask run --window'",
        intout[0]
    );
}

/// Batch-2 demo: a real `form_alert` (opcode 52) — parses the alert
/// string, builds + draws the alert tree, then runs the modal loop
/// (press a key in the serial terminal to dismiss: Enter = default,
/// a digit = that button).
pub fn run_form_alert() {
    static ALERT: &[u8] = b"[2][This is a NewTOS|form_alert dialog!][OK|Cancel|Help]\0";

    let mut control = [0i16; 8];
    let mut global = [0i16; 32];
    let mut intin = [0i16; 16];
    let mut intout = [0i16; 16];
    let mut addrin = [0i64; 4];
    let mut addrout = [0i64; 4];
    let block: [u64; 6] = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];

    control[0] = 52; // form_alert
    control[1] = 1; // one intin
    control[3] = 1; // one addrin
    intin[0] = 1; // default button = 1 (OK)
    addrin[0] = ALERT.as_ptr() as i64;
    aes(block.as_ptr() as u64);

    kprintln!("[objc] form_alert returned button {}", intout[0]);
}

/// Batch-3 demo: a dialog with a `G_IMAGE` (16×16 "X") and a `G_ICON`
/// (32×32 frame + char + text), rendered via `objc_draw`.
pub fn run_icons() {
    // 16×16 monochrome "X" image (MSB = leftmost pixel).
    static XIMG: [u16; 16] = [
        0x8001, 0x4002, 0x2004, 0x1008, 0x0810, 0x0420, 0x0240, 0x0180,
        0x0180, 0x0240, 0x0420, 0x0810, 0x1008, 0x2004, 0x4002, 0x8001,
    ];
    static ITEXT: &[u8] = b"Info\0";
    static TITLE: &[u8] = b"Icons & Images\0";
    static IMGLBL: &[u8] = b"G_IMAGE\0";

    // 32×32 icon: data = a frame, mask = solid (2 words/row).
    let mut idata = [0u16; 64];
    idata[0] = 0xFFFF;
    idata[1] = 0xFFFF;
    for r in 1..31 {
        idata[r * 2] = 0x8000;
        idata[r * 2 + 1] = 0x0001;
    }
    idata[62] = 0xFFFF;
    idata[63] = 0xFFFF;
    let imask = [0xFFFFu16; 64];

    let bi = BitBlk {
        bi_pdata: XIMG.as_ptr() as u64,
        bi_wb: 2, bi_hl: 16, bi_x: 0, bi_y: 0, bi_color: 1, // black
    };
    let ib = IconBlk {
        ib_pmask: imask.as_ptr() as u64,
        ib_pdata: idata.as_ptr() as u64,
        ib_ptext: ITEXT.as_ptr() as u64,
        ib_char: (1 << 12) | b'i' as i16, // fg=black, bg=white, char 'i'
        ib_xchar: 12, ib_ychar: 9,
        ib_xicon: 0, ib_yicon: 0, ib_wicon: 32, ib_hicon: 32,
        ib_xtext: 0, ib_ytext: 36, ib_wtext: 32, ib_htext: 16,
    };

    let tree = [
        Object { ob_next: -1, ob_head: 1, ob_tail: 4, ob_type: G_BOX, ob_flags: FL3DBAK,
                 ob_state: 0, ob_spec: 1 << 16, ob_x: 150, ob_y: 140, ob_width: 340, ob_height: 190 },
        Object { ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
                 ob_state: 0, ob_spec: TITLE.as_ptr() as u64, ob_x: 20, ob_y: 18, ob_width: 240, ob_height: 16 },
        Object { ob_next: 3, ob_head: -1, ob_tail: -1, ob_type: G_IMAGE, ob_flags: 0,
                 ob_state: 0, ob_spec: &bi as *const _ as u64, ob_x: 60, ob_y: 70, ob_width: 16, ob_height: 16 },
        Object { ob_next: 4, ob_head: -1, ob_tail: -1, ob_type: G_ICON, ob_flags: 0,
                 ob_state: 0, ob_spec: &ib as *const _ as u64, ob_x: 220, ob_y: 60, ob_width: 32, ob_height: 56 },
        Object { ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0x0020, // LASTOB
                 ob_state: 0, ob_spec: IMGLBL.as_ptr() as u64, ob_x: 44, ob_y: 96, ob_width: 64, ob_height: 16 },
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
    control[0] = 42; // objc_draw
    control[3] = 1;
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0;
    intin[1] = 8;
    intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);

    kprintln!("[objc] icons/images demo rendered, ret={}", intout[0]);
}

/// Batch-4 demo: TEDINFO text — a `G_TEXT` (red, centred) and a
/// `G_BOXTEXT` (boxed left-justified), rendered via `objc_draw`.
pub fn run_tedinfo() {
    static T1: &[u8] = b"Red centered TEDINFO text\0";
    static T2: &[u8] = b"Boxed G_BOXTEXT field\0";
    static TITLE: &[u8] = b"TEDINFO (G_TEXT / G_BOXTEXT)\0";
    static EMPTY: &[u8] = b"\0";

    let ted1 = TedInfo {
        te_ptext: T1.as_ptr() as u64, te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64, te_font: 3, te_junk1: 0,
        te_just: 2, te_color: 2 << 8, te_junk2: 0, te_thickness: 0,
        te_txtlen: 0, te_tmplen: 0, // red text, centred
    };
    let ted2 = TedInfo {
        te_ptext: T2.as_ptr() as u64, te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64, te_font: 3, te_junk1: 0,
        te_just: 0, te_color: 1 << 8, te_junk2: 0, te_thickness: 1,
        te_txtlen: 0, te_tmplen: 0, // black text, left, 1px box
    };

    let tree = [
        Object { ob_next: -1, ob_head: 1, ob_tail: 3, ob_type: G_BOX, ob_flags: FL3DBAK,
                 ob_state: 0, ob_spec: 1 << 16, ob_x: 140, ob_y: 150, ob_width: 360, ob_height: 170 },
        Object { ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
                 ob_state: 0, ob_spec: TITLE.as_ptr() as u64, ob_x: 20, ob_y: 18, ob_width: 320, ob_height: 16 },
        Object { ob_next: 3, ob_head: -1, ob_tail: -1, ob_type: G_TEXT, ob_flags: 0,
                 ob_state: 0, ob_spec: &ted1 as *const _ as u64, ob_x: 20, ob_y: 60, ob_width: 320, ob_height: 16 },
        Object { ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_BOXTEXT, ob_flags: 0x0020,
                 ob_state: 0, ob_spec: &ted2 as *const _ as u64, ob_x: 40, ob_y: 100, ob_width: 280, ob_height: 28 },
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
    control[0] = 42; control[3] = 1;
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0; intin[1] = 8;
    intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] TEDINFO demo rendered, ret={}", intout[0]);
}

/// Batch-5 demo: `form_do` — draw a dialog (objc_draw), then run the
/// modal loop (click a button, or Enter for the default OK). Prints the
/// exit object index.
pub fn run_form_do() {
    const DEFAULT: u16 = 0x0002;
    static TITLE: &[u8] = b"form_do: click a button (or Enter)\0";
    static OK: &[u8] = b"OK\0";
    static CANCEL: &[u8] = b"Cancel\0";

    let tree = [
        Object { ob_next: -1, ob_head: 1, ob_tail: 3, ob_type: G_BOX, ob_flags: 0,
                 ob_state: 0, ob_spec: 1 << 16, ob_x: 160, ob_y: 140, ob_width: 320, ob_height: 160 },
        Object { ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
                 ob_state: 0, ob_spec: TITLE.as_ptr() as u64, ob_x: 24, ob_y: 24, ob_width: 280, ob_height: 16 },
        Object { ob_next: 3, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | DEFAULT, ob_state: 0,
                 ob_spec: OK.as_ptr() as u64, ob_x: 40, ob_y: 110, ob_width: 100, ob_height: 28 },
        Object { ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | LASTOB, ob_state: 0,
                 ob_spec: CANCEL.as_ptr() as u64, ob_x: 180, ob_y: 110, ob_width: 100, ob_height: 28 },
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

    // objc_draw the dialog
    control[0] = 42; control[3] = 1;
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0; intin[1] = 8; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);

    // form_do — modal loop
    control = [0; 8];
    control[0] = 50; control[1] = 1; control[3] = 1;
    intin[0] = 0; // start object
    addrin[0] = tree.as_ptr() as i64;
    aes(block.as_ptr() as u64);

    kprintln!("[objc] form_do returned object {}", intout[0]);
}

/// Batch-6 demo: an editable `G_BOXTEXT` field. Draw it, then drive
/// `objc_edit` (EDINIT + EDCHAR) to type "NewTOS!" into it.
pub fn run_edit() {
    const EDITABLE: u16 = 0x0008;
    static TITLE: &[u8] = b"objc_edit - editable TEDINFO field\0";
    static EMPTY: &[u8] = b"\0";

    let mut textbuf = [0u8; 24]; // mutable, initially empty
    let ted = TedInfo {
        te_ptext: textbuf.as_mut_ptr() as u64, te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64, te_font: 3, te_junk1: 0, te_just: 0,
        te_color: 1 << 8, te_junk2: 0, te_thickness: 1, te_txtlen: 24, te_tmplen: 0,
    };
    let tree = [
        Object { ob_next: -1, ob_head: 1, ob_tail: 2, ob_type: G_BOX, ob_flags: 0,
                 ob_state: 0, ob_spec: 1 << 16, ob_x: 140, ob_y: 150, ob_width: 360, ob_height: 150 },
        Object { ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
                 ob_state: 0, ob_spec: TITLE.as_ptr() as u64, ob_x: 24, ob_y: 24, ob_width: 320, ob_height: 16 },
        Object { ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_BOXTEXT,
                 ob_flags: EDITABLE | 0x0020, ob_state: 0, ob_spec: &ted as *const _ as u64,
                 ob_x: 40, ob_y: 70, ob_width: 200, ob_height: 24 },
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

    // objc_draw the dialog
    control[0] = 42; control[3] = 1;
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0; intin[1] = 8; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);

    // objc_edit EDINIT (field = object 2)
    control = [0; 8]; control[0] = 46; control[3] = 1;
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 2; intin[1] = 0; intin[2] = 0; intin[3] = 1; // EDINIT
    aes(block.as_ptr() as u64);
    let mut idx = intout[1];

    // type "NewTOS!" via EDCHAR
    for &c in b"NewTOS!" {
        control = [0; 8]; control[0] = 46; control[3] = 1;
        addrin[0] = tree.as_ptr() as i64;
        intin[0] = 2; intin[1] = c as i16; intin[2] = idx; intin[3] = 2; // EDCHAR
        aes(block.as_ptr() as u64);
        idx = intout[1];
    }

    kprintln!("[objc] objc_edit typed text, cursor idx={}", idx);
}

/// Batch-7 demo: form_do with an editable field. Draw a dialog (text
/// field + OK), then form_do focuses the field and routes keystrokes to
/// it; Enter activates the default OK button.
pub fn run_form_edit() {
    const EDITABLE: u16 = 0x0008;
    const DEFAULT: u16 = 0x0002;
    static TITLE: &[u8] = b"form_do + objc_edit: type, then Enter\0";
    static OK: &[u8] = b"OK\0";
    static EMPTY: &[u8] = b"\0";

    let mut textbuf = [0u8; 24];
    let ted = TedInfo {
        te_ptext: textbuf.as_mut_ptr() as u64, te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64, te_font: 3, te_junk1: 0, te_just: 0,
        te_color: 1 << 8, te_junk2: 0, te_thickness: 1, te_txtlen: 24, te_tmplen: 0,
    };
    let tree = [
        Object { ob_next: -1, ob_head: 1, ob_tail: 3, ob_type: G_BOX, ob_flags: FL3DBAK,
                 ob_state: 0, ob_spec: 1 << 16, ob_x: 140, ob_y: 150, ob_width: 360, ob_height: 170 },
        Object { ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
                 ob_state: 0, ob_spec: TITLE.as_ptr() as u64, ob_x: 24, ob_y: 20, ob_width: 320, ob_height: 16 },
        Object { ob_next: 3, ob_head: -1, ob_tail: -1, ob_type: G_BOXTEXT, ob_flags: EDITABLE,
                 ob_state: 0, ob_spec: &ted as *const _ as u64, ob_x: 40, ob_y: 56, ob_width: 200, ob_height: 24 },
        Object { ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | DEFAULT | LASTOB, ob_state: 0,
                 ob_spec: OK.as_ptr() as u64, ob_x: 130, ob_y: 110, ob_width: 100, ob_height: 28 },
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

    control[0] = 42; control[3] = 1; // objc_draw
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0; intin[1] = 8; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);

    control = [0; 8]; control[0] = 50; control[1] = 1; control[3] = 1; // form_do
    intin[0] = 0; addrin[0] = tree.as_ptr() as i64;
    aes(block.as_ptr() as u64);

    let n = textbuf.iter().position(|&b| b == 0).unwrap_or(0);
    kprintln!("[objc] form_do+edit returned obj {}, field=\"{}\"",
        intout[0], core::str::from_utf8(&textbuf[..n]).unwrap_or("?"));
}

/// Batch-7 demo: form_error — a localised DOS-error alert (errnum -33 =
/// "File not found.").
pub fn run_form_error() {
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
    control[0] = 53; control[1] = 1; // form_error
    intin[0] = -33; // EFILNF
    aes(block.as_ptr() as u64);
    kprintln!("[objc] form_error returned button {}", intout[0]);
}

/// D5 Batch-1 demo: a menu bar (Desk/File/Edit) via menu_bar, with the
/// "File" title highlighted via menu_tnormal.
pub fn run_menu() {
    const G_IBOX: u16 = 25;
    const G_TITLE: u16 = 32;
    static DESK: &[u8] = b"Desk\0";
    static FILE: &[u8] = b"File\0";
    static EDIT: &[u8] = b"Edit\0";

    let tree = [
        // 0: ROOT
        Object { ob_next: -1, ob_head: 1, ob_tail: 1, ob_type: G_IBOX, ob_flags: 0,
                 ob_state: 0, ob_spec: 0, ob_x: 0, ob_y: 0, ob_width: 640, ob_height: 19 },
        // 1: THEBAR (white box, full width)
        Object { ob_next: 0, ob_head: 2, ob_tail: 4, ob_type: G_BOX, ob_flags: 0,
                 ob_state: 0, ob_spec: 0, ob_x: 0, ob_y: 0, ob_width: 640, ob_height: 19 },
        // 2..4: titles
        Object { ob_next: 3, ob_head: -1, ob_tail: -1, ob_type: G_TITLE, ob_flags: 0,
                 ob_state: 0, ob_spec: DESK.as_ptr() as u64, ob_x: 0, ob_y: 0, ob_width: 56, ob_height: 19 },
        Object { ob_next: 4, ob_head: -1, ob_tail: -1, ob_type: G_TITLE, ob_flags: 0,
                 ob_state: 0, ob_spec: FILE.as_ptr() as u64, ob_x: 56, ob_y: 0, ob_width: 56, ob_height: 19 },
        Object { ob_next: 1, ob_head: -1, ob_tail: -1, ob_type: G_TITLE, ob_flags: 0x0020,
                 ob_state: 0, ob_spec: EDIT.as_ptr() as u64, ob_x: 112, ob_y: 0, ob_width: 56, ob_height: 19 },
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

    // menu_bar(tree, show=1)
    control[0] = 30; control[1] = 1; control[3] = 1;
    intin[0] = 1; addrin[0] = tree.as_ptr() as i64;
    aes(block.as_ptr() as u64);

    // menu_tnormal(title=3 "File", normal=0 → highlight)
    control = [0; 8]; control[0] = 33; control[1] = 2; control[3] = 1;
    intin[0] = 3; intin[1] = 0; addrin[0] = tree.as_ptr() as i64;
    aes(block.as_ptr() as u64);

    let _ = G_IBOX;
    kprintln!("[objc] menu bar drawn (Desk/File/Edit, File highlighted)");
}

/// D5 Batch-2 demo: a full menu tree (Desk/File/Edit, each with a
/// drop-down) drawn via menu_bar, with the File menu opened and "Open"
/// highlighted — the rendering the interactive `track` shows mid-drag.
pub fn run_menu_open() {
    const G_IBOX: u16 = 25;
    const G_TITLE: u16 = 32;
    const LAST: u16 = 0x0020;
    static DESK: &[u8] = b"Desk\0";
    static FILE: &[u8] = b"File\0";
    static EDIT: &[u8] = b"Edit\0";
    static ABOUT: &[u8] = b"About NewTOS\0";
    static INFO: &[u8] = b"Desk Info\0";
    static NEW: &[u8] = b"New\0";
    static OPEN: &[u8] = b"Open\0";
    static QUIT: &[u8] = b"Quit\0";
    static CUT: &[u8] = b"Cut\0";
    static COPY: &[u8] = b"Copy\0";

    let t = |ty, fl, spec: u64, x, y, w, h, next, head, tail| Object {
        ob_next: next, ob_head: head, ob_tail: tail, ob_type: ty, ob_flags: fl,
        ob_state: 0, ob_spec: spec, ob_x: x, ob_y: y, ob_width: w, ob_height: h,
    };
    let s = |p: &[u8], x, y, next, last| t(G_STRING, if last { LAST } else { 0 },
        p.as_ptr() as u64, x, y, 112, 19, next, -1, -1);

    let tree = [
        /* 0 ROOT     */ t(G_IBOX, 0, 0, 0, 0, 640, 480, -1, 1, 6),
        /* 1 THEBAR   */ t(G_BOX, 0, 0, 0, 0, 640, 19, 6, 2, 2),
        /* 2 THEACTIVE*/ t(G_IBOX, 0, 0, 0, 0, 640, 19, 1, 3, 5),
        /* 3 Desk     */ t(G_TITLE, 0, DESK.as_ptr() as u64, 0, 0, 56, 19, 4, -1, -1),
        /* 4 File     */ t(G_TITLE, 0, FILE.as_ptr() as u64, 56, 0, 56, 19, 5, -1, -1),
        /* 5 Edit     */ t(G_TITLE, 0, EDIT.as_ptr() as u64, 112, 0, 56, 19, 2, -1, -1),
        /* 6 themenus */ t(G_IBOX, 0, 0, 0, 0, 640, 480, 0, 7, 14),
        /* 7 Desk-box */ t(G_BOX, 0, 1 << 16, 0, 19, 112, 38, 10, 8, 9),
        /* 8  About   */ s(ABOUT, 0, 0, 9, false),
        /* 9  Info    */ s(INFO, 0, 19, 7, false),
        /*10 File-box */ t(G_BOX, 0, 1 << 16, 56, 19, 112, 57, 14, 11, 13),
        /*11  New     */ s(NEW, 0, 0, 12, false),
        /*12  Open    */ s(OPEN, 0, 19, 13, false),
        /*13  Quit    */ s(QUIT, 0, 38, 10, false),
        /*14 Edit-box */ t(G_BOX, 0, 1 << 16, 112, 19, 112, 38, 6, 15, 16),
        /*15  Cut     */ s(CUT, 0, 0, 16, false),
        /*16  Copy    */ s(COPY, 0, 19, 14, true),
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
    // menu_bar(tree, show=1)
    control[0] = 30; control[1] = 1; control[3] = 1;
    intin[0] = 1; addrin[0] = tree.as_ptr() as i64;
    aes(block.as_ptr() as u64);

    // Programmatically open File (4) and highlight "Open" (12).
    if let Some(backend) = tos_api::host::host().and_then(|h| h.raster()) {
        let tp = tree.as_ptr() as *mut Object;
        let box_idx = tos_api::aes::menu::open(tp, 4, 12, backend);
        kprintln!("[objc] File menu opened (box obj {}), 'Open' highlighted", box_idx);
    }
}

/// D4 Batch-1 demo: create + open a full-frame window (title, closer,
/// fuller, mover, info, scroll bars, sizer) via wind_create/set/open.
pub fn run_window() {
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
    static NAME: &[u8] = b"NewTOS Window\0";
    static INFO: &[u8] = b"Ready.\0";

    // wind_create(kind = all gadgets, full extent = screen)
    control[0] = 100; control[1] = 5; control[3] = 0;
    intin[0] = 0x0FFF; intin[1] = 0; intin[2] = 0; intin[3] = 640; intin[4] = 480;
    aes(block.as_ptr() as u64);
    let handle = intout[0];

    // wind_set(handle, WF_NAME, "NewTOS Window")
    control = [0; 8]; control[0] = 105; control[1] = 2; control[3] = 1;
    intin[0] = handle; intin[1] = 2; addrin[0] = NAME.as_ptr() as i64;
    aes(block.as_ptr() as u64);
    // wind_set(handle, WF_INFO, "Ready.")
    control = [0; 8]; control[0] = 105; control[1] = 2; control[3] = 1;
    intin[0] = handle; intin[1] = 3; addrin[0] = INFO.as_ptr() as i64;
    aes(block.as_ptr() as u64);

    // wind_open(handle, rect)
    control = [0; 8]; control[0] = 101; control[1] = 5; control[3] = 0;
    intin[0] = handle; intin[1] = 80; intin[2] = 60; intin[3] = 440; intin[4] = 320;
    aes(block.as_ptr() as u64);

    kprintln!("[objc] window opened, handle {}", handle);

    // D4 Batch-2 logic check: gadget hit-test at known frame points.
    let probe = |x: i16, y: i16| tos_api::aes::window::gadget_name_at(handle, x, y);
    kprintln!("[objc] hit (88,68)={} (510,68)={} (300,68)={}",
        probe(88, 68), probe(510, 68), probe(300, 68));
    kprintln!("[objc] hit (510,370)={} (510,88)={} (300,200)={}",
        probe(510, 370), probe(510, 88), probe(300, 200));
}

/// D4 Batch-3a check: graf_handle (VDI handle + font metrics) and
/// graf_mkstate (instantaneous mouse poll), the non-interactive graf_*.
pub fn run_graf() {
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
    // graf_handle (77)
    control[0] = 77;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] graf_handle: vdi={} wchar={} hchar={} wbox={} hbox={}",
        intout[0], intout[1], intout[2], intout[3], intout[4]);
    // graf_mkstate (79)
    control = [0; 8]; control[0] = 79;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] graf_mkstate: mx={} my={} mstate={} kstate={}",
        intout[1], intout[2], intout[3], intout[4]);
}

/// D4 Batch-3b demo (toward EC1): open two overlapping windows (B on top
/// of A), then walk A's visible rectangle list (work area minus B).
pub fn run_two_windows() {
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
    static NAME_A: &[u8] = b"Window A\0";
    static NAME_B: &[u8] = b"Window B\0";

    let mut create = |x, y, w, h, name: &[u8]| -> i16 {
        control = [0; 8]; control[0] = 100; control[1] = 5;
        intin[0] = 0x0FFF; intin[1] = 0; intin[2] = 0; intin[3] = 640; intin[4] = 480;
        aes(block.as_ptr() as u64);
        let hdl = intout[0];
        control = [0; 8]; control[0] = 105; control[1] = 2; control[3] = 1;
        intin[0] = hdl; intin[1] = 2; addrin[0] = name.as_ptr() as i64;
        aes(block.as_ptr() as u64);
        control = [0; 8]; control[0] = 101; control[1] = 5;
        intin[0] = hdl; intin[1] = x; intin[2] = y; intin[3] = w; intin[4] = h;
        aes(block.as_ptr() as u64);
        hdl
    };

    let ha = create(60, 50, 360, 250, NAME_A);
    let hb = create(230, 170, 360, 250, NAME_B);
    kprintln!("[objc] two windows: A={} B={} (B on top)", ha, hb);

    // Walk A's visible rectangle list (work area minus B).
    control = [0; 8]; control[0] = 104; control[1] = 2;
    intin[0] = ha; intin[1] = 11; // WF_FIRSTXYWH
    aes(block.as_ptr() as u64);
    let mut piece = 0;
    while intout[3] != 0 && piece < 8 {
        kprintln!("[objc]   A piece {}: x={} y={} w={} h={}",
            piece, intout[1], intout[2], intout[3], intout[4]);
        piece += 1;
        control = [0; 8]; control[0] = 104; control[1] = 2;
        intin[0] = ha; intin[1] = 12; // WF_NEXTXYWH
        aes(block.as_ptr() as u64);
    }
    kprintln!("[objc] A visible pieces: {}", piece);
}

/// P5 (QED-redraw #123) proof — NOT wired into boot. Opens two overlapping AES
/// windows (A bottom, B top), then (1) closes B and (2) re-tops A, asserting
/// each time that a `WM_REDRAW` (msg type 20) was queued for the newly-exposed
/// window A with its work-area rectangle. Drives the real GEM trap end-to-end;
/// the message is read back through `window::test_pending_msg`.
///
/// Invoke by hand (temporary hook before `desk::run()`) to capture the serial
/// PASS/FAIL lines; it is intentionally not called during normal boot.
#[allow(unused_assignments)] // the `control = [0; 8]` reset idiom, as elsewhere here
pub fn run_p5_redraw() {
    use tos_api::aes::window::{test_clear_msg, test_pending_msg};

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
    let bp = block.as_ptr() as u64;

    // Clean window table, then register the pd so WM_REDRAW has a target queue.
    control = [0; 8]; control[0] = 109; aes(bp); // wind_new
    control = [0; 8]; control[0] = 10; aes(bp);  // appl_init
    let ap_id = intout[0];

    // Window A (bottom): create (all gadgets) + open at (60,50,360,250).
    control = [0; 8]; control[0] = 100; control[1] = 5;
    intin[0] = 0x0FFF; intin[1] = 0; intin[2] = 0; intin[3] = 640; intin[4] = 480;
    aes(bp);
    let ha = intout[0];
    control = [0; 8]; control[0] = 101; control[1] = 5;
    intin[0] = ha; intin[1] = 60; intin[2] = 50; intin[3] = 360; intin[4] = 250;
    aes(bp);

    // Window B (top): create + open at (230,170,360,250) — overlaps A.
    control = [0; 8]; control[0] = 100; control[1] = 5;
    intin[0] = 0x0FFF; intin[1] = 0; intin[2] = 0; intin[3] = 640; intin[4] = 480;
    aes(bp);
    let hb = intout[0];
    control = [0; 8]; control[0] = 101; control[1] = 5;
    intin[0] = hb; intin[1] = 230; intin[2] = 170; intin[3] = 360; intin[4] = 250;
    aes(bp);

    // Expected redraw rect = A's work area (wind_get WF_WXYWH = field 4).
    control = [0; 8]; control[0] = 104; control[1] = 2;
    intin[0] = ha; intin[1] = 4;
    aes(bp);
    let (ewx, ewy, eww, ewh) = (intout[1], intout[2], intout[3], intout[4]);
    kprintln!(
        "[p5] ap_id={} A={} B={} (B on top); A work=({},{},{},{})",
        ap_id, ha, hb, ewx, ewy, eww, ewh
    );

    // ---- Case 1: wind_close(B) must post WM_REDRAW to A ----
    test_clear_msg(ap_id);
    control = [0; 8]; control[0] = 102; control[1] = 1;
    intin[0] = hb;
    aes(bp);
    report_p5("close", test_pending_msg(ap_id), ha, ewx, ewy, eww, ewh);

    // ---- Case 2: re-open B on top, then wind_set(A, WF_TOP) posts WM_REDRAW ----
    control = [0; 8]; control[0] = 101; control[1] = 5;
    intin[0] = hb; intin[1] = 230; intin[2] = 170; intin[3] = 360; intin[4] = 250;
    aes(bp);
    test_clear_msg(ap_id);
    control = [0; 8]; control[0] = 105; control[1] = 2;
    intin[0] = ha; intin[1] = 10; // WF_TOP
    aes(bp);
    report_p5("top", test_pending_msg(ap_id), ha, ewx, ewy, eww, ewh);

    // Leave the window table clean for whatever runs next.
    control = [0; 8]; control[0] = 109; aes(bp); // wind_new
}

/// Print a P5 selftest verdict line: the queued message and whether it is the
/// expected `WM_REDRAW` (type 20) for window `hexp` with rect `(x,y,w,h)`.
#[allow(clippy::too_many_arguments)]
fn report_p5(
    tag: &str,
    msg: Option<[i16; 8]>,
    hexp: i16,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
) {
    match msg {
        Some(m) => {
            let ok = m[0] == 20 && m[3] == hexp && m[4] == x && m[5] == y && m[6] == w && m[7] == h;
            kprintln!(
                "[p5] {}: msg=[{},{},{},{},{},{},{},{}] -> {}",
                tag, m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7],
                if ok { "PASS" } else { "FAIL" }
            );
        }
        None => kprintln!("[p5] {}: no WM_REDRAW queued -> FAIL", tag),
    }
}

/// P6 (#125 message-queue) proof — NOT wired into boot. Drives the real GEM
/// trap to prove the per-app FIFO message queue that replaced the single slot:
///
///  (a) Post TWO different-type messages (WM_REDRAW then WM_TOPPED) to one app
///      via `appl_write` (opcode 12), then drain them via successive
///      `evnt_mesag` (opcode 23) calls: BOTH are delivered, in FIFO order,
///      neither dropped. Before the fix the 2nd post overwrote the 1st — this
///      is the exact intermittent-empty-window bug.
///  (b) Post TWO WM_REDRAWs for the SAME window with different rects: the queue
///      coalesces them into ONE entry whose rect is the UNION of the two
///      (EmuTOS `doq` rect-union, `gemqueue.c:45-56`), not two entries and not
///      just the last rect.
///
/// Invoke by hand (temporary hook, e.g. in `initinfo::run()`) to capture the
/// serial PASS/FAIL lines; it is intentionally not called during normal boot.
#[allow(unused_assignments)] // the `control = [0; 8]` reset idiom, as elsewhere here
pub fn run_p6_msgqueue() {
    use tos_api::aes::window::{test_clear_msg, test_msg_count, test_pending_msg};

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
    let bp = block.as_ptr() as u64;

    // Clean window table + register the pd so the queue has an owner.
    control = [0; 8]; control[0] = 109; aes(bp); // wind_new
    control = [0; 8]; control[0] = 10; aes(bp);  // appl_init
    let ap = intout[0];

    // Raw pointers to the parameter-block arrays (Copy), so the two trap
    // helpers below can share them without re-borrowing the arrays.
    let c = control.as_mut_ptr();
    let ii = intin.as_mut_ptr();
    let ai = addrin.as_mut_ptr();

    // `appl_write` (opcode 12) a 16-byte message into `ap`'s own queue.
    let post = |target: i16, msg: &[i16; 8]| {
        // SAFETY: block pointers alias these arrays; the trap reads them.
        unsafe {
            for k in 0..8 { c.add(k).write(0); }
            c.write(12); c.add(1).write(2); c.add(3).write(1); // op, n_intin, n_addrin
            ii.write(target); ii.add(1).write(16); // id, byte length
            ai.write(msg.as_ptr() as i64);
        }
        aes(bp);
    };
    // `evnt_mesag` (opcode 23): pop the front message into `out`.
    let read = |out: &mut [i16; 8]| {
        // SAFETY: as above.
        unsafe {
            for k in 0..8 { c.add(k).write(0); }
            c.write(23); c.add(3).write(1); // op, n_addrin
            ai.write(out.as_ptr() as i64);
        }
        aes(bp);
    };

    // ---- (a) two different-type messages both survive, FIFO ----
    test_clear_msg(ap);
    let m1 = [20i16, ap, 0, 1, 10, 20, 30, 40]; // WM_REDRAW, win 1
    let m2 = [21i16, ap, 0, 1, 0, 0, 0, 0]; // WM_TOPPED, win 1
    post(ap, &m1);
    post(ap, &m2);
    let queued = test_msg_count(ap);
    let mut o1 = [0i16; 8];
    let mut o2 = [0i16; 8];
    // Guard the blocking `evnt_mesag` with the count so a broken build reports
    // FAIL instead of hanging on an empty pop.
    if test_msg_count(ap) > 0 { read(&mut o1); }
    if test_msg_count(ap) > 0 { read(&mut o2); }
    let fifo_ok = queued == 2 && o1[0] == 20 && o1[3] == 1 && o2[0] == 21 && o2[3] == 1;
    kprintln!(
        "[p6] fifo: queued={} first=[{},h{}] second=[{},h{}] -> {}",
        queued, o1[0], o1[3], o2[0], o2[3],
        if fifo_ok { "PASS" } else { "FAIL" }
    );

    // ---- (b) two same-window WM_REDRAWs coalesce to one union rect ----
    test_clear_msg(ap);
    let r1 = [20i16, ap, 0, 7, 10, 20, 30, 40]; // rect (10,20,30,40)
    let r2 = [20i16, ap, 0, 7, 50, 5, 20, 100]; // rect (50, 5,20,100)
    post(ap, &r1);
    post(ap, &r2);
    let count = test_msg_count(ap);
    let front = test_pending_msg(ap);
    // union: x=min(10,50)=10 y=min(20,5)=5 w=max(40,70)-10=60 h=max(60,105)-5=100
    let union_ok = count == 1
        && matches!(front, Some(m)
            if m[0] == 20 && m[3] == 7 && m[4] == 10 && m[5] == 5 && m[6] == 60 && m[7] == 100);
    match front {
        Some(m) => kprintln!(
            "[p6] union: count={} rect=({},{},{},{}) exp=(10,5,60,100) -> {}",
            count, m[4], m[5], m[6], m[7],
            if union_ok { "PASS" } else { "FAIL" }
        ),
        None => kprintln!("[p6] union: count={} no message -> FAIL", count),
    }

    // Leave the window table clean for whatever runs next.
    test_clear_msg(ap);
    control = [0; 8]; control[0] = 109; aes(bp); // wind_new
}

/// D7 Batch-1 demo: build a native .RSC blob (string pointers as offsets),
/// relocate it (offset→pointer), fetch its tree via rsrc_gaddr, and draw
/// it via objc_draw — proving the native resource format end-to-end.
pub fn run_rsrc() {
    use tos_api::aes::resource::NrscHeader;
    let mut buf = [0u8; 512];
    let base = buf.as_mut_ptr();

    // String data at offset 256.
    let copy = |buf: &mut [u8; 512], off: usize, s: &[u8]| {
        buf[off..off + s.len()].copy_from_slice(s);
    };
    let s_title = 256usize;
    let s_ok = 320usize;
    copy(&mut buf, s_title, b"Dialog loaded from .RSC\0");
    copy(&mut buf, s_ok, b"OK\0");

    // SAFETY: building the blob in our own buffer.
    unsafe {
        let h = base as *mut NrscHeader;
        (*h).magic = *b"NRSC";
        (*h).version = 1;
        (*h).n_trees = 1;
        (*h).n_objects = 3;
        (*h).off_trindex = 160;
        (*h).off_objects = 32;
        (*h).total_size = 512;
        // tree index: root = object 0
        *(base.add(160) as *mut u32) = 0;
        // objects at offset 32
        let objs = base.add(32) as *mut Object;
        objs.add(0).write(Object {
            ob_next: -1, ob_head: 1, ob_tail: 2, ob_type: G_BOX, ob_flags: FL3DBAK,
            ob_state: 0, ob_spec: 1 << 16, ob_x: 170, ob_y: 150, ob_width: 300, ob_height: 130,
        });
        objs.add(1).write(Object {
            ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
            ob_state: 0, ob_spec: s_title as u64, ob_x: 24, ob_y: 28, ob_width: 250, ob_height: 16,
        });
        objs.add(2).write(Object {
            ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
            ob_flags: SELECTABLE | EXIT | LASTOB | FL3DACT | DEFAULT, ob_state: 0,
            ob_spec: s_ok as u64, ob_x: 100, ob_y: 90, ob_width: 100, ob_height: 28,
        });

        if !tos_api::aes::resource::relocate_and_register(base) {
            kprintln!("[objc] rsrc relocate FAILED");
            return;
        }
    }

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

    // rsrc_gaddr(R_TREE=0, index=0) → addrout[0] = tree
    control[0] = 112; control[1] = 2;
    intin[0] = 0; intin[1] = 0;
    aes(block.as_ptr() as u64);
    let tree = addrout[0];
    kprintln!("[objc] rsrc_gaddr R_TREE 0 -> {:#x} (ret {})", tree as u64, intout[0]);

    // objc_draw the loaded tree
    control = [0; 8]; control[0] = 42; control[3] = 1;
    addrin[0] = tree;
    intin[0] = 0; intin[1] = 8; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);

    // rsrc_free
    control = [0; 8]; control[0] = 111;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] .RSC dialog drawn + freed");
}

/// D7 Batch-2 demo: load TEST.RSC from the SD card via rsrc_load (searches
/// C:\GEMSYS\RSC\ then C:\), fetch its tree, and draw it.
pub fn run_rsrc_file() {
    static NAME: &[u8] = b"TEST.RSC\0";
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
    // rsrc_load("TEST.RSC")
    control[0] = 110; control[3] = 1;
    addrin[0] = NAME.as_ptr() as i64;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] rsrc_load(TEST.RSC) -> {}", intout[0]);
    if intout[0] == 0 {
        kprintln!("[objc] .RSC not loaded (no SD file?)");
        return;
    }
    // rsrc_gaddr(R_TREE 0)
    control = [0; 8]; control[0] = 112; control[1] = 2;
    intin[0] = 0; intin[1] = 0;
    aes(block.as_ptr() as u64);
    let tree = addrout[0];
    // objc_draw
    control = [0; 8]; control[0] = 42; control[3] = 1;
    addrin[0] = tree;
    intin[0] = 0; intin[1] = 8; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] .RSC from SD drawn");
}

/// D7 Batch-3 demo: load the converted EmuTOS GEM.RSC from SD and draw
/// tree 0 (the FSELECTR file-selector dialog) — proving classic→native
/// conversion + load + render.
pub fn run_rsrc_gem() {
    static NAME: &[u8] = b"GEM.RSC\0";
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
    control[0] = 110; control[3] = 1;
    addrin[0] = NAME.as_ptr() as i64;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] rsrc_load(GEM.RSC) -> {}", intout[0]);
    if intout[0] == 0 { return; }
    control = [0; 8]; control[0] = 112; control[1] = 2;
    intin[0] = 0; intin[1] = 0; // R_TREE, tree 0 (FSELECTR)
    aes(block.as_ptr() as u64);
    let tree = addrout[0];
    control = [0; 8]; control[0] = 42; control[3] = 1;
    addrin[0] = tree;
    intin[0] = 0; intin[1] = 16; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] GEM.RSC tree 0 (file selector) drawn");
}

/// D8 Batch-1 demo: open the file selector on C:\*.* — loads GEM.RSC,
/// lists the directory into the slots, draws the FSELECTR dialog (the
/// modal loop then waits for a click; the screendump captures the drawn
/// dialog with the file list populated).
pub fn run_fsel() {
    let mut path = [0u8; 96];
    let mut sel = [0u8; 16];
    path[..6].copy_from_slice(b"C:\\*.*");
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
    control[0] = 90; control[3] = 2; // fsel_input
    addrin[0] = path.as_mut_ptr() as i64;
    addrin[1] = sel.as_mut_ptr() as i64;
    kprintln!("[objc] opening file selector on C:\\*.*");
    aes(block.as_ptr() as u64); // draws, then blocks in the modal loop
    kprintln!("[objc] fsel button={} sel ok", intout[1]);
}

/// D9 demo: round-trip the Scrap + Shell managers (scrp_read/write,
/// shel_put/get, shel_find, shel_envrn).
pub fn run_d9() {
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
    fn s(b: &[u8]) -> &str {
        core::str::from_utf8(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).unwrap_or("?")
    }
    let mut buf = [0u8; 64];

    // scrp_read (default)
    control = [0;8]; control[0]=80; control[3]=1; addrin[0]=buf.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] scrp_read default = \"{}\"", s(&buf));

    // scrp_write then scrp_read
    let mut p = *b"C:\\TMP\\\0\0";
    control=[0;8]; control[0]=81; control[3]=1; addrin[0]=p.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    buf=[0;64]; control=[0;8]; control[0]=80; control[3]=1; addrin[0]=buf.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] scrp after write = \"{}\"", s(&buf));

    // shel_put / shel_get
    let mut data = *b"NewTOS\0";
    control=[0;8]; control[0]=123; control[1]=1; control[3]=1; intin[0]=7; addrin[0]=data.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    buf=[0;64]; control=[0;8]; control[0]=122; control[1]=1; control[3]=1; intin[0]=7; addrin[0]=buf.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] shel_get = \"{}\"", s(&buf));

    // shel_find existing + missing
    let mut f1 = *b"HELLO.PRG\0\0\0\0\0\0";
    control=[0;8]; control[0]=124; control[3]=1; addrin[0]=f1.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] shel_find HELLO.PRG -> {} (\"{}\")", intout[0], s(&f1));
    let mut f2 = *b"NOPE.XXX\0";
    control=[0;8]; control[0]=124; control[3]=1; addrin[0]=f2.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] shel_find NOPE.XXX -> {}", intout[0]);

    // shel_envrn PATH=
    let mut val: u64 = 0;
    let mut needle = *b"PATH=\0";
    control=[0;8]; control[0]=125; control[3]=2;
    addrin[0]=&mut val as *mut u64 as i64; addrin[1]=needle.as_mut_ptr() as i64;
    aes(block.as_ptr() as u64);
    let mut ev = [0u8; 32];
    if val != 0 { for i in 0..31 { let c = unsafe { *(val as *const u8).add(i) }; if c==0 {break;} ev[i]=c; } }
    kprintln!("[objc] shel_envrn PATH= -> ret {} val=\"{}\"", intout[0], s(&ev));
}

/// Group-B verification: a non-modal button showcase that renders, in one
/// `objc_draw`, every button-border case the D3/D4/D5 fix touches — 2D
/// buttons (plain `th=-1`, EXIT `th=-2`, DEFAULT `th=-2`, DEFAULT+EXIT
/// `th=-3`), 3D buttons (raised EXIT + raised DEFAULT+EXIT + a pressed one),
/// and a hollow `G_BOX` with a negative border (`th=-1`) that exercises the
/// D5 `|th|+1` edge-inclusive geometry independently of any button. Draw-only
/// (no modal loop), so it is screendump-friendly.
// The AESPB arrays are read by the trap through the raw `block` pointer, which
// clippy's dataflow cannot see, so it mis-reports the final config writes as
// dead (same false positive as the rest of this selftest module).
#[allow(unused_assignments)]
pub fn run_button_showcase() {
    const SHADOWED: u16 = 0x0020; // ob_state, unused here
    static LBL: &[u8] = b"Group B: 2D th=-1/-2/-3, 3D outline+bevel, hollow neg-border box\0";
    static PLAIN: &[u8] = b"Plain\0";
    static EXITB: &[u8] = b"Exit\0";
    static DEFLT: &[u8] = b"Deflt\0";
    static DEFEX: &[u8] = b"Def+Ex\0";
    static D3EX: &[u8] = b"3D Exit\0";
    static D3DEF: &[u8] = b"3D Def+Ex\0";
    static D3PRS: &[u8] = b"3D Press\0";
    let _ = SHADOWED;

    // hollow box: colour word 0x1000 = black border, hollow-white interior;
    // byte 1 of the LONG = 0xFF → th = -1 (D5 negative-border case).
    let box_hollow: u64 = 0x00FF_1000;
    let btn = |spec: &[u8], fl, st, x, y, next| Object {
        ob_next: next, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
        ob_flags: fl, ob_state: st, ob_spec: spec.as_ptr() as u64,
        ob_x: x, ob_y: y, ob_width: 96, ob_height: 28,
    };

    let tree = [
        // 0: plain dialog frame (2D, th=1 inside border)
        Object { ob_next: -1, ob_head: 1, ob_tail: 9, ob_type: G_BOX, ob_flags: 0,
                 ob_state: 0, ob_spec: 1 << 16, ob_x: 80, ob_y: 80, ob_width: 480, ob_height: 250 },
        // 1: label
        Object { ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
                 ob_state: 0, ob_spec: LBL.as_ptr() as u64, ob_x: 12, ob_y: 8, ob_width: 456, ob_height: 16 },
        // 2..5: 2D buttons — plain / EXIT / DEFAULT / DEFAULT+EXIT
        btn(PLAIN, SELECTABLE, 0, 16, 40, 3),
        btn(EXITB, SELECTABLE | EXIT, 0, 124, 40, 4),
        btn(DEFLT, SELECTABLE | DEFAULT, 0, 232, 40, 5),
        btn(DEFEX, SELECTABLE | EXIT | DEFAULT, 0, 340, 40, 6),
        // 6..8: 3D buttons — raised EXIT / raised DEFAULT+EXIT / pressed
        Object { ob_next: 7, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | FL3DACT, ob_state: 0,
                 ob_spec: D3EX.as_ptr() as u64, ob_x: 40, ob_y: 108, ob_width: 110, ob_height: 30 },
        Object { ob_next: 8, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | DEFAULT | FL3DACT, ob_state: 0,
                 ob_spec: D3DEF.as_ptr() as u64, ob_x: 185, ob_y: 108, ob_width: 110, ob_height: 30 },
        Object { ob_next: 9, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | FL3DACT, ob_state: 0x0001, // SELECTED = pressed
                 ob_spec: D3PRS.as_ptr() as u64, ob_x: 330, ob_y: 108, ob_width: 110, ob_height: 30 },
        // 9: hollow negative-border box (D5), LASTOB
        Object { ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_BOX, ob_flags: LASTOB,
                 ob_state: 0, ob_spec: box_hollow, ob_x: 40, ob_y: 175, ob_width: 150, ob_height: 55 },
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
    control[0] = 42; control[3] = 1; // objc_draw
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0; intin[1] = 8; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] button showcase drawn, ret={}", intout[0]);
}

/// 3D / modern-design test: a grey background panel (FL3DBAK) with a raised
/// 3D OK button (FL3DACT) and a pressed one, to show the bevel look.
pub fn run_3d() {
    const FL3DBAK: u16 = 0x0400;
    const FL3DACT: u16 = 0x0600;
    static TITLE: &[u8] = b"Modern (Falcon) 3D design\0";
    static OK: &[u8] = b"OK\0";
    static CANCEL: &[u8] = b"Cancel\0";
    let tree = [
        // 0: 3D background panel (grey, recessed)
        Object { ob_next: -1, ob_head: 1, ob_tail: 3, ob_type: G_BOX, ob_flags: FL3DBAK,
                 ob_state: 0, ob_spec: 0, ob_x: 160, ob_y: 140, ob_width: 320, ob_height: 160 },
        // 1: title
        Object { ob_next: 2, ob_head: -1, ob_tail: -1, ob_type: G_STRING, ob_flags: 0,
                 ob_state: 0, ob_spec: TITLE.as_ptr() as u64, ob_x: 24, ob_y: 24, ob_width: 272, ob_height: 16 },
        // 2: raised 3D OK button
        Object { ob_next: 3, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | FL3DACT, ob_state: 0,
                 ob_spec: OK.as_ptr() as u64, ob_x: 40, ob_y: 108, ob_width: 100, ob_height: 30 },
        // 3: pressed 3D button (SELECTED → recessed)
        Object { ob_next: 0, ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
                 ob_flags: SELECTABLE | EXIT | LASTOB | FL3DACT, ob_state: 0x0001,
                 ob_spec: CANCEL.as_ptr() as u64, ob_x: 180, ob_y: 108, ob_width: 100, ob_height: 30 },
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
    control[0] = 42; control[3] = 1;
    addrin[0] = tree.as_ptr() as i64;
    intin[0] = 0; intin[1] = 8; intin[2] = 0; intin[3] = 0; intin[4] = 640; intin[5] = 480;
    aes(block.as_ptr() as u64);
    kprintln!("[objc] 3D design test drawn");
}
