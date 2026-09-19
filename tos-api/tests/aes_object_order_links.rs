//! `objc_order` (45) — the tree it leaves behind, link by link — `TASK/A47.md`.
//!
//! EmuTOS `ob_order` (`references/emutos/aes/gemoblib.c:1201-1246`, pinned
//! `1a7484ab`) unlinks the object with `ob_delete` (`:1147-1193`) and then
//! re-inserts it *without ever restoring* what the delete undid:
//!
//! ```c
//! ob_delete(tree, mov_obj);
//! chg_obj = parentptr->ob_head;
//! if (new_pos == 0)
//! {
//!       movptr->ob_next = chg_obj;    /* put mov_obj at head of list */
//!       parentptr->ob_head = mov_obj;
//! }
//! else
//! {
//!     if (new_pos == NIL)
//!         chg_obj = parentptr->ob_tail;
//!     else
//!         for (ii = 1; ii < new_pos; ii++)
//!             chg_obj = (treeptr + chg_obj)->ob_next;
//!     chgptr = treeptr + chg_obj;
//!     movptr->ob_next = chgptr->ob_next;
//!     chgptr->ob_next = mov_obj;
//! }
//! if (movptr->ob_next == parent)
//!     parentptr->ob_tail = mov_obj;
//! return TRUE;
//! ```
//!
//! For an **only child** `ob_delete` sets the parent's `ob_head` *and*
//! `ob_tail` to `NIL` (`:1164-1175`). With `new_pos == 0` the head branch then
//! copies that `NIL` into the object's `ob_next` and re-hangs it as head; the
//! final tail test compares `NIL` with the parent and misses. EmuTOS returns
//! `TRUE` with `parent.ob_head = obj`, `parent.ob_tail = NIL`,
//! `obj.ob_next = NIL` — a chain that no longer closes back on the parent.
//! That is the reference, so that is what NewTOS leaves too (spec:
//! `docs/api/aes/object-form.md` § `objc_order`).
//!
//! With `new_pos != 0` on an only child, `chg_obj` is `NIL` and EmuTOS
//! indexes `tree[-1]` — a read and a write **before the array**. No reference
//! result exists there; NewTOS re-links the object as the parent's sole child
//! and touches nothing outside the tree. Every tree below is fenced with a
//! poisoned record on each side so that "nothing outside the tree" is
//! asserted, not assumed.
//!
//! The two-sibling cases are the ones the window manager relies on
//! (`wm_mktop` → `ob_order(gl_wtree, w_handle, NIL)`, `gemwmlib.c:1605-1610`)
//! and are derived from the same lines; they close the chain correctly on both
//! machines.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK, POISON};
use tos_api::aes::object::Object;

const OP_OBJC_ORDER: i16 = 45;
const NIL: i16 = -1;
const ROOT: i16 = 0;
const G_BOX: u16 = 20;
const LASTOB: u16 = 0x0020;

/// Every WORD of a fence record — a value no link field of a live tree holds.
const FENCE: i16 = 0x7777;

fn lock() -> std::sync::MutexGuard<'static, ()> {
    AES_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// A `G_BOX` with the given links; type/flags/spec/geometry are not read by
/// `ob_order` and stay constant so a change to them would be a finding too.
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

fn fence() -> Object {
    Object {
        ob_next: FENCE,
        ob_head: FENCE,
        ob_tail: FENCE,
        ob_type: FENCE as u16,
        ob_flags: FENCE as u16,
        ob_state: FENCE as u16,
        ob_spec: 0x7777_7777_7777_7777,
        ob_x: FENCE,
        ob_y: FENCE,
        ob_width: FENCE,
        ob_height: FENCE,
    }
}

/// The three link WORDs of one record, in `ob_next, ob_head, ob_tail` order.
fn links(o: &Object) -> (i16, i16, i16) {
    (o.ob_next, o.ob_head, o.ob_tail)
}

/// `N` tree records between two fence records. [`Fenced::tree`] is the AES's
/// view: index 0 is `ROOT`.
struct Fenced<const N: usize> {
    /// The whole allocation, tree in the middle.
    mem: Vec<Object>,
}

impl<const N: usize> Fenced<N> {
    fn new(nodes: [Object; N]) -> Fenced<N> {
        let mut mem = Vec::with_capacity(N + 2);
        mem.push(fence());
        mem.extend_from_slice(&nodes);
        mem.push(fence());
        Fenced { mem }
    }

    /// Base of the tree — the record after the leading fence.
    fn tree(&mut self) -> *mut Object {
        // SAFETY: `mem` holds N + 2 records; index 1 is inside it.
        unsafe { self.mem.as_mut_ptr().add(1) }
    }

    fn at(&self, i: usize) -> &Object {
        &self.mem[i + 1]
    }

    /// Both fences must still be the fences: the call wrote nothing outside
    /// `tree[0..N]`.
    fn assert_fences(&self, when: &str) {
        let f = fence();
        assert_eq!(links(&self.mem[0]), links(&f), "{when}: the record before the tree was written");
        assert_eq!(links(&self.mem[N + 1]), links(&f), "{when}: the record after the tree was written");
        assert_eq!(self.mem[0].ob_spec, f.ob_spec, "{when}: the record before the tree was written");
        assert_eq!(self.mem[N + 1].ob_spec, f.ob_spec, "{when}: the record after the tree was written");
    }
}

/// `objc_order(tree, obj, new_pos)` through the dispatcher, gemlib's
/// `AES_PARAMS(45,2,1,1,0)` (`a_objc_order.c:29`); returns `int_out[0]`.
fn objc_order<const N: usize>(pb: &mut Pb, t: &mut Fenced<N>, obj: i16, new_pos: i16) -> i16 {
    pb.poison();
    pb.intin[0] = obj;
    pb.intin[1] = new_pos;
    pb.addrin[0] = t.tree() as u64;
    pb.call(OP_OBJC_ORDER, 2, 1, 1, 0);
    assert_eq!(pb.intout[1], POISON, "int_out[1] is past OUT_LEN 1");
    pb.intout[0]
}

/// `ROOT` with one child, index 1: `ROOT.head = ROOT.tail = 1`,
/// `1.next = ROOT`.
fn only_child() -> Fenced<2> {
    Fenced::new([node(NIL, 1, 1, 0), node(ROOT, NIL, NIL, LASTOB)])
}

/// `ROOT` with two children 1 and 2 in that order: `ROOT.head = 1`,
/// `ROOT.tail = 2`, `1.next = 2`, `2.next = ROOT`.
fn two_children() -> Fenced<3> {
    Fenced::new([node(NIL, 1, 2, 0), node(2, NIL, NIL, 0), node(ROOT, NIL, NIL, LASTOB)])
}

// ── (a) only child, new_pos = 0 — the A47 finding ─────────────────────────

/// `gemoblib.c:1215` `ob_delete` → `ROOT.head = ROOT.tail = NIL` (`:1164-1175`,
/// `1.next` untouched); `:1216` `chg_obj = NIL`; `:1219-1220` `1.next = NIL`,
/// `ROOT.head = 1`; `:1242` `NIL != ROOT` → `ROOT.tail` stays `NIL`; `:1245`
/// `TRUE`. Field by field:
///
/// | field | before | EmuTOS after |
/// |---|---|---|
/// | `ROOT.ob_head` | 1 | **1** |
/// | `ROOT.ob_tail` | 1 | **NIL** |
/// | `ROOT.ob_next` | NIL | NIL |
/// | `1.ob_next` | ROOT | **NIL** |
/// | `1.ob_head` / `1.ob_tail` | NIL | NIL |
#[test]
fn only_child_to_head_leaves_tail_and_next_at_nil_as_emutos_does() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut t = only_child();

    let ret = objc_order(&mut pb, &mut t, 1, 0);

    assert_eq!(ret, 1, "ob_order answers TRUE (`gemoblib.c:1245`)");
    assert_eq!(
        links(t.at(0)),
        (NIL, 1, NIL),
        "ROOT (next, head, tail): the delete cleared the tail and the head branch does not restore it (`:1171,1220,1242`)"
    );
    assert_eq!(
        links(t.at(1)),
        (NIL, NIL, NIL),
        "object 1 (next, head, tail): `movptr->ob_next = chg_obj` with `chg_obj == NIL` (`:1216,1219`)"
    );
    t.assert_fences("only child to head");
}

// ── (b) only child, new_pos = NIL — EmuTOS has no result here ─────────────

/// `chg_obj = parentptr->ob_tail = NIL` (`:1226`) and `treeptr + NIL` is the
/// record **before the tree** (`:1237-1239`): EmuTOS reads and writes it.
/// NewTOS' defined answer: the object is the parent's sole child again
/// (`1.next = ROOT`, `ROOT.head = ROOT.tail = 1`), `TRUE`, and neither fence
/// is touched.
#[test]
fn only_child_to_tail_relinks_it_as_sole_child_without_writing_outside_the_tree() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut t = only_child();

    let ret = objc_order(&mut pb, &mut t, 1, NIL);

    assert_eq!(ret, 1);
    assert_eq!(links(t.at(0)), (NIL, 1, 1), "ROOT (next, head, tail)");
    assert_eq!(links(t.at(1)), (ROOT, NIL, NIL), "object 1 (next, head, tail)");
    t.assert_fences("only child to tail");
}

// ── (c) two children, the last one to the head ────────────────────────────

/// `ob_delete(2)`: `prev = 1` (`get_prev`, `:987-1006`), `1.next = ROOT`,
/// `ROOT.tail = 1` (`:1179-1189`). Then `chg_obj = ROOT.head = 1`, head
/// branch: `2.next = 1`, `ROOT.head = 2`; `1 != ROOT` → tail stays 1.
#[test]
fn last_of_two_to_head_closes_the_chain_the_other_way_round() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut t = two_children();

    let ret = objc_order(&mut pb, &mut t, 2, 0);

    assert_eq!(ret, 1);
    assert_eq!(links(t.at(0)), (NIL, 2, 1), "ROOT (next, head, tail)");
    assert_eq!(links(t.at(2)), (1, NIL, NIL), "object 2 (next, head, tail): now first, chained to 1");
    assert_eq!(links(t.at(1)), (ROOT, NIL, NIL), "object 1 (next, head, tail): now last, chained to ROOT");
    t.assert_fences("last of two to head");
}

// ── (d) two children, the first one to the tail ───────────────────────────

/// `ob_delete(1)`: head child, not the only one → `ROOT.head = 2`
/// (`:1162-1175`). `new_pos == NIL` → `chg_obj = ROOT.tail = 2` (`:1226`);
/// `1.next = 2.next = ROOT`, `2.next = 1` (`:1237-1239`); `1.next == ROOT` →
/// `ROOT.tail = 1` (`:1242-1243`). The `wm_mktop` move.
#[test]
fn first_of_two_to_tail_closes_the_chain_the_other_way_round() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut t = two_children();

    let ret = objc_order(&mut pb, &mut t, 1, NIL);

    assert_eq!(ret, 1);
    assert_eq!(links(t.at(0)), (NIL, 2, 1), "ROOT (next, head, tail)");
    assert_eq!(links(t.at(2)), (1, NIL, NIL), "object 2 (next, head, tail): now first, chained to 1");
    assert_eq!(links(t.at(1)), (ROOT, NIL, NIL), "object 1 (next, head, tail): now last, chained to ROOT");
    t.assert_fences("first of two to tail");
}

// ── (e) ROOT cannot be moved ──────────────────────────────────────────────

/// `if (mov_obj == ROOT) return FALSE;` (`:1208-1209`) before anything is
/// written.
#[test]
fn root_is_refused_and_the_tree_is_untouched() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    let mut t = two_children();

    let ret = objc_order(&mut pb, &mut t, ROOT, 0);

    assert_eq!(ret, 0, "ob_order answers FALSE for ROOT (`gemoblib.c:1208-1209`)");
    assert_eq!(links(t.at(0)), (NIL, 1, 2));
    assert_eq!(links(t.at(1)), (2, NIL, NIL));
    assert_eq!(links(t.at(2)), (ROOT, NIL, NIL));
    t.assert_fences("ROOT refused");
}
