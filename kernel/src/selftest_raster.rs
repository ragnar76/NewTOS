//! VDI raster self-test (`vro_cpyfm` / `vrt_cpyfm`, opcodes 109 / 121).
//!
//! Two layers, matching how the audit found the bugs:
//!
//! 1. **Numeric** — the pure 16-entry BitBlt op table
//!    ([`crate::vdi_backend::raster_op`]) checked against the boolean
//!    spec (`docs/api/vdi/raster/_raster.md:378-382`) for all 16 ops on a
//!    known `(S, D)`, incl. `op 5 = D`, `op 7 = S | D`, and `0x16`
//!    (`XOR | PAT_FLAG`) collapsing to op 6 after the low-nibble mask.
//! 2. **Observable** — the real VDI trap path (`svc #2`): blit into a
//!    scratch screen region and read the pixels back (`v_get_pixel` via
//!    the backend), proving the `raster.rs` decode: `vro_cpyfm`'s
//!    replace / XOR / OR / D-only / invalid handling and PAT_FLAG strip,
//!    and `vrt_cpyfm`'s four `MD_*` modes plus the invalid (0 / ≥5)
//!    no-op — including that `MD_ERASE` paints the **background**
//!    (`INTIN[2]`) colour on the 0-bits.
//!
//! The visual `demo()` leaves an XOR-once / XOR-twice / OR-overlay strip
//! on screen for a QMP `screendump`.

use tos_api::vdi::backend::RasterBackend;

use crate::kprintln;
use crate::vdi_backend::{raster_op, VDI_SCREEN};
use crate::vdi_client::{Mfdb, Vdi};

// GEM palette indices used by the observable tests (see
// `vdi_backend::DEFAULT_PALETTE`): pure entries so `read_pixel` maps back
// exactly.
const RED: i16 = 2; // 0xFF0000
const GREEN: i16 = 3; // 0x00FF00
const GREY: i16 = 8; // 0xC0C0C0
const YELLOW: u8 = 6; // 0xFFFF00 == RED | GREEN on the pixel word

/// Read the palette index at `(x, y)` straight from the backend
/// (equivalent to `v_get_pixel`, without a second trap round-trip).
fn pel_at(x: i16, y: i16) -> u8 {
    VDI_SCREEN.read_pixel(x, y)
}

/// Numeric check of the 16-entry logic-op table on a known `(S, D)`.
/// Returns the number of failing assertions.
fn test_op_table() -> u32 {
    // `S` and `D` carry all four `(s, d)` bit combinations across their
    // bit positions, so any mis-mapped / swapped op yields a different
    // 32-bit result and is detected.
    let s: u32 = 0xF0F0_F0F0;
    let d: u32 = 0xCCCC_CCCC;
    // (op, expected D') straight from the boolean spec — the RHS is the
    // independent definition, not `raster_op`.
    let cases: [(u8, u32); 16] = [
        (0, 0),
        (1, s & d),
        (2, s & !d),
        (3, s),
        (4, !s & d),
        (5, d),
        (6, s ^ d),
        (7, s | d),
        (8, !(s | d)),
        (9, !(s ^ d)),
        (10, !d),
        (11, s | !d),
        (12, !s),
        (13, !s | d),
        (14, !(s & d)),
        (15, !0),
    ];
    let mut fails = 0;
    for (op, want) in cases {
        let got = raster_op(op, s, d);
        if got != want {
            kprintln!("[raster]   op {:2} FAIL: got {:#010x} want {:#010x}", op, got, want);
            fails += 1;
        }
    }
    // Spotlighted contract points.
    let checks: [(&str, bool); 4] = [
        ("op 5 = D (unchanged)", raster_op(5, s, d) == d),
        ("op 7 = S | D (OR)", raster_op(7, s, d) == (s | d)),
        ("op 6 = S ^ D (XOR)", raster_op(6, s, d) == (s ^ d)),
        // PAT_FLAG (0x10) is masked off by the low nibble → 0x16 == op 6.
        ("0x16 (XOR|PAT) -> op 6", raster_op(0x16, s, d) == raster_op(6, s, d)),
    ];
    for (label, ok) in checks {
        kprintln!("[raster]   {}: {}", label, if ok { "OK" } else { "FAIL" });
        if !ok {
            fails += 1;
        }
    }
    if fails == 0 {
        kprintln!("[raster]   16-op table: all 16 ops + spotlights OK");
    }
    fails
}

/// Observable `vro_cpyfm` (109) test through the real trap: copy a red
/// source block over a green destination block with several logic ops and
/// read the result back. Returns the number of failing assertions.
fn test_vro(v: &mut Vdi) -> u32 {
    // Non-overlapping source (red) and destination (green) blocks.
    let (srx, sry) = (400, 300);
    let (drx, dry) = (440, 300);
    let (w, h) = (16, 16);
    let (px, py) = (drx + 4, dry + 4); // a pixel well inside the dst block

    let mut fails = 0;
    let expect = |label: &str, got: u8, want: u8, fails: &mut u32| {
        let ok = got == want;
        kprintln!("[raster]   vro {}: {} (pel {} want {})", label, if ok { "OK" } else { "FAIL" }, got, want);
        if !ok {
            *fails += 1;
        }
    };

    v.set_fill(RED);
    v.fill_rect(srx, sry, srx + w - 1, sry + h - 1);

    let reset_dst = |v: &mut Vdi| {
        v.set_fill(GREEN);
        v.fill_rect(drx, dry, drx + w - 1, dry + h - 1);
    };

    // op 3 (S_ONLY / replace): dst becomes the source (red).
    reset_dst(v);
    v.copy_screen(srx, sry, w, h, drx, dry, 3);
    expect("op3 replace -> red", pel_at(px, py), RED as u8, &mut fails);

    // op 7 (S_OR_D): red | green == yellow on the pixel word.
    reset_dst(v);
    v.copy_screen(srx, sry, w, h, drx, dry, 7);
    expect("op7 OR -> yellow", pel_at(px, py), YELLOW, &mut fails);

    // op 6 (S_XOR_D) applied twice restores the destination (green).
    reset_dst(v);
    v.copy_screen(srx, sry, w, h, drx, dry, 6);
    v.copy_screen(srx, sry, w, h, drx, dry, 6);
    expect("op6 XOR x2 -> green", pel_at(px, py), GREEN as u8, &mut fails);

    // op 5 (D_ONLY): destination untouched (stays green).
    reset_dst(v);
    v.copy_screen(srx, sry, w, h, drx, dry, 5);
    expect("op5 D-only -> green", pel_at(px, py), GREEN as u8, &mut fails);

    // 0x16 (XOR | PAT_FLAG): PAT_FLAG stripped -> op 6 XOR once -> yellow.
    reset_dst(v);
    v.copy_screen(srx, sry, w, h, drx, dry, 0x16);
    expect("0x16 XOR|PAT -> yellow", pel_at(px, py), YELLOW, &mut fails);

    // 0x20 (bit above PAT_FLAG set): invalid mode -> no-op (stays green).
    reset_dst(v);
    v.copy_screen(srx, sry, w, h, drx, dry, 0x20);
    expect("0x20 invalid -> green", pel_at(px, py), GREEN as u8, &mut fails);

    fails
}

/// Observable `vrt_cpyfm` (121) test: a 1-plane mono source whose left 8
/// columns are set and right 8 clear, blitted into a grey base block in
/// each `MD_*` mode; read back a left (set) and right (clear) pixel.
/// Returns the number of failing assertions.
fn test_vrt(v: &mut Vdi) -> u32 {
    // Mono source: 16 wide x 8 tall, one plane. Each row 0xFF00 → columns
    // 0..7 set (bit 0x8000>>col), columns 8..15 clear.
    let bits: [u16; 8] = [0xFF00; 8];
    let mfdb = Mfdb {
        addr: bits.as_ptr() as u64,
        w: 16,
        h: 8,
        wdwidth: 1,
        stand: 0,
        nplanes: 1,
        r1: 0,
        r2: 0,
        r3: 0,
    };
    let src = &mfdb as *const Mfdb as u64;
    let (bx, by) = (400, 340);
    let (setx, clrx, ry) = (bx + 2, bx + 10, by + 3); // left(set)/right(clear) probes

    let reset = |v: &mut Vdi| {
        v.set_fill(GREY);
        v.fill_rect(bx, by, bx + 15, by + 7);
    };

    let mut fails = 0;
    let probe = |v: &mut Vdi, mode: i16, label: &str, want_set: i16, want_clr: i16, fails: &mut u32| {
        reset(v);
        // fg = RED (INTIN[1]), bg = GREEN (INTIN[2]).
        v.blit_mono(src, 16, 8, bx, by, RED, GREEN, mode);
        let (gs, gc) = (pel_at(setx, ry), pel_at(clrx, ry));
        // A negative `want` means "changed / any value != GREY" (used for
        // XOR, whose complement colour is palette-dependent).
        let ok_set = if want_set < 0 { gs != GREY as u8 } else { gs == want_set as u8 };
        let ok_clr = if want_clr < 0 { gc != GREY as u8 } else { gc == want_clr as u8 };
        let ok = ok_set && ok_clr;
        kprintln!("[raster]   vrt {}: {} (set-bit pel {}, clear-bit pel {})", label, if ok { "OK" } else { "FAIL" }, gs, gc);
        if !ok {
            *fails += 1;
        }
    };

    // MD_REPLACE(1): set→fg(red), clear→bg(green).
    probe(v, 1, "MD_REPLACE  set=red clear=green", RED, GREEN, &mut fails);
    // MD_TRANS(2): set→fg(red), clear→untouched(grey).
    probe(v, 2, "MD_TRANS    set=red clear=grey", RED, GREY, &mut fails);
    // MD_XOR(3): set→complement(changed), clear→untouched(grey).
    probe(v, 3, "MD_XOR      set=changed clear=grey", -1, GREY, &mut fails);
    // MD_ERASE(4): set→untouched(grey), clear→bg(green)  ← bg = INTIN[2].
    probe(v, 4, "MD_ERASE    set=grey clear=green", GREY, GREEN, &mut fails);
    // mode 0: invalid → whole block untouched (grey/grey).
    probe(v, 0, "mode 0 invalid -> no-op", GREY, GREY, &mut fails);
    // mode 5: invalid (≥5) → whole block untouched (grey/grey).
    probe(v, 5, "mode 5 invalid -> no-op", GREY, GREY, &mut fails);

    fails
}

/// Leave a visible XOR-once / XOR-twice / OR-overlay strip on screen for a
/// QMP `screendump` (source blocks on the right, results on the left).
fn demo(v: &mut Vdi) {
    let sz = 48;
    // Source blocks (kept intact, used as the blit source).
    v.set_fill(RED);
    v.fill_rect(520, 380, 520 + sz - 1, 380 + sz - 1); // red source
    v.set_fill(GREEN);
    v.fill_rect(520, 300, 520 + sz - 1, 300 + sz - 1); // green source

    // Block A: blue, XOR red once (visibly changed).
    v.set_fill(4); // blue
    v.fill_rect(60, 380, 60 + sz - 1, 380 + sz - 1);
    v.copy_screen(520, 380, sz, sz, 60, 380, 6);

    // Block B: blue, XOR red twice (restored to blue — overlay gone).
    v.set_fill(4);
    v.fill_rect(160, 380, 160 + sz - 1, 380 + sz - 1);
    v.copy_screen(520, 380, sz, sz, 160, 380, 6);
    v.copy_screen(520, 380, sz, sz, 160, 380, 6);

    // Block C: red, OR green overlay (→ yellow).
    v.set_fill(RED);
    v.fill_rect(260, 380, 260 + sz - 1, 380 + sz - 1);
    v.copy_screen(520, 300, sz, sz, 260, 380, 7);
}

/// Run the raster self-test (numeric op table + observable trap tests) and
/// draw the visual demo strip.
pub fn run() {
    let (sw, sh) = crate::vdi_backend::dims();
    if sw == 0 || sh == 0 {
        kprintln!("[raster] no framebuffer — skipping raster test");
        return;
    }
    kprintln!("[raster] === VDI raster test (vro_cpyfm 109 / vrt_cpyfm 121) ===");

    let mut fails = test_op_table();

    let mut v = Vdi::new();
    if !v.open() {
        kprintln!("[raster] could not open workstation — skipping trap tests");
        return;
    }
    fails += test_vro(&mut v);
    fails += test_vrt(&mut v);
    demo(&mut v);
    v.close();

    if fails == 0 {
        kprintln!("[raster] === raster PASS (all asserts) ===");
    } else {
        kprintln!("[raster] === raster FAILED ({} asserts) ===", fails);
    }
}
