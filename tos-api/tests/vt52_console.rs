//! CON04 — the VT52 console behind `Bconout(2)` and the raw console behind
//! `Bconout(5)`, against `docs/api/bios/_vt52.md` (EmuTOS `bios/vt52.c`,
//! `bios/conout.c`): one test per escape sequence and per control character,
//! the cursor (cell inversion, `disab_cnt`, blink, `Cursconf`), colours
//! (`ESC b`/`ESC c`, decision D1), Handle 5 (D5) and the `v_escape` coupling
//! (`v_enter_cur`/`v_exit_cur`, `vs_curaddress` Q6, `v_curtext` via `Crawio`).
//!
//! The screen is a 640×480 store of 16-bit screen words (80×30 cells of
//! 8×16), the form the console writes through `write_row_rgb565`. The
//! physical workstation is opened once (as the AES does), so colours resolve
//! through its pseudo-palette; `vt52_console_default_palette.rs` covers the
//! case without a workstation.

mod vt52_common;

use vt52_common::*;

fn setup() -> (std::sync::MutexGuard<'static, ()>, &'static Screen) {
    vt52_common::setup(true)
}

// ── initial state (`vt52_init`, spec §1) ─────────────────────────────────

#[test]
fn init_clears_homes_and_hides_the_cursor() {
    let (_g, s) = setup();
    assert_eq!(cursor(), (0, 0));
    for (c, r) in [(0, 0), (40, 15), (MX, MY)] {
        assert!(is_blank(s, c, r, WHITE), "cell {c},{r} not cleared to register 0");
    }
}

// ── printable characters, wrap (§7.1) ────────────────────────────────────

#[test]
fn printable_bytes_draw_in_fg_15_on_bg_0_and_advance() {
    let (_g, s) = setup();
    out(s, b"Az");
    assert!(is_char(s, 0, 0, b'A', BLACK, WHITE));
    assert!(is_char(s, 1, 0, b'z', BLACK, WHITE));
    assert_eq!(cursor(), (2, 0));
    // 0x7F and 0x80..0xFF are glyphs too.
    out(s, &[0x7f, 0x80, 0xff]);
    assert!(is_char(s, 2, 0, 0x7f, BLACK, WHITE));
    assert!(is_char(s, 4, 0, 0xff, BLACK, WHITE));
    assert_eq!(cursor(), (5, 0));
}

#[test]
fn wrap_is_off_after_init_and_the_last_cell_is_overwritten() {
    let (_g, s) = setup();
    out(s, &[0x1b, b'Y', 32 + 3, 32 + 77]);
    out(s, b"abcd");
    assert!(is_char(s, 77, 3, b'a', BLACK, WHITE));
    assert!(is_char(s, 78, 3, b'b', BLACK, WHITE));
    assert!(is_char(s, 79, 3, b'd', BLACK, WHITE), "c overwritten by d");
    assert!(is_blank(s, 0, 4, WHITE));
    assert_eq!(cursor(), (MX, 3));
}

#[test]
fn esc_v_wraps_after_drawing_and_esc_w_turns_it_off() {
    let (_g, s) = setup();
    out(s, b"\x1bv\x1bY\x23\x6eabcd");
    assert!(is_char(s, 78, 3, b'a', BLACK, WHITE));
    assert!(is_char(s, 79, 3, b'b', BLACK, WHITE));
    assert!(is_char(s, 0, 4, b'c', BLACK, WHITE));
    assert!(is_char(s, 1, 4, b'd', BLACK, WHITE));
    assert_eq!(cursor(), (2, 4));
    out(s, b"\x1bw\x1bY\x24\x6exyz");
    assert!(is_char(s, 79, 4, b'z', BLACK, WHITE));
    assert_eq!(cursor(), (MX, 4));
}

#[test]
fn wrap_in_the_last_row_scrolls() {
    let (_g, s) = setup();
    out(s, b"top");
    out(s, b"\x1bv\x1bY\x3d\x6eAB"); // row 29, column 78
    assert!(is_char(s, 79, MY - 1, b'B', BLACK, WHITE), "scrolled up one row");
    assert!(is_blank(s, 0, MY, WHITE));
    assert!(is_blank(s, 0, 0, WHITE), "the top row left the screen");
    assert_eq!(cursor(), (0, MY));
}

// ── control characters (§7.2) ─────────────────────────────────────────────

#[test]
fn cr_returns_to_column_0() {
    let (_g, s) = setup();
    out(s, b"abc\rX");
    assert!(is_char(s, 0, 0, b'X', BLACK, WHITE));
    assert!(is_char(s, 1, 0, b'b', BLACK, WHITE));
    assert_eq!(cursor(), (1, 0));
}

#[test]
fn lf_keeps_the_column() {
    let (_g, s) = setup();
    out(s, b"L1\nL2");
    assert!(row_text(s, 0, b"L1"));
    assert!(is_char(s, 2, 1, b'L', BLACK, WHITE));
    assert!(is_char(s, 3, 1, b'2', BLACK, WHITE));
    assert_eq!(cursor(), (4, 1));
}

#[test]
fn lf_on_the_last_row_scrolls_and_keeps_the_column() {
    let (_g, s) = setup();
    out(s, b"first\x1bY\x3d\x25x\nB"); // row 29, column 5
    assert!(is_char(s, 5, MY - 1, b'x', BLACK, WHITE));
    assert!(is_char(s, 6, MY, b'B', BLACK, WHITE));
    assert!(is_blank(s, 0, 0, WHITE), "'first' scrolled away");
}

#[test]
fn vt_and_ff_are_line_feeds() {
    let (_g, s) = setup();
    out(s, b"V\x0bW\x0cX");
    assert!(is_char(s, 0, 0, b'V', BLACK, WHITE));
    assert!(is_char(s, 1, 1, b'W', BLACK, WHITE));
    assert!(is_char(s, 2, 2, b'X', BLACK, WHITE), "FF does not clear");
}

#[test]
fn bs_moves_left_without_erasing_and_stops_at_column_0() {
    let (_g, s) = setup();
    out(s, b"ABCD\x08\x08");
    assert_eq!(cursor(), (2, 0));
    assert!(is_char(s, 2, 0, b'C', BLACK, WHITE), "BS does not erase");
    out(s, b"\r\x08");
    assert_eq!(cursor(), (0, 0));
}

#[test]
fn ht_goes_to_the_next_multiple_of_8_and_clamps() {
    let (_g, s) = setup();
    out(s, b"a\tb");
    assert!(is_char(s, 8, 0, b'b', BLACK, WHITE));
    out(s, b"\t");
    assert_eq!(cursor(), (16, 0));
    out(s, b"\x1bY\x20\x6b\t\t"); // column 75
    assert_eq!(cursor(), (MX, 0), "clamped, no wrap");
}

#[test]
fn bel_draws_nothing_and_does_not_move() {
    let (_g, s) = setup();
    out(s, b"a\x07b");
    assert!(row_text(s, 0, b"ab"));
    assert_eq!(cursor(), (2, 0));
}

#[test]
fn the_other_control_bytes_are_discarded() {
    let (_g, s) = setup();
    out(s, b"<");
    for c in (0u8..=6).chain(14..=26).chain(28..=31) {
        out(s, &[c]);
    }
    out(s, b">");
    assert!(row_text(s, 0, b"<>"));
    assert_eq!(cursor(), (2, 0));
}

// ── ESC + capital (§4.1) ─────────────────────────────────────────────────

#[test]
fn esc_a_b_c_d_move_one_cell_and_stop_at_the_edges() {
    let (_g, s) = setup();
    out(s, b"\x1bA\x1bD");
    assert_eq!(cursor(), (0, 0), "no scroll, no wrap at the top-left");
    out(s, b"\x1bB\x1bC");
    assert_eq!(cursor(), (1, 1));
    out(s, b"\x1bY\x3d\x6f\x1bB\x1bC");
    assert_eq!(cursor(), (MX, MY), "no scroll, no wrap at the bottom-right");
    out(s, b"\x1bA\x1bD");
    assert_eq!(cursor(), (MX - 1, MY - 1));
    let _ = s;
}

#[test]
fn esc_e_clears_with_the_background_and_homes_keeping_the_modes() {
    let (_g, s) = setup();
    out(s, b"\x1bc\x34\x1bp\x1bvhello\x1bE");
    assert_eq!(cursor(), (0, 0));
    assert!(is_blank(s, 0, 0, BLUE), "cleared in register 4");
    assert!(is_blank(s, MX, MY, BLUE));
    // Reverse and the colours survive: 'x' is register 4 on register 15.
    out(s, b"x");
    assert!(is_char(s, 0, 0, b'x', BLUE, BLACK));
}

#[test]
fn esc_h_homes() {
    let (_g, s) = setup();
    out(s, b"\x1bY\x2a\x2a\x1bH");
    assert_eq!(cursor(), (0, 0));
}

#[test]
fn esc_i_moves_up_and_scrolls_down_on_the_top_row() {
    let (_g, s) = setup();
    out(s, b"\x1bY\x25\x23\x1bI");
    assert_eq!(cursor(), (3, 4));
    out(s, b"\x1bHtop\x1bY\x3d\x20bottom\x1bY\x20\x27\x1bI");
    assert_eq!(cursor(), (7, 0), "column kept");
    assert!(is_blank(s, 0, 0, WHITE), "new blank top row");
    assert!(row_text(s, 1, b"top"));
    assert!(is_blank(s, 0, MY, WHITE), "'bottom' fell out");
}

#[test]
fn esc_j_erases_to_the_end_of_the_screen() {
    let (_g, s) = setup();
    for r in 0..=MY {
        fill_row(s, r, b'#');
    }
    out(s, b"\x1bY\x25\x48\x1bJ"); // row 5, column 40
    assert_eq!(cursor(), (40, 5));
    assert!(is_char(s, 39, 5, b'#', BLACK, WHITE));
    assert!(is_blank(s, 40, 5, WHITE));
    assert!(is_blank(s, MX, 5, WHITE));
    assert!(is_blank(s, 0, 6, WHITE));
    assert!(is_blank(s, MX, MY, WHITE));
    assert!(is_char(s, MX, 4, b'#', BLACK, WHITE));
}

#[test]
fn esc_k_erases_to_the_end_of_the_line() {
    let (_g, s) = setup();
    fill_row(s, 2, b'#');
    out(s, b"\x1bY\x22\x2b\x1bK"); // column 11
    assert_eq!(cursor(), (11, 2));
    assert!(is_char(s, 10, 2, b'#', BLACK, WHITE));
    assert!((11..=MX).all(|c| is_blank(s, c, 2, WHITE)));
}

/// Q3: under reverse video EmuTOS paints the border cell of `ESC K` at an
/// odd column (and at the last column) in the foreground — an artefact of
/// the bit-plane pairing (`conout.c:586-596`), seen on Hatari Falcon.
/// NewTOS erases the whole run with the background.
#[test]
fn esc_k_under_reverse_video_erases_uniformly_with_the_background() {
    let (_g, s) = setup();
    fill_row(s, 3, b'#');
    fill_row(s, 6, b'#');
    out(s, b"\x1bp\x1bY\x23\x2b\x1bK\x1bY\x26\x6f\x1bK\x1bq");
    assert!((11..=MX).all(|c| is_blank(s, c, 3, WHITE)), "odd column");
    assert!(is_blank(s, MX, 6, WHITE), "last column");
    assert!(is_char(s, MX - 1, 6, b'#', BLACK, WHITE));
}

#[test]
fn esc_l_inserts_a_line_and_goes_to_column_0() {
    let (_g, s) = setup();
    out(s, b"\x1bY\x2a\x20row10\x1bY\x2b\x20row11\x1bY\x3d\x20last");
    out(s, b"\x1bY\x2a\x25\x1bL");
    assert_eq!(cursor(), (0, 10));
    assert!(is_blank(s, 0, 10, WHITE));
    assert!(row_text(s, 11, b"row10"));
    assert!(row_text(s, 12, b"row11"));
    assert!(is_blank(s, 0, MY, WHITE), "'last' fell out");
}

#[test]
fn esc_m_deletes_a_line_and_goes_to_column_0() {
    let (_g, s) = setup();
    out(s, b"\x1bY\x2a\x20row10\x1bY\x2b\x20row11\x1bY\x3d\x20last");
    out(s, b"\x1bY\x2a\x25\x1bM");
    assert_eq!(cursor(), (0, 10));
    assert!(row_text(s, 10, b"row11"));
    assert!(row_text(s, MY - 1, b"last"));
    assert!(is_blank(s, 0, MY, WHITE), "bottom row blank");
}

#[test]
fn esc_y_addresses_row_then_column_and_clamps() {
    let (_g, s) = setup();
    out(s, b"\x1bY\x25\x2a");
    assert_eq!(cursor(), (10, 5));
    out(s, b"\x1bY\x7f\x7f");
    assert_eq!(cursor(), (MX, MY), "beyond the screen -> last row/column");
    out(s, b"\x1bY\x01\x35");
    assert_eq!(cursor(), (21, 0), "below 0x20 -> 0");
    // Any byte counts as a coordinate, also ESC and CR.
    out(s, &[0x1b, b'Y', 0x1b, b'\r']);
    assert_eq!(cursor(), (0, 0));
    out(s, b"Q");
    assert!(is_char(s, 0, 0, b'Q', BLACK, WHITE));
}

// ── ESC + small letter (§4.2) ────────────────────────────────────────────

#[test]
fn esc_b_and_esc_c_take_the_low_4_bits_as_registers() {
    let (_g, s) = setup();
    out(s, b"\x1bb\x31F\x1bc\x14B\x1bb\x3f\x1bc\x30N");
    assert!(is_char(s, 0, 0, b'F', RED, WHITE), "fg register 1 = pen 2");
    assert!(is_char(s, 1, 0, b'B', RED, BLUE), "0x14 & 0x0f = register 4");
    assert!(is_char(s, 2, 0, b'N', BLACK, WHITE));
}

#[test]
fn esc_b_follows_the_physical_workstation_palette() {
    let (_g, s) = setup();
    let mut pb = Pb::new();
    // vs_color(pen 2 = register 1, pure blue).
    pb.intin[..4].copy_from_slice(&[2, 0, 0, 1000]);
    pb.op(14, 0, 0, 4, 1);
    out(s, b"\x1bb\x31F\x1bb\x3f");
    pb.intin[..4].copy_from_slice(&[2, 1000, 0, 0]);
    pb.op(14, 0, 0, 4, 1);
    assert!(is_char(s, 0, 0, b'F', BLUE, WHITE), "D1: register -> pen -> palette");
}

#[test]
fn esc_d_erases_from_home_through_the_cursor() {
    let (_g, s) = setup();
    for r in 0..=4 {
        fill_row(s, r, b'#');
    }
    out(s, b"\x1bp\x1bY\x22\x34\x1bd\x1bq"); // row 2, column 20, reverse on
    assert_eq!(cursor(), (20, 2));
    assert!(is_blank(s, 0, 0, WHITE) && is_blank(s, MX, 1, WHITE));
    assert!((0..=20).all(|c| is_blank(s, c, 2, WHITE)), "Q3: uniform background");
    assert!(is_char(s, 21, 2, b'#', BLACK, WHITE));
    assert!(is_char(s, 0, 3, b'#', BLACK, WHITE));
}

#[test]
fn esc_e_shows_the_cursor_as_an_inverted_cell_and_esc_f_hides_it() {
    let (_g, s) = setup();
    out(s, b"\x1bY\x25\x25\x1be");
    assert!(is_cursor_on_blank(s, 5, 5, WHITE));
    out(s, b"\x1bf");
    assert!(is_blank(s, 5, 5, WHITE));
}

#[test]
fn esc_f_counts_and_one_esc_e_forces_the_cursor_back() {
    let (_g, s) = setup();
    out(s, b"\x1bf\x1bf\x1bf\x1be");
    assert!(is_cursor_on_blank(s, 0, 0, WHITE));
    // An internal bracket (ESC E: cursor_off .. cursor_on_cnt) keeps it.
    out(s, b"\x1bE");
    assert!(is_cursor_on_blank(s, 0, 0, WHITE));
}

#[test]
fn the_cursor_follows_output_and_movement() {
    let (_g, s) = setup();
    out(s, b"\x1beAB");
    assert!(is_char(s, 0, 0, b'A', BLACK, WHITE));
    assert!(is_char(s, 1, 0, b'B', BLACK, WHITE));
    assert!(is_cursor_on_blank(s, 2, 0, WHITE));
    out(s, b"\x1bD");
    assert!(is_blank(s, 2, 0, WHITE), "old cell restored");
    assert_eq!(cell(s, 1, 0), glyph_cell(b'B', !BLACK, !WHITE), "B inverted");
    out(s, b"\x1bf");
}

#[test]
fn esc_j_and_esc_k_save_and_restore_and_a_second_k_homes() {
    let (_g, s) = setup();
    out(s, b"\x1bY\x2a\x2b\x1bj\x1bY\x20\x20\x1bk");
    assert_eq!(cursor(), (11, 10));
    out(s, b"\x1bY\x22\x22\x1bk");
    assert_eq!(cursor(), (0, 0), "no saved position -> home");
}

#[test]
fn esc_l_lowercase_erases_the_whole_line_and_goes_to_column_0() {
    let (_g, s) = setup();
    fill_row(s, 7, b'#');
    out(s, b"\x1bY\x27\x40\x1bl");
    assert_eq!(cursor(), (0, 7));
    assert!((0..=MX).all(|c| is_blank(s, c, 7, WHITE)));
}

#[test]
fn esc_o_erases_from_the_line_start_through_the_cursor() {
    let (_g, s) = setup();
    fill_row(s, 4, b'#');
    fill_row(s, 5, b'#');
    // Q3: even column under reverse video (EmuTOS paints cell 10 in the
    // foreground there) — NewTOS erases with the background.
    out(s, b"\x1bp\x1bY\x24\x2a\x1bo\x1bY\x25\x2b\x1bo\x1bq");
    assert!((0..=10).all(|c| is_blank(s, c, 4, WHITE)));
    assert!(is_char(s, 11, 4, b'#', BLACK, WHITE));
    assert!((0..=11).all(|c| is_blank(s, c, 5, WHITE)));
    assert!(is_char(s, 12, 5, b'#', BLACK, WHITE));
    assert_eq!(cursor(), (11, 5));
}

#[test]
fn esc_p_and_esc_q_swap_the_colours_of_new_glyphs() {
    let (_g, s) = setup();
    out(s, b"a\x1bpb\x1bqc");
    assert!(is_char(s, 0, 0, b'a', BLACK, WHITE));
    assert!(is_char(s, 1, 0, b'b', WHITE, BLACK));
    assert!(is_char(s, 2, 0, b'c', BLACK, WHITE));
}

#[test]
fn unknown_sequences_are_swallowed_with_their_byte() {
    let (_g, s) = setup();
    out(s, b"[\x1ba\x1bZ\x1bx\x1bN\x1b0\x1b\r\x1b\x80]");
    assert!(row_text(s, 0, b"[]"));
    // ESC ESC: the second ESC is eaten, the next byte prints.
    out(s, b"\x1b\x1bQ");
    assert!(is_char(s, 2, 0, b'Q', BLACK, WHITE));
}

#[test]
fn nop_sequences_take_no_extra_byte() {
    let (_g, s) = setup();
    for c in b"FGghimnrstu" {
        out(s, &[0x1b, *c]);
    }
    out(s, b"!");
    assert!(is_char(s, 0, 0, b'!', BLACK, WHITE));
    assert_eq!(cursor(), (1, 0));
}

// ── Handle 5 (§9, D5) ────────────────────────────────────────────────────

#[test]
fn bconout_5_draws_every_byte_as_a_glyph() {
    let (_g, s) = setup();
    for b in [0x1bu8, b'E', 0x07, b'\r', b'\n', 0x01] {
        assert_eq!(tos_api::bios::chardev::bconout_handle(5, b), 1);
    }
    assert!(is_char(s, 0, 0, 0x1b, BLACK, WHITE));
    assert!(is_char(s, 1, 0, b'E', BLACK, WHITE));
    assert!(is_char(s, 3, 0, b'\r', BLACK, WHITE));
    assert!(is_char(s, 5, 0, 0x01, BLACK, WHITE));
    assert_eq!(cursor(), (6, 0));
}

#[test]
fn bconout_5_leaves_a_pending_handle_2_escape_alone() {
    let (_g, s) = setup();
    assert_eq!(tos_api::bios::chardev::bconout_handle(2, 0x1b), 1);
    tos_api::bios::chardev::bconout_handle(5, b'A');
    tos_api::bios::chardev::bconout_handle(2, b'p'); // completes ESC p
    tos_api::bios::chardev::bconout_handle(2, b'r');
    assert!(is_char(s, 0, 0, b'A', BLACK, WHITE));
    assert!(is_char(s, 1, 0, b'r', WHITE, BLACK));
}

// ── blink and Cursconf (§6, Q5) ──────────────────────────────────────────

#[test]
fn the_cursor_blinks_every_half_second_of_the_200_hz_counter() {
    let (_g, s) = setup();
    out(s, b"\x1be");
    assert!(is_cursor_on_blank(s, 0, 0, WHITE));
    tos_api::bios::tick::advance(99);
    tos_api::vdi::alpha_blink(s);
    assert!(is_cursor_on_blank(s, 0, 0, WHITE), "not yet 0.5 s");
    tos_api::bios::tick::advance(1);
    tos_api::vdi::alpha_blink(s);
    assert!(is_blank(s, 0, 0, WHITE), "off after 100 ticks");
    tos_api::bios::tick::advance(100);
    tos_api::vdi::alpha_blink(s);
    assert!(is_cursor_on_blank(s, 0, 0, WHITE), "on again");
    out(s, b"\x1bf");
}

#[test]
fn cursconf_2_stops_the_blink_and_3_restarts_it() {
    let (_g, s) = setup();
    out(s, b"\x1be");
    assert_eq!(tos_api::vdi::alpha_cursconf(Some(s), 2, 0), 0);
    tos_api::bios::tick::advance(500);
    tos_api::vdi::alpha_blink(s);
    assert!(is_cursor_on_blank(s, 0, 0, WHITE), "steady: M_CFLASH clear");
    tos_api::vdi::alpha_cursconf(Some(s), 3, 0);
    tos_api::bios::tick::advance(500);
    tos_api::vdi::alpha_blink(s);
    assert!(is_blank(s, 0, 0, WHITE), "flashing again");
    tos_api::vdi::alpha_cursconf(Some(s), 0, 0);
}

#[test]
fn cursconf_rate_round_trips_and_0_1_hide_and_show() {
    let (_g, s) = setup();
    assert_eq!(tos_api::vdi::alpha_cursconf(Some(s), 5, 0), 30);
    assert_eq!(tos_api::vdi::alpha_cursconf(Some(s), 4, 0x0114), 0);
    assert_eq!(tos_api::vdi::alpha_cursconf(Some(s), 5, 0), 0x14, "LOBYTE");
    assert_eq!(tos_api::vdi::alpha_cursconf(Some(s), 1, 0), 0);
    assert!(is_cursor_on_blank(s, 0, 0, WHITE));
    assert_eq!(tos_api::vdi::alpha_cursconf(Some(s), 0, 0), 0);
    assert!(is_blank(s, 0, 0, WHITE));
    assert_eq!(tos_api::vdi::alpha_cursconf(Some(s), 9, 0), 0);
}

// ── v_escape coupling (§8) ───────────────────────────────────────────────

#[test]
fn v_enter_cur_clears_and_shows_the_cursor_v_exit_cur_hides_it() {
    let (_g, s) = setup();
    out(s, b"junk");
    let mut pb = Pb::new();
    pb.escape(3, &[]); // ESC E ESC e CR
    assert!(is_cursor_on_blank(s, 0, 0, WHITE));
    assert!(is_blank(s, 1, 0, WHITE));
    out(s, b"\x1bY\x25\x25");
    pb.escape(2, &[]); // ESC H ESC f ESC E
    assert_eq!(cursor(), (0, 0));
    assert!(is_blank(s, 0, 0, WHITE) && is_blank(s, 5, 5, WHITE));
}

#[test]
fn vs_curaddress_sets_the_position_directly_with_clamping() {
    let (_g, s) = setup();
    let mut pb = Pb::new();
    pb.escape(11, &[6, 11]);
    assert_eq!(cursor(), (10, 5));
    // Q6: a column beyond 223 still clamps to the last column.
    pb.escape(11, &[1, 300]);
    assert_eq!(cursor(), (MX, 0));
    pb.escape(11, &[-5, -5]);
    assert_eq!(cursor(), (0, 0));
    pb.escape(15, &[]);
    assert_eq!((pb.intout[0], pb.intout[1]), (1, 1));
    let _ = s;
}

#[test]
fn v_curtext_goes_through_the_automaton() {
    let (_g, s) = setup();
    let mut pb = Pb::new();
    pb.escape(12, &[b'a' as i16, 0x1b, b'p' as i16, b'b' as i16, b'\n' as i16, b'c' as i16]);
    assert!(is_char(s, 0, 0, b'a', BLACK, WHITE));
    assert!(is_char(s, 1, 0, b'b', WHITE, BLACK), "ESC p interpreted");
    assert!(is_char(s, 2, 1, b'c', WHITE, BLACK), "LF keeps the column");
    out(s, b"\x1bq");
}

#[test]
fn v_eeol_v_rvon_and_cursor_moves_write_into_the_same_console() {
    let (_g, s) = setup();
    fill_row(s, 0, b'#');
    let mut pb = Pb::new();
    pb.escape(8, &[]); // home
    pb.escape(6, &[]); // right
    pb.escape(10, &[]); // erase to eol
    assert!(is_char(s, 0, 0, b'#', BLACK, WHITE));
    assert!(is_blank(s, 1, 0, WHITE));
    pb.escape(13, &[]); // reverse on
    out(s, b"r");
    pb.escape(14, &[]);
    assert!(is_char(s, 1, 0, b'r', WHITE, BLACK));
    pb.escape(1, &[]);
    assert_eq!((pb.intout[0], pb.intout[1]), (30, 80));
}
