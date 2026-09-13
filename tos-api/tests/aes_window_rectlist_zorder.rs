//! The owner rectangle list subtracts the covering windows in **ascending
//! z-order**, never in handle/slot order.
//!
//! `draw_change` rebuilds every window's list with one sweep,
//! `everyobj(gl_wtree, ROOT, NIL, newrect, …)` (`aes/gemwmlib.c:1089`), and
//! `everyobj` walks the `W_TREE` sibling chain `ob_head` → `ob_tail`
//! (`aes/gemobjop.c:76-131`). That chain **is** the z-order by construction:
//! `w_obadd` appends a newly opened window at `ob_tail` (`gemwmlib.c:186-201`,
//! from `wm_open` `:1425`) and `wm_mktop` moves a topped one there with
//! `ob_order(gl_wtree, w_handle, NIL)` (`:1605-1610`, `gemoblib.c:1201-1240`,
//! the `new_pos == NIL` branch). Inside the sweep, `newrect(tree, wh)` first
//! resets `wh`'s own list to its unbroken `WS_TRUE` rect and then calls
//! `everyobj(tree, ROOT, wh, mkrect, …)` (`gemwrect.c:165-205`), which breaks
//! the list of every window *below* `wh` with `wh`'s rect. Read across the
//! whole sweep: a window's list is its own true rect, cut once per window
//! above it, **one at a time, in ascending z-order of those covering
//! windows**.
//!
//! The order is not cosmetic. `brkrct` replaces a *surviving* piece with up to
//! four sub-pieces the moment it is cut (`gemwrect.c:101-132`, `mkpiece`
//! `:64-98`, in the fixed order TOP/LEFT/RIGHT/BOTTOM); a piece an earlier cut
//! already removed is never revisited, and a piece that survives is re-split by
//! every later cut. Cutting with A then B therefore does not in general produce
//! the same fragments as B then A — even though both remove the same union.
//!
//! **Measured, not assumed.** The two groups below are section D of
//! `tosdump/cases/aes-r3-wind-state.tsv` (39 cases, CRC `979FAC60`) and the
//! expected pieces are the ones in `tosdump/reference/EMUTOS14-AES-r3.BIN`,
//! word for word — EmuTOS 1.4 at TT-medium 640×480/4 planes, the geometry this
//! harness supplies. The case id of every piece is named on its row.
//!
//! The spec side is `docs/api/aes/window-graphics.md`, "`wind_get` (104)" §
//! "The rectangle-list iteration" → "The subtraction order is normative —
//! ascending z, not handle order" and its worked example.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};

// ── the AES calls this file issues ───────────────────────────────────────

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_GET: i16 = 104;
const OP_WIND_SET: i16 = 105;
const OP_WIND_NEW: i16 = 109;

const WF_WXYWH: i16 = 4;
const WF_TOP: i16 = 10;
const WF_FIRSTXYWH: i16 = 11;
const WF_NEXTXYWH: i16 = 12;

/// The kind the table creates all three windows with: every gadget bit except
/// `INFO` (`0x0FEF` has no `0x0010`), so the border thicknesses are the ones
/// the reference was measured with.
const KIND: i16 = 0x0FEF;

type R = (i16, i16, i16, i16);

/// The three rectangles, identical in both groups: 1 underneath, 2 over its
/// top left, 3 over its bottom right and overlapping 2 as well.
const W1: R = (30, 40, 320, 240);
const W2: R = (60, 80, 200, 160);
const W3: R = (180, 160, 200, 160);

/// Window 1's work area, `wind_get(1, WF_WXYWH)` — case 431 of the table,
/// which both machines already answer alike. Every piece below is clipped to
/// it by `w_owns` (`gemwmlib.c:1230-1240`), so a test that asserted pieces
/// without pinning this would not know what it was measuring.
const W1_WORK: R = (31, 63, 296, 194);

/// **Group I**, opened 1, 2, 3 — ascending z is 1, 2, 3, so window 1's rect is
/// broken by 2 and then by 3. Four pieces
/// (`EMUTOS14-AES-r3.BIN`, cases 439, 448, 458, 469; the list ends at 481).
const GROUP_I: [R; 4] = [
    (31, 63, 296, 17),  // 439
    (31, 80, 29, 162),  // 448
    (262, 80, 65, 80),  // 458
    (31, 242, 149, 15), // 469
];

/// **Group II**, opened 1, 3, 2 — ascending z is 1, 3, 2, so window 1's rect is
/// broken by 3 and then by 2. The same three rectangles, the same visible area,
/// **five** pieces (cases 519, 528, 538, 549, 561; the list ends at 574).
const GROUP_II: [R; 5] = [
    (31, 63, 296, 17),  // 519
    (31, 80, 29, 80),   // 528
    (262, 80, 65, 80),  // 538
    (31, 160, 29, 82),  // 549
    (31, 242, 149, 15), // 561
];

/// **Group I, the middle window.** Window 2 breaks window 1's list while being
/// broken by window 3 — the case two windows cannot produce. Cases 502, 511.
const GROUP_I_MIDDLE: [R; 2] = [(61, 103, 176, 57), (61, 160, 119, 57)];

// ── the calls ────────────────────────────────────────────────────────────

/// `wind_new` — the reset `tosdump/cases/aes-r3-wind-state.tsv` declares on its
/// `!reset` line, issued before and after every case (`aes/gemwmlib.c:1875-1893`,
/// `tosdump/core/runner_aes.c:150-154`). `OUT_LEN` 0 is not cosmetic: the
/// dispatcher copies `OUT_LEN` words back out (`aes/gemsuper.c:430-432,444-445`).
fn wind_new(pb: &mut Pb) {
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);
}

fn wind_create(pb: &mut Pb) -> i16 {
    pb.poison();
    pb.intin[0] = KIND;
    pb.intin[1] = 10;
    pb.intin[2] = 20;
    pb.intin[3] = 300;
    pb.intin[4] = 200;
    pb.call(OP_WIND_CREATE, 5, 1, 0, 0);
    pb.intout[0]
}

fn wind_open(pb: &mut Pb, handle: i16, r: R) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = r.0;
    pb.intin[2] = r.1;
    pb.intin[3] = r.2;
    pb.intin[4] = r.3;
    pb.call(OP_WIND_OPEN, 5, 1, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_open({handle}, {r:?}) failed");
}

/// `wind_set(handle, WF_TOP)` — `wm_mktop`, which reorders the window tree
/// (`gemwmlib.c:1600-1611`).
fn wind_set_top(pb: &mut Pb, handle: i16) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = WF_TOP;
    pb.intin[2] = 0;
    pb.intin[3] = 0;
    pb.intin[4] = 0;
    pb.intin[5] = 0;
    pb.call(OP_WIND_SET, 6, 1, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_set({handle}, WF_TOP) failed");
}

fn wind_get_rect(pb: &mut Pb, handle: i16, field: i16) -> R {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_get({handle}, {field}) returned FALSE");
    (pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4])
}

/// Walk `handle`'s owner rectangle list the way a redraw handler does:
/// `WF_FIRSTXYWH`, then `WF_NEXTXYWH` until a piece comes back with
/// `w == 0 && h == 0` (`w_owns`' end marker, `gemwmlib.c:1240` — `ret` stays
/// TRUE, so the rectangle is the terminator, not the return value).
///
/// The cap is generous on purpose: a list longer than the expectation must show
/// up as a length mismatch, not as a hang.
fn walk(pb: &mut Pb, handle: i16) -> Vec<R> {
    let mut out = Vec::new();
    let mut field = WF_FIRSTXYWH;
    for _ in 0..24 {
        let r = wind_get_rect(pb, handle, field);
        field = WF_NEXTXYWH;
        if r.2 == 0 && r.3 == 0 {
            break;
        }
        out.push(r);
    }
    out
}

/// Create three windows and open them in `order`, which lists the handles.
/// Returns the three handles in creation order (1, 2, 3).
fn three_windows(pb: &mut Pb, order: [usize; 3]) -> [i16; 3] {
    let h = [wind_create(pb), wind_create(pb), wind_create(pb)];
    assert_eq!(h, [1, 2, 3], "wind_new must hand out handles 1, 2, 3 in creation order");
    let rects = [W1, W2, W3];
    for &i in order.iter() {
        wind_open(pb, h[i], rects[i]);
    }
    h
}

fn area(list: &[R]) -> i32 {
    list.iter().map(|p| p.2 as i32 * p.3 as i32).sum()
}

/// Do the pieces of `list` overlap each other? The owner rectangle list is a
/// **disjoint** decomposition; a re-ordering fix that produced overlaps would
/// make a redraw handler paint the same pixels twice.
fn any_overlap(list: &[R]) -> Option<(R, R)> {
    for (i, a) in list.iter().enumerate() {
        for b in list.iter().skip(i + 1) {
            if a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3 {
                return Some((*a, *b));
            }
        }
    }
    None
}

// ── the tests ────────────────────────────────────────────────────────────

/// Group I is the ordering in which handle order and z-order agree, so it
/// passes before the fix as well. It is here as the regression guard: the fix
/// may not disturb the four pieces the dump already agrees on.
#[test]
fn group_i_opened_1_2_3_gives_the_four_measured_pieces() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    three_windows(&mut pb, [0, 1, 2]);

    let work = wind_get_rect(&mut pb, 1, WF_WXYWH);
    assert_eq!(work, W1_WORK, "window 1's work area (case 431) — every piece is clipped to it");

    let got = walk(&mut pb, 1);
    assert_eq!(
        got,
        GROUP_I.to_vec(),
        "window 1, opened first and covered by 2 then 3: EMUTOS14-AES-r3.BIN cases \
         439/448/458/469 with the list ending at 481"
    );

    wind_new(&mut pb);
}

/// The middle window of group I: broken by window 3 while itself breaking
/// window 1. Two windows cannot produce this.
#[test]
fn group_i_middle_window_is_broken_while_breaking_another() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    three_windows(&mut pb, [0, 1, 2]);

    let got = walk(&mut pb, 2);
    assert_eq!(
        got,
        GROUP_I_MIDDLE.to_vec(),
        "window 2's own list: EMUTOS14-AES-r3.BIN cases 502/511"
    );

    wind_new(&mut pb);
}

/// **The defect.** The same three rectangles, opened 1, 3, 2 instead of
/// 1, 2, 3. Handle order subtracts window 2 before window 3 and answers group
/// I's four pieces; ascending z-order subtracts 3 before 2 and answers the five
/// EmuTOS measured.
#[test]
fn group_ii_opened_1_3_2_gives_the_five_measured_pieces() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    three_windows(&mut pb, [0, 2, 1]);

    let work = wind_get_rect(&mut pb, 1, WF_WXYWH);
    assert_eq!(work, W1_WORK, "the work area may not depend on the opening order");

    let got = walk(&mut pb, 1);
    assert_eq!(
        got,
        GROUP_II.to_vec(),
        "window 1 covered by 3 and then by 2: EMUTOS14-AES-r3.BIN cases \
         519/528/538/549/561 with the list ending at 574. Subtracting in handle \
         order (2 before 3) answers group I's {GROUP_I:?} instead — the covering \
         windows must be subtracted in ascending z-order, not in slot order \
         (aes/gemwmlib.c:1089,186-201 + aes/gemwrect.c:165-205)"
    );

    wind_new(&mut pb);
}

/// Both orders remove the identical union from window 1, so the *area* is the
/// same; what differs is the decomposition. Stating it here keeps the two
/// expectations above honest — a "fix" that dropped a piece would satisfy the
/// count but not this.
#[test]
fn both_orders_cover_the_same_area_with_disjoint_pieces() {
    assert_eq!(
        area(&GROUP_I),
        area(&GROUP_II),
        "the two orderings remove the same union, so the visible area is equal — \
         group I {GROUP_I:?}, group II {GROUP_II:?}"
    );
    assert_ne!(
        GROUP_I.len(),
        GROUP_II.len(),
        "the point of the two groups is that the piece COUNT differs"
    );
    assert_eq!(any_overlap(&GROUP_I), None, "group I's pieces must be disjoint");
    assert_eq!(any_overlap(&GROUP_II), None, "group II's pieces must be disjoint");
}

/// `wind_set(WF_TOP)` reaches the same tree position as opening last, so it has
/// to reach the same rectangle list: `wm_mktop` calls
/// `ob_order(gl_wtree, w_handle, NIL)` and `NIL` appends after the current tail
/// (`gemwmlib.c:1605-1610`, `gemoblib.c:1213-1240`) — the same place `w_obadd`
/// puts a newly opened window (`:186-201`).
///
/// Opening 1, 2, 3 and then topping 2 therefore has to answer exactly what
/// opening 1, 3, 2 answers.
///
/// **Derived from the source, not measured:** the frozen table has no `WF_TOP`
/// case, so this expectation rests on `ob_order`/`w_obadd` alone. It is listed
/// as an open measurement question in `TASK/A22-STATUS-UNFINISHED.MD`.
#[test]
fn topping_a_window_moves_it_in_the_subtraction_order_like_a_later_open() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    three_windows(&mut pb, [0, 1, 2]);
    // 1, 2, 3 → group I. Topping 2 makes the ascending z-order 1, 3, 2.
    wind_set_top(&mut pb, 2);

    let got = walk(&mut pb, 1);
    assert_eq!(
        got,
        GROUP_II.to_vec(),
        "after wind_set(2, WF_TOP) the covering order is 3 then 2, which is \
         group II — the tree position a top gives is the one a later open gives \
         (aes/gemwmlib.c:1605-1610, aes/gemoblib.c:1213-1240)"
    );

    wind_new(&mut pb);
}
