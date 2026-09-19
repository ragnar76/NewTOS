//! `objc_add` (40) answers `1` — always — `TASK/A55.md`.
//!
//! `crysbind` starts every opcode with `ret = TRUE`
//! (`references/emutos/aes/gemsuper.c:85`, pinned `1a7484ab`) and the
//! `OBJC_ADD` arm never touches it:
//!
//! ```c
//! case OBJC_ADD:
//!     ob_add((OBJECT *)OB_TREE, OB_PARENT, OB_CHILD);
//!     break;
//! ```
//!
//! (`:204-206`; `ob_add` is `void`, `aes/gemoblib.c:1121-1141`). So the
//! answer is `1` whether the link was made or not — `ob_add`'s own guard,
//! `if ((parent != NIL) && (child != NIL))` (`:1127`), only decides whether
//! the tree is written. Cases 550–552 of `tosdump/cases/aes-r4-subfunctions.tsv`
//! (CRC `227422B2`) measured `1` on the TT where NewTOS answered `0`
//! (`docs/analysis/aes-r4-subfunktionen-einordnung-2026-09-12.md`, D-3).
//!
//! NewTOS keeps its guards (they are what stops an out-of-range index from
//! being dereferenced) but they no longer change the answer: a refused add
//! leaves the tree exactly as it was, and says `1`.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK, POISON};
use tos_api::aes::object::Object;

const OP_OBJC_ADD: i16 = 40;
const NIL: i16 = -1;
const ROOT: i16 = 0;
const G_BOX: u16 = 20;
const LASTOB: u16 = 0x0020;

fn lock() -> std::sync::MutexGuard<'static, ()> {
    AES_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn node(next: i16, head: i16, tail: i16, flags: u16) -> Object {
    Object {
        ob_next: next,
        ob_head: head,
        ob_tail: tail,
        ob_type: G_BOX,
        ob_flags: flags,
        ob_state: 0,
        ob_spec: 0x0001_1100,
        ob_x: 0,
        ob_y: 0,
        ob_width: 100,
        ob_height: 100,
    }
}

/// The three link WORDs of one record, in `ob_next, ob_head, ob_tail` order.
fn links(o: &Object) -> (i16, i16, i16) {
    (o.ob_next, o.ob_head, o.ob_tail)
}

/// `ROOT` with no children yet and two unlinked records 1 and 2 behind it —
/// what a program has after allocating the records and before `objc_add`.
fn unlinked() -> Vec<Object> {
    vec![node(NIL, NIL, NIL, 0), node(NIL, NIL, NIL, 0), node(NIL, NIL, NIL, LASTOB)]
}

/// `objc_add(tree, parent, child)` through the dispatcher, gemlib's
/// `AES_PARAMS(40,2,1,1,0)` (`a_objc_add.c:28`); returns `int_out[0]`.
fn objc_add(pb: &mut Pb, tree: &mut [Object], parent: i16, child: i16) -> i16 {
    pb.poison();
    pb.intin[0] = parent;
    pb.intin[1] = child;
    pb.addrin[0] = tree.as_mut_ptr() as u64;
    pb.call(OP_OBJC_ADD, 2, 1, 1, 0);
    assert_eq!(pb.intout[1], POISON, "int_out[1] is past OUT_LEN 1");
    pb.intout[0]
}

// ── the link is made ──────────────────────────────────────────────────────

/// First child: `child.ob_next = parent`, `parent.ob_head = parent.ob_tail =
/// child` (`gemoblib.c:1132-1139`). Second child: appended after the tail,
/// `parent.ob_tail` moves. `1` both times.
#[test]
fn a_valid_add_links_the_child_last_and_answers_1() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut tree = unlinked();

    let ret = objc_add(&mut pb, &mut tree, ROOT, 1);
    assert_eq!(ret, 1, "crysbind's `ret = TRUE` (`gemsuper.c:85`), untouched by the arm (`:204-206`)");
    assert_eq!(links(&tree[0]), (NIL, 1, 1), "ROOT (next, head, tail): first child is head and tail");
    assert_eq!(links(&tree[1]), (ROOT, NIL, NIL), "object 1 (next, head, tail): chained back to ROOT");

    let ret = objc_add(&mut pb, &mut tree, ROOT, 2);
    assert_eq!(ret, 1);
    assert_eq!(links(&tree[0]), (NIL, 1, 2), "ROOT (next, head, tail): tail moved to the new child");
    assert_eq!(links(&tree[1]), (2, NIL, NIL), "object 1 (next, head, tail): now points at 2");
    assert_eq!(links(&tree[2]), (ROOT, NIL, NIL), "object 2 (next, head, tail): the new last child");
}

// ── the guards refuse, the answer is still 1 ──────────────────────────────

/// `parent == NIL`: `ob_add`'s guard skips the body (`:1127`), the arm
/// still answers `1` (case 551). NewTOS' `parent < 0` guard is that guard
/// plus protection against any other negative index; the tree stays as it
/// was.
#[test]
fn a_nil_parent_is_refused_the_tree_is_untouched_and_the_answer_is_still_1() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut tree = unlinked();
    let before: Vec<(i16, i16, i16)> = tree.iter().map(links).collect();

    let ret = objc_add(&mut pb, &mut tree, NIL, 1);

    assert_eq!(ret, 1, "the OBJC_ADD arm never writes `ret` (`gemsuper.c:85,204-206`)");
    let after: Vec<(i16, i16, i16)> = tree.iter().map(links).collect();
    assert_eq!(after, before, "a refused add leaves every link as it was");
}

/// `child == NIL`: same guard, same answer (case 552).
#[test]
fn a_nil_child_is_refused_the_tree_is_untouched_and_the_answer_is_still_1() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut tree = unlinked();
    let before: Vec<(i16, i16, i16)> = tree.iter().map(links).collect();

    let ret = objc_add(&mut pb, &mut tree, ROOT, NIL);

    assert_eq!(ret, 1);
    let after: Vec<(i16, i16, i16)> = tree.iter().map(links).collect();
    assert_eq!(after, before);
}

/// Cases 550–552 of `aes-r4-subfunctions.tsv` as measured: `AIN_LEN 1` with
/// the pointer slot left at the runner's poison — "the only arm reachable
/// without a tree" — and `(NIL, NIL)`, `(NIL, ROOT)`, `(ROOT, NIL)`. The TT
/// answered `1` three times; so does NewTOS, without touching the poison
/// address.
#[test]
fn the_three_r4_guard_cases_answer_1_without_a_tree() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();

    for (case, parent, child) in [(550, NIL, NIL), (551, NIL, ROOT), (552, ROOT, NIL)] {
        pb.poison(); // addr_in[0] = POISON_PTR, as the runner leaves it
        pb.intin[0] = parent;
        pb.intin[1] = child;
        pb.call(OP_OBJC_ADD, 2, 1, 1, 0);
        assert_eq!(pb.intout[0], 1, "case {case}: objc_add({parent}, {child}) must answer 1 as the TT did");
        assert_eq!(pb.intout[1], POISON, "case {case}: int_out[1] is past OUT_LEN 1");
    }
}

/// A null tree: EmuTOS would write through the null base; NewTOS' guard
/// refuses the write and — since the arm never sets `ret` — still answers
/// `1`.
#[test]
fn a_null_tree_is_refused_and_the_answer_is_still_1() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();

    pb.poison();
    pb.intin[0] = ROOT;
    pb.intin[1] = 1;
    pb.addrin[0] = 0;
    pb.call(OP_OBJC_ADD, 2, 1, 1, 0);

    assert_eq!(pb.intout[0], 1);
    assert_eq!(pb.intout[1], POISON);
}
