//! The owner rectangle list is as long as EmuTOS's `ORECT` pool, and running
//! past it is not silent.
//!
//! EmuTOS keeps every window's owner rectangle list as `ORECT` cells linked
//! out of one pool of `NUM_ORECT = NUM_WIN * 10` (`aes/gemlib.h:88`) — 160 at
//! `NUM_WIN = 16` (`include/sysconf.h:32`, the AES 0x0340 table NewTOS
//! mirrors). Each covering window breaks every piece it overlaps into up to
//! four (`brkrct`, `aes/gemwrect.c:101-132`, in the order TOP/LEFT/RIGHT/
//! BOTTOM `:30-33,118-122`), so a window under several others owns far more
//! pieces than there are windows over it: the seven strips of
//! [`thirty_pieces_all_reach_the_caller`] leave **thirty**.
//!
//! Until A21 NewTOS held sixteen per window (`RCAP = 16`,
//! `tos-api/src/aes/window.rs`) and dropped the rest without a word:
//! `WF_FIRSTXYWH`/`WF_NEXTXYWH` ended early and a redraw handler that walks
//! the list — every conforming one — left the missing area unpainted. The
//! first test is red against that and green with the capacity raised to the
//! pool's size.
//!
//! Beyond the pool EmuTOS has no behaviour: `get_orect` returns the free-list
//! head, `NULL` once the list is empty (`gemwrect.c:53-61`), and `mkpiece`
//! writes `rl->o_link = old` and four rectangle words straight through it
//! (`:68-75`); `newrect` (`:201-203`) and `wm_init` (`gemwmlib.c:1287-1289`)
//! do the same. [`the_pool_limit_stops_the_open_that_crosses_it`] pins what
//! NewTOS does at that point instead — stop with a named panic, in the very
//! call (`wind_open` → `draw_change` → `newrect`) that would have run
//! EmuTOS's pool dry — and, on the way there, that a list of 159 pieces still
//! reaches the caller whole.
//!
//! **Where the expectations come from.** The pieces are computed by
//! [`reference_list`], the `brkrct`/`mkpiece` decomposition itself, applied in
//! ascending z-order to the window's `WS_TRUE` rectangle with each covering
//! window's `WS_TRUE` rectangle (`newrect`, `gemwrect.c:190,203`;
//! `w_getsize`, `gemwmlib.c:254-262`). That algorithm — order and boundaries —
//! is contract (`docs/api/aes/window-graphics.md`, "`wind_get` (104)" §
//! "The rectangle-list iteration") and is pinned word for word to the TT
//! measurement by `aes_window_rectlist_zorder.rs`; this file only asks it for
//! *more* pieces. Independently of the decomposition, a pixel mask checks
//! that the pieces delivered cover the window's visible work area exactly.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use std::panic::{catch_unwind, AssertUnwindSafe};

use harness::{context, Pb, AES_LOCK, SCREEN_H, SCREEN_W};

// ── the AES calls this file issues ───────────────────────────────────────

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_CLOSE: i16 = 102;
const OP_WIND_GET: i16 = 104;
const OP_WIND_NEW: i16 = 109;

const WF_WXYWH: i16 = 4;
const WF_FIRSTXYWH: i16 = 11;
const WF_NEXTXYWH: i16 = 12;

/// The desktop window, `DESKWH` (`aes/gemwmlib.h:13`). It has an owner
/// rectangle list like any other window (`wm_init`, `gemwmlib.c:1287-1289`;
/// rebuilt by every `draw_change` sweep, `:1089`), and `wind_get(0,
/// WF_FIRSTXYWH)` walks it — measured, `tosdump/cases/aes-r2b-rlist.tsv`
/// cases 201/208.
const DESKWH: i16 = 0;

/// EmuTOS `NUM_ORECT = NUM_WIN * 10` (`aes/gemlib.h:88`) at `NUM_WIN = 16`
/// (`include/sysconf.h:32`, `AES_VERSION > 0x0320`).
const NUM_ORECT: usize = 16 * 10;

/// What the old per-window array held. Named here only so the tests can say
/// "more than that".
const OLD_RCAP: usize = 16;

/// `DROP_SHADOW_SIZE` (`gemwmlib.c:49`): `WS_TRUE` adds it to a window's width
/// and height (`w_getsize`, `:254-262`), and `WS_TRUE` is both the rectangle a
/// list starts from and the rectangle each covering window breaks it with
/// (`newrect`, `gemwrect.c:190,203`).
const S: i16 = tos_api::aes::window::DROP_SHADOW_SIZE;

/// Every gadget except `INFO`, the kind the measured tables use.
const KIND_FULL: i16 = 0x0FEF;
/// `NONE`: a window with no gadgets at all — the covering strips. Their kind
/// does not enter the lists (only `WS_TRUE` does), and a gadget-less window
/// can be a few pixels wide without its gadgets overlapping.
const KIND_NONE: i16 = 0;

type R = (i16, i16, i16, i16);

// ── the calls ────────────────────────────────────────────────────────────

fn wind_new(pb: &mut Pb) {
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);
}

fn wind_create(pb: &mut Pb, kind: i16, full: R) -> i16 {
    pb.poison();
    pb.intin[0] = kind;
    pb.intin[1] = full.0;
    pb.intin[2] = full.1;
    pb.intin[3] = full.2;
    pb.intin[4] = full.3;
    pb.call(OP_WIND_CREATE, 5, 1, 0, 0);
    let h = pb.intout[0];
    assert!(h > 0, "wind_create({kind:#x}, {full:?}) returned {h}");
    h
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

fn wind_close(pb: &mut Pb, handle: i16) {
    pb.poison();
    pb.intin[0] = handle;
    pb.call(OP_WIND_CLOSE, 1, 1, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_close({handle}) failed");
}

fn wind_get_rect(pb: &mut Pb, handle: i16, field: i16) -> R {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_get({handle}, {field}) returned FALSE");
    (pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4])
}

/// Walk `handle`'s owner rectangle list as a redraw handler does:
/// `WF_FIRSTXYWH`, then `WF_NEXTXYWH` until a piece comes back with
/// `w == 0 && h == 0` (`w_owns`, `gemwmlib.c:1240`). The cap is well above
/// the pool so a runaway list fails as a length, not as a hang.
fn walk(pb: &mut Pb, handle: i16) -> Vec<R> {
    let mut out = Vec::new();
    let mut field = WF_FIRSTXYWH;
    for _ in 0..(2 * NUM_ORECT) {
        let r = wind_get_rect(pb, handle, field);
        field = WF_NEXTXYWH;
        if r.2 == 0 && r.3 == 0 {
            return out;
        }
        out.push(r);
    }
    panic!("wind_get({handle}) list did not end within {} pieces", 2 * NUM_ORECT);
}

// ── the reference decomposition (`aes/gemwrect.c`) ───────────────────────

/// `w_getsize(WS_TRUE, …)`: the rectangle plus the drop shadow
/// (`gemwmlib.c:254-262`).
fn ws_true(r: R) -> R {
    (r.0, r.1, r.2 + S, r.3 + S)
}

/// `brkrct` + `mkpiece` (`aes/gemwrect.c:64-132`) for one covering rectangle
/// `c` over the list `pieces`: a piece `c` overlaps is replaced, in place, by
/// its TOP, LEFT, RIGHT and BOTTOM remainders — those it has, in that order.
fn brkrct(pieces: &[R], c: R) -> Vec<R> {
    let mut out = Vec::new();
    for &(x, y, w, h) in pieces {
        let (cx, cy, cw, ch) = c;
        // `brkrct`'s overlap test, `:107-110`.
        if !(cx < x + w && cx + cw > x && cy < y + h && cy + ch > y) {
            out.push((x, y, w, h));
            continue;
        }
        // `have_piece[]`, `:113-116`; `mkpiece`'s formulas, `:72-94`.
        if cy > y {
            out.push((x, y, w, cy - y));
        }
        let oy = y.max(cy);
        let oh = (y + h).min(cy + ch) - oy;
        if cx > x {
            out.push((x, oy, cx - x, oh));
        }
        if cx + cw < x + w {
            out.push((cx + cw, oy, (x + w) - (cx + cw), oh));
        }
        if cy + ch < y + h {
            out.push((x, cy + ch, w, (y + h) - (cy + ch)));
        }
    }
    out
}

/// The raw owner rectangle list of a window whose current rectangle is
/// `curr`, covered by `covers` (their current rectangles, in ascending
/// z-order) — `newrect` for that window across one `draw_change` sweep
/// (`gemwrect.c:165-205`, `gemwmlib.c:1089`).
fn reference_list(curr: R, covers: &[R]) -> Vec<R> {
    let mut pieces = vec![ws_true(curr)];
    for &c in covers {
        pieces = brkrct(&pieces, ws_true(c));
    }
    pieces
}

/// `rc_intersect` (`util/rectfunc.c:63-78`) as `w_owns` applies it
/// (`gemwmlib.c:1230-1240`): the piece clipped to the work area, or nothing
/// if the clip is empty — which the walk steps over.
fn clip_to(r: R, work: R) -> Option<R> {
    let tw = (r.0 + r.2).min(work.0 + work.2);
    let th = (r.1 + r.3).min(work.1 + work.3);
    let tx = r.0.max(work.0);
    let ty = r.1.max(work.1);
    (tw > tx && th > ty).then_some((tx, ty, tw - tx, th - ty))
}

/// What `WF_FIRSTXYWH`/`WF_NEXTXYWH` deliver for that list: each piece
/// clipped to `work`, the empty clips skipped.
fn expected_walk(curr: R, covers: &[R], work: R) -> Vec<R> {
    reference_list(curr, covers).into_iter().filter_map(|r| clip_to(r, work)).collect()
}

// ── the geometric checks that do not depend on the decomposition ─────────

/// A screen-sized mask, one bool per pixel.
struct Mask(Vec<bool>);

impl Mask {
    fn new() -> Mask {
        Mask(vec![false; SCREEN_W as usize * SCREEN_H as usize])
    }

    fn set(&mut self, r: R, v: bool) {
        for y in r.1.max(0)..(r.1 + r.3).min(SCREEN_H as i16) {
            for x in r.0.max(0)..(r.0 + r.2).min(SCREEN_W as i16) {
                self.0[y as usize * SCREEN_W as usize + x as usize] = v;
            }
        }
    }

    fn count(&self) -> usize {
        self.0.iter().filter(|&&b| b).count()
    }

    fn first_difference(&self, other: &Mask) -> Option<(i16, i16)> {
        self.0.iter().zip(other.0.iter()).position(|(a, b)| a != b).map(|i| {
            ((i % SCREEN_W as usize) as i16, (i / SCREEN_W as usize) as i16)
        })
    }
}

/// The visible part of `work`: everything a covering window's `WS_TRUE`
/// rectangle does not reach.
fn visible_mask(work: R, covers: &[R]) -> Mask {
    let mut m = Mask::new();
    m.set(work, true);
    for &c in covers {
        m.set(ws_true(c), false);
    }
    m
}

fn pieces_mask(pieces: &[R]) -> Mask {
    let mut m = Mask::new();
    for &p in pieces {
        m.set(p, true);
    }
    m
}

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

fn inside(r: R, work: R) -> bool {
    r.2 > 0 && r.3 > 0 && r.0 >= work.0 && r.1 >= work.1 && r.0 + r.2 <= work.0 + work.2 && r.1 + r.3 <= work.1 + work.3
}

/// Every check a delivered list has to pass against the window's work area
/// and the windows covering it — disjoint, inside, covering the visible area
/// exactly (pixel for pixel), and equal to the reference decomposition.
fn check_list(what: &str, got: &[R], curr: R, covers: &[R], work: R) {
    for r in got {
        assert!(inside(*r, work), "{what}: piece {r:?} is not inside the work area {work:?}");
    }
    assert_eq!(any_overlap(got), None, "{what}: two delivered pieces overlap");

    let visible = visible_mask(work, covers);
    let delivered = pieces_mask(got);
    if let Some((x, y)) = visible.first_difference(&delivered) {
        panic!(
            "{what}: the delivered pieces do not cover the visible work area exactly — first \
             difference at ({x}, {y}); visible {} pixels, delivered {} pixels, {} pieces",
            visible.count(),
            delivered.count(),
            got.len()
        );
    }

    let expected = expected_walk(curr, covers, work);
    assert_eq!(
        got,
        &expected[..],
        "{what}: the delivered sequence is not the brkrct/mkpiece decomposition \
         (aes/gemwrect.c:64-132) clipped by w_owns (gemwmlib.c:1230-1240)"
    );
}

// ── case 1: more than sixteen pieces ─────────────────────────────────────

/// The window underneath.
const W1: R = (30, 40, 500, 400);

/// Seven strips over it: three vertical, then four horizontal crossing them.
/// Every crossing multiplies the pieces; the reference decomposition of `W1`
/// under these is thirty pieces, all of them inside `W1`'s work area.
const STRIPS: [R; 7] = [
    (100, 90, 4, 260),
    (220, 90, 4, 260),
    (340, 90, 4, 260),
    (60, 110, 420, 4),
    (60, 180, 420, 4),
    (60, 250, 420, 4),
    (60, 320, 420, 4),
];

/// **The defect.** Seven small windows over one large one leave the large
/// one thirty visible pieces — nearly twice the old capacity. With `RCAP =
/// 16` the list ended after sixteen and the caller never saw the other
/// fourteen: the pixel mask comes up short and the sequence is truncated.
#[test]
fn thirty_pieces_all_reach_the_caller() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h1 = wind_create(&mut pb, KIND_FULL, W1);
    assert_eq!(h1, 1);
    wind_open(&mut pb, h1, W1);
    for &s in STRIPS.iter() {
        let h = wind_create(&mut pb, KIND_NONE, s);
        wind_open(&mut pb, h, s);
    }

    let work = wind_get_rect(&mut pb, h1, WF_WXYWH);
    let expected = expected_walk(W1, &STRIPS, work);
    assert!(
        expected.len() > OLD_RCAP,
        "the arrangement must produce more than {OLD_RCAP} pieces to test anything — the \
         reference decomposition gives {}",
        expected.len()
    );
    assert_eq!(expected.len(), 30, "the arrangement is designed for thirty pieces");

    let got = walk(&mut pb, h1);
    assert!(
        got.len() > OLD_RCAP,
        "window 1 under seven strips owns {} pieces but only {} were delivered — the list \
         was cut at the old RCAP = {OLD_RCAP} and the rest dropped silently",
        expected.len(),
        got.len()
    );
    check_list("window 1 under seven strips", &got, W1, &STRIPS, work);

    wind_new(&mut pb);
}

// ── case 2: past EmuTOS's pool ───────────────────────────────────────────

/// Fifteen windows — every application handle of the sixteen-slot table —
/// over the desktop, arranged so that the desktop's list grows past the pool
/// on the **last** open and not before: nine vertical and six horizontal
/// hairline strips, alternating, each one further out and longer than the
/// one before it, so that each new strip crosses the TOP/BOTTOM (or
/// LEFT/RIGHT) remainders every earlier strip left behind. The reference
/// decomposition of the desktop's `WS_TRUE` rectangle under the first
/// fourteen is 159 pieces; under all fifteen, 190.
fn fifteen_strips() -> Vec<R> {
    let vertical: Vec<R> = (0..9).map(|i| (400 - 30 * i, 200 - 5 * i, 1, 100 + 10 * i)).collect();
    let horizontal: Vec<R> = (0..6).map(|j| (300 - 20 * j, 250 + 5 * j, 100 + 40 * j, 1)).collect();
    let mut out = Vec::new();
    for i in 0..9 {
        out.push(vertical[i]);
        if i < 6 {
            out.push(horizontal[i]);
        }
    }
    out
}

/// The desktop window's current rectangle — `gl_rscreen` (`wm_init`,
/// `gemwmlib.c:1291`), the whole screen — and its work area, `gl_rfull`
/// (`:1294`, the screen below the menu bar), read back through `wind_get`.
fn desktop_curr() -> R {
    (0, 0, SCREEN_W as i16, SCREEN_H as i16)
}

/// **Past the pool.** Fourteen strips leave the desktop 159 pieces — one
/// short of `NUM_ORECT` — and every one of them reaches the caller. The
/// fifteenth would make 190. In EmuTOS that `wind_open`'s `draw_change` runs
/// `get_orect` dry and `mkpiece` writes through the `NULL` it gets back
/// (`gemwrect.c:53-61`, `:68-75`): no error, no message, the vector table at
/// address 0 overwritten and the list cut short. NewTOS has no such path to
/// reimplement; it stops that call with a panic that names the pool, and
/// this test pins that the stop happens **there** — not one window earlier
/// (the capacity is the pool's, not sixteen) and not silently (the open must
/// not succeed with a shortened list).
#[test]
fn the_pool_limit_stops_the_open_that_crosses_it() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let strips = fifteen_strips();
    assert_eq!(strips.len(), 15);
    let curr = desktop_curr();
    let work = wind_get_rect(&mut pb, DESKWH, WF_WXYWH);

    // The arrangement is designed to cross the pool exactly at the last open.
    let counts: Vec<usize> = (1..=15).map(|n| reference_list(curr, &strips[..n]).len()).collect();
    assert!(
        counts[13] <= NUM_ORECT && counts[14] > NUM_ORECT,
        "the arrangement must fit the pool under fourteen strips and exceed it under fifteen; \
         the reference decomposition gives {counts:?} against NUM_ORECT = {NUM_ORECT}"
    );
    assert_eq!((counts[13], counts[14]), (159, 190), "the arrangement is designed for 159 then 190");

    let handles: Vec<i16> = strips.iter().map(|&s| wind_create(&mut pb, KIND_NONE, s)).collect();
    assert_eq!(handles, (1..=15).collect::<Vec<i16>>(), "the sixteen-slot table hands out 1..=15");

    // Fourteen opens, each one checked: the desktop's whole list reaches the
    // caller at every size on the way up to 159.
    for n in 0..14 {
        wind_open(&mut pb, handles[n], strips[n]);
        let got = walk(&mut pb, DESKWH);
        check_list(&format!("desktop under {} strips", n + 1), &got, curr, &strips[..=n], work);
    }
    let got = walk(&mut pb, DESKWH);
    assert_eq!(
        got.len(),
        expected_walk(curr, &strips[..14], work).len(),
        "fourteen strips: the whole 159-piece list (159 raw, all inside the work area) is delivered"
    );
    assert!(got.len() > OLD_RCAP, "the list is far past the old sixteen by now");

    // The fifteenth open crosses the pool. EmuTOS: undefined. NewTOS: stop.
    let outcome = catch_unwind(AssertUnwindSafe(|| wind_open(&mut pb, handles[14], strips[14])));
    let payload = match outcome {
        Ok(()) => panic!(
            "the fifteenth open succeeded — the desktop's list would be {} pieces against a pool \
             of {NUM_ORECT}; EmuTOS has no defined behaviour here and NewTOS must stop rather \
             than deliver a shortened list",
            counts[14]
        ),
        Err(p) => p,
    };
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&'static str>().copied())
        .unwrap_or("<non-string panic payload>")
        .to_string();
    assert!(
        message.contains("NUM_ORECT") && message.contains("gemwrect.c"),
        "the stop must name the pool and the EmuTOS line it stands in for; got: {message}"
    );

    // Back out top-down: every state on the way down is one the way up
    // already delivered whole, so no close can trip the limit again. The
    // fifteenth window is open — `wm_open` sets `VF_ISOPEN` before
    // `draw_change` — and comes off first.
    for &h in handles.iter().rev() {
        wind_close(&mut pb, h);
    }
    wind_new(&mut pb);
}
