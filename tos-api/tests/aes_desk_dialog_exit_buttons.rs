//! **DESK13** — the AES property the two new file-operation dialogs read their
//! answer from: after `form_do` ends, **exactly the pressed `EXIT` button
//! carries `SELECTED`**.
//!
//! Neither of EmuTOS's two `deskdir.c` dialogs uses `form_do`'s return value:
//!
//! * `dir_op` asks `more = (tree[CDOK].ob_state & SELECTED) ? TRUE : FALSE;`
//!   (`references/emutos/desk/deskdir.c:963`) — "did the user press OK, or
//!   Cancel?";
//! * `do_namecon` asks `ob = inf_gindex(tree, CAOK, 3) + CAOK;`
//!   (`deskdir.c:121`) — "which of OK / Skip / Stop?", by scanning the three
//!   exit buttons for the selected one.
//!
//! That is a stronger requirement than "`form_do` returned the object": it
//! means the AES must leave the state of a three-button exit run unambiguous.
//! If two were selected, `inf_gindex` would return the first — and a *Skip*
//! read as an *OK* overwrites a file the user asked to keep. If none were,
//! `inf_gindex` returns `-1` and `CAOK - 1` is the editable name field.
//!
//! `programs/desktop/src/dir.rs` reproduces both reads, so both shapes are pinned
//! here, driven through the real dispatcher with `form_button` (AES 56) — the
//! same entry `form_do`'s inner loop uses for a click.
//!
//! The sibling test `aes_desk_prefs_radio.rs` covers the *radio* groups of
//! `ADSETPRE`; this one covers the *exit* runs of `ADCPYDEL`/`ADCPALER`, which
//! are plain `SELECTABLE | EXIT` buttons with no `RBUTTON` and therefore no
//! sibling-deselect machinery behind them at all.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, host_only, Pb, AES_LOCK};
use tos_api::aes::object::Object;
use tos_api::host::Host;

const OP_FORM_BUTTON: i16 = 56;

// obdefs.h
const G_BOX: u16 = 20;
const G_BUTTON: u16 = 26;
const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const LASTOB: u16 = 0x0020;
const SELECTED: u16 = 0x0001;

const NIL: i16 = -1;

/// Opaque dialog-box colour word, as `programs/desktop/src/inf.rs` uses.
const BOX_SPEC: u64 = (1 << 16) | 0x1100;

static LBL: &[u8] = b" x \0";

fn mk(ty: u16, fl: u16, spec: u64, x: i16, y: i16, w: i16, h: i16, next: i16, head: i16, tail: i16) -> Object {
    Object {
        ob_next: next,
        ob_head: head,
        ob_tail: tail,
        ob_type: ty,
        ob_flags: fl,
        ob_state: 0,
        ob_spec: spec,
        ob_x: x,
        ob_y: y,
        ob_width: w,
        ob_height: h,
    }
}

fn exit_btn(x: i16, next: i16, first: bool, last: bool) -> Object {
    let fl = SELECTABLE | EXIT | if first { DEFAULT } else { 0 } | if last { LASTOB } else { 0 };
    mk(G_BUTTON, fl, LBL.as_ptr() as u64, x, 60, 90, 28, next, NIL, NIL)
}

/// `ADCPYDEL`'s shape, reduced to what the read needs: a root with `CDOK` and
/// `CDCNCL` under it (`desk_rsc.c:1224-1234`).
const CDOK: usize = 1;

fn cpydel() -> Vec<Object> {
    vec![
        mk(G_BOX, 0, BOX_SPEC, 0, 0, 300, 120, NIL, 1, 2),
        exit_btn(20, 2, true, false),
        exit_btn(190, 0, false, true),
    ]
}

/// `ADCPALER`'s shape: `CAOK`, `CASKIP`, `CASTOP` (`desk_rsc.c:1263-1279`).
const CAOK: usize = 1;

fn cpaler() -> Vec<Object> {
    vec![
        mk(G_BOX, 0, BOX_SPEC, 0, 0, 340, 120, NIL, 1, 3),
        exit_btn(20, 2, true, false),
        exit_btn(125, 3, false, false),
        exit_btn(230, 0, false, true),
    ]
}

/// Park the VDI mouse pointer, button **up**, at `(x, y)`.
///
/// An `EXIT` button is not an `RBUTTON`, so `fm_button` sends it through
/// `graf::watchbox` (`tos-api/src/aes/graf.rs:400-419`), which answers "was the
/// release inside the object?". With the pointer parked inside and no button
/// held, the watch loop breaks on its first pass and answers `true` — a
/// completed click. That is the release edge `form_do` reacts to, and without
/// it `watchbox` reports a release *outside* the button and the dialog stays up
/// (which is correct behaviour, and is why this parking is not a fudge).
fn park(x: i16, y: i16) {
    let backend = Host::raster(host_only()).expect("the harness host has a raster backend");
    // Relative feed: walk hard into the top-left clamp first, then out to the
    // wanted position.
    tos_api::vdi::feed_relative(-4000, -4000, 0, backend);
    tos_api::vdi::feed_relative(x, y, 0, backend);
}

/// `form_button(tree, obj, clks)` — `int_out[0]` is "keep the dialog running",
/// `int_out[1]` the next object. The pointer is parked over `obj` first, as a
/// real click has it.
fn form_button(pb: &mut Pb, tree: &mut [Object], obj: usize) -> (i16, i16) {
    let o = &tree[obj];
    park(o.ob_x + o.ob_width / 2, o.ob_y + o.ob_height / 2);
    pb.intin.fill(0);
    pb.addrin[0] = tree.as_mut_ptr() as u64;
    pb.intin[0] = obj as i16;
    pb.intin[1] = 1; // one click
    pb.call(OP_FORM_BUTTON, 2, 2, 1, 0);
    (pb.intout[0], pb.intout[1])
}

/// `inf_gindex` (`util/optimize.c:134-147`) — the index, counted from the
/// start, of the first `SELECTED` object in `tree[base .. base+n]`, or `-1`.
/// Reproduced from `programs/desktop/src/inf.rs`.
fn inf_gindex(tree: &[Object], base: usize, n: usize) -> i16 {
    for (k, obj) in tree.iter().skip(base).take(n).enumerate() {
        if obj.ob_state & SELECTED != 0 {
            return k as i16;
        }
    }
    -1
}

#[test]
fn the_copy_delete_box_can_tell_ok_from_cancel_by_state_alone() {
    let _lock = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ctx = context();
    let mut pb = Pb::new();

    // Cancel: `tree[CDOK].ob_state & SELECTED` must be clear, which is what
    // `dir_op` turns into "stop before touching a single file".
    let mut tree = cpydel();
    let (cont, _next) = form_button(&mut pb, &mut tree, CDOK + 1);
    assert_eq!(cont, 0, "an EXIT button ends the dialog");
    assert_eq!(tree[CDOK].ob_state & SELECTED, 0, "OK must not be selected by a Cancel click");
    assert_ne!(tree[CDOK + 1].ob_state & SELECTED, 0, "Cancel is");

    // OK: the same read, the other way round.
    let mut tree = cpydel();
    let (cont, _next) = form_button(&mut pb, &mut tree, CDOK);
    assert_eq!(cont, 0);
    assert_ne!(tree[CDOK].ob_state & SELECTED, 0, "`deskdir.c:963` reads exactly this bit");
    assert_eq!(tree[CDOK + 1].ob_state & SELECTED, 0);
}

#[test]
fn the_name_conflict_box_identifies_one_of_its_three_buttons() {
    let _lock = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ctx = context();
    let mut pb = Pb::new();

    // OK (0), Skip (1), Stop (2) — `do_namecon` adds `CAOK` to the index and
    // compares the result against `CASTOP` / `CASKIP` (`deskdir.c:121,346-349`).
    for pressed in 0..3usize {
        let mut tree = cpaler();
        let (cont, _next) = form_button(&mut pb, &mut tree, CAOK + pressed);
        assert_eq!(cont, 0, "every one of the three is an EXIT button");
        assert_eq!(
            inf_gindex(&tree, CAOK, 3),
            pressed as i16,
            "the pressed button, and only it, is left SELECTED — a Skip read as \
             an OK would overwrite a file the user asked to keep"
        );
        let selected: Vec<usize> = (0..3).filter(|&k| tree[CAOK + k].ob_state & SELECTED != 0).collect();
        assert_eq!(selected, vec![pressed], "exactly one");
    }
}
