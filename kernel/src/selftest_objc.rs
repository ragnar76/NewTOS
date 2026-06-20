//! AES Object Manager demo: build a small dialog tree and render it
//! through the real GEM trap (`objc_draw`, opcode 42) so it is visible in
//! `cargo xtask run --window`.

use tos_api::aes::object::Object;

use crate::kprintln;

const G_BOX: u16 = 20;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;

const SELECTABLE: u16 = 0x0001;
const EXIT: u16 = 0x0004;
const LASTOB: u16 = 0x0020;

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
            ob_type: G_BOX, ob_flags: 0, ob_state: 0, ob_spec: box_spec,
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
            ob_type: G_BUTTON, ob_flags: SELECTABLE | EXIT, ob_state: 0x0001,
            ob_spec: OK.as_ptr() as u64,
            ob_x: 40, ob_y: 110, ob_width: 100, ob_height: 28,
        },
        // 3: Cancel button (last child → ob_next = parent 0; LASTOB)
        Object {
            ob_next: 0, ob_head: -1, ob_tail: -1,
            ob_type: G_BUTTON, ob_flags: SELECTABLE | EXIT | LASTOB, ob_state: 0,
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
