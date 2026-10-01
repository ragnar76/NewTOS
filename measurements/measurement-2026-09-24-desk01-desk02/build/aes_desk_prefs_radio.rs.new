//! **DESK02** — the one AES property the desktop's new *Set preferences* box
//! depends on and no earlier desktop dialog did: a **radio-button group is
//! scoped by its parent object**.
//!
//! EmuTOS's `ADSETPRE` tree (`references/emutos/desk/desk_rsc.c:1369-1560`)
//! wraps each `RBUTTON` run in a `G_IBOX` of its own — six of them, one per
//! preference — and the boxes are the only thing that keeps the six groups
//! apart. `fm_button` deselects siblings by walking the **parent's** child
//! chain:
//!
//! ```c
//! parent = get_par(tree, new_obj);
//! for (cur = (tree+parent)->ob_head; cur != parent; cur = (tree+cur)->ob_next)
//!     if ((tree+cur)->ob_flags & RBUTTON) …
//! ```
//!
//! (`aes/gemfmlib.c`, reproduced in `tos-api/src/aes/form.rs::select_radio`),
//! so a tree that hangs two groups directly under `ROOT` has, as far as the
//! AES is concerned, **one** group: picking a date format would clear the time
//! format. `kernel/src/desk/inf.rs::inf_pref` therefore builds a `G_IBOX` per
//! group; this test is what says the box is load-bearing rather than
//! decorative.
//!
//! Driven through the real dispatcher with `form_button` (AES 56), the same
//! entry `form_do`'s inner loop uses for a click.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};
use tos_api::aes::object::Object;

const OP_FORM_BUTTON: i16 = 56;

// obdefs.h
const G_BOX: u16 = 20;
const G_IBOX: u16 = 25;
const G_BUTTON: u16 = 26;
const SELECTABLE: u16 = 0x0001;
const RBUTTON: u16 = 0x0010;
const LASTOB: u16 = 0x0020;
const SELECTED: u16 = 0x0001;

const NIL: i16 = -1;

/// Opaque dialog-box colour word, as `kernel/src/desk/inf.rs` uses.
const BOX_SPEC: u64 = (1 << 16) | 0x1070;

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

fn rb(x: i16, next: i16, last: bool) -> Object {
    let fl = SELECTABLE | RBUTTON | if last { LASTOB } else { 0 };
    mk(G_BUTTON, fl, LBL.as_ptr() as u64, x, 0, 40, 20, next, NIL, NIL)
}

/// `ADSETPRE`'s shape, cut down to two groups: a `G_BOX` root with two
/// `G_IBOX` children, three `RBUTTON`s under each.
///
/// ```text
/// 0 ROOT      head 1  tail 5
/// 1 IBOX-A    head 2  tail 4   next 5
/// 2..4  RBUTTON A0 A1 A2       next 3, 4, 1
/// 5 IBOX-B    head 6  tail 8   next 0
/// 6..8  RBUTTON B0 B1 B2       next 7, 8, 5
/// ```
const A0: usize = 2;
const B0: usize = 6;

fn boxed_groups() -> Vec<Object> {
    vec![
        mk(G_BOX, 0, BOX_SPEC, 0, 0, 200, 90, NIL, 1, 5),
        mk(G_IBOX, 0, 0, 10, 10, 180, 20, 5, 2, 4),
        rb(0, 3, false),
        rb(45, 4, false),
        rb(90, 1, false),
        mk(G_IBOX, 0, 0, 10, 50, 180, 20, 0, 6, 8),
        rb(0, 7, false),
        rb(45, 8, false),
        rb(90, 5, true),
    ]
}

/// The same six buttons with the two `G_IBOX`es removed — every `RBUTTON`
/// directly under `ROOT`. This is the tree a dialog gets if the boxes are
/// dropped as "invisible anyway".
const F_A0: usize = 1;
const F_B0: usize = 4;

fn flat_groups() -> Vec<Object> {
    vec![
        mk(G_BOX, 0, BOX_SPEC, 0, 0, 200, 90, NIL, 1, 6),
        rb(10, 2, false),
        rb(55, 3, false),
        rb(100, 4, false),
        rb(10, 5, false),
        rb(55, 6, false),
        rb(100, 0, true),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, mut o)| {
        // The lower three sit on the second row.
        if i >= F_B0 {
            o.ob_y = 40;
        }
        o
    })
    .collect()
}

/// `form_button(tree, obj, clks)` — `int_out[0]` is "keep the dialog running",
/// `int_out[1]` the next object.
fn form_button(pb: &mut Pb, tree: &mut [Object], obj: usize) -> (i16, i16) {
    pb.intin.fill(0);
    pb.addrin[0] = tree.as_mut_ptr() as u64;
    pb.intin[0] = obj as i16;
    pb.intin[1] = 1; // one click
    pb.call(OP_FORM_BUTTON, 2, 2, 1, 0);
    (pb.intout[0], pb.intout[1])
}

/// Which of `tree[base .. base+3]` carry `SELECTED`.
fn selected(tree: &[Object], base: usize) -> Vec<usize> {
    (0..3).filter(|i| tree[base + i].ob_state & SELECTED != 0).collect()
}

#[test]
fn a_radio_click_selects_one_button_of_its_own_group() {
    let _lock = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ctx = context();
    let mut pb = Pb::new();

    let mut tree = boxed_groups();
    // The dialog's pre-selection (`inf_pref`'s `tree[base + n].ob_state |=
    // SELECTED`): the middle button of group A, the first of group B.
    tree[A0 + 1].ob_state |= SELECTED;
    tree[B0].ob_state |= SELECTED;

    let (cont, next) = form_button(&mut pb, &mut tree, A0 + 2);
    assert_eq!(cont, 1, "an RBUTTON without EXIT must not end the dialog");
    assert_eq!(next, 0, "a click that continues and is not editable selects no object");

    assert_eq!(selected(&tree, A0), vec![2], "exactly the clicked button of group A");
    assert_eq!(selected(&tree, B0), vec![0], "group B must be untouched by a group-A click");
}

#[test]
fn each_group_keeps_its_own_selection_across_clicks() {
    let _lock = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ctx = context();
    let mut pb = Pb::new();

    let mut tree = boxed_groups();
    tree[A0].ob_state |= SELECTED;
    tree[B0].ob_state |= SELECTED;

    // Walk every button of both groups in turn; after each click both groups
    // must still hold exactly one selection, and only the clicked group's may
    // have moved. This is what the Preferences dialog reads back with
    // `inf_which`/`inf_gindex`, which return -1 for a group with nothing
    // selected and would silently fall back to a default.
    for a in 0..3 {
        form_button(&mut pb, &mut tree, A0 + a);
        assert_eq!(selected(&tree, A0), vec![a]);
        assert_eq!(selected(&tree, B0), vec![0], "group B follows group A's clicks");
    }
    for b in 0..3 {
        form_button(&mut pb, &mut tree, B0 + b);
        assert_eq!(selected(&tree, B0), vec![b]);
        assert_eq!(selected(&tree, A0), vec![2], "group A follows group B's clicks");
    }
}

/// The counter-example, so the `G_IBOX` in `inf_pref`'s tree cannot be removed
/// as redundant: with the boxes gone the AES sees **one** group and a click in
/// the second run clears the first run's selection.
#[test]
fn without_its_ibox_a_group_takes_the_whole_parent_with_it() {
    let _lock = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ctx = context();
    let mut pb = Pb::new();

    let mut tree = flat_groups();
    tree[F_A0 + 1].ob_state |= SELECTED;
    tree[F_B0].ob_state |= SELECTED;

    form_button(&mut pb, &mut tree, F_B0 + 2);

    assert_eq!(selected(&tree, F_B0), vec![2], "the clicked button is selected");
    assert!(
        selected(&tree, F_A0).is_empty(),
        "and the other run lost its selection — which is why every group in \
         `inf_pref` hangs under a G_IBOX of its own"
    );
}
