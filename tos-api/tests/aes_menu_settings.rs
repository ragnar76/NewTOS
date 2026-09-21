//! `menu_settings` (39): the selector is a two-way switch, not a boolean.
//!
//! EmuTOS `mn_settings` (`references/emutos/aes/gemmnext.c:596-616`, pinned
//! tree `1a7484ab`) is a `switch(flag)` with exactly two arms and **no
//! `default`**:
//!
//! * `case 0` copies the AES's `settings` block into the caller's `MN_SET`;
//! * `case 1` copies every **non-negative** field of the caller's `MN_SET`
//!   into `settings`, running `height` through `clamp_height`
//!   (`:83-95`: at least `MIN_HEIGHT_VALUE` = 5 items, at most
//!   `gl_rfull.g_h / gl_hchar` — the desktop area **below the bar**,
//!   `gl_rfull = (0, gl_hbox, gl_width, gl_height - gl_hbox)`,
//!   `aes/gemgraf.c:424` — so 28 rows of 16 px in the 461 px under a 19 px
//!   bar on the 480-row reference screen, not 30 (A102));
//! * any other `flag` falls out of the switch — nothing is read, nothing is
//!   written. The caller's block and the AES's block both stay as they were.
//!
//! NewTOS treated the selector as "0 = get, anything else = set", so a
//! `flag` of 2 or -1 wrote the block exactly as `flag == 1` would have.
//! Finding A-2 of the 2026-09-12 sub-function audit
//! (`docs/api/aes/_full-function-inventory.md`, part II), ticket
//! `TASK/A43.md`.
//!
//! The initial values are part of the same contract: `mnext_init`
//! (`gemmnext.c:103-110`) seeds `settings` with `INIT_DISPLAY_VALUE` 200,
//! `INIT_DRAG_VALUE` 10000, `INIT_DELAY_VALUE` 250, `INIT_SPEED_VALUE` 0 and
//! `clamp_height(INIT_HEIGHT_VALUE)` = 16 (`:71-75`), and `menu_settings(0)`
//! is the only way an application can see them.
//!
//! `mn_settings` is `void`; `crysbind` leaves `ret = TRUE`, so `int_out[0]` is
//! 1 whatever the selector (`gemsuper.c:85`, `:198-200`).
//!
//! The AES keeps one process-wide `settings` block, and the reads below
//! depend on the order of the writes, so everything runs inside one test
//! function.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK, SCREEN_H};

const OP_MENU_SETTINGS: i16 = 39;

/// `MN_SET` as the AES reads it: four `LONG`s and a `WORD`
/// (`aes/gemmnext.h:27-33`), laid out as `tos_api::aes::menu::MnSet` lays it.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MnSet {
    display: i32,
    drag: i32,
    delay: i32,
    speed: i32,
    height: i16,
}

/// A block whose every field is negative: nothing in it is a value the
/// `set` arm would accept, so it can only come back out of a `get`.
const POISON: MnSet = MnSet {
    display: -0x5A5A_5A5A,
    drag: -0x5A5A_5A5A,
    delay: -0x5A5A_5A5A,
    speed: -0x5A5A_5A5A,
    height: -0x5A5A,
};

/// `mnext_init` (`gemmnext.c:103-110`) at the reference geometry: the
/// `INIT_*` constants (`:71-75`), `height` through `clamp_height` — 16 is
/// inside `[5, gl_rfull.g_h / gl_hchar]` at 480 rows of a 16-pixel font, so
/// the clamp leaves it alone.
const EMUTOS_DEFAULTS: MnSet = MnSet {
    display: 200,
    drag: 10000,
    delay: 250,
    speed: 0,
    height: 16,
};

/// `MIN_HEIGHT_VALUE` (`gemmnext.c:76`).
const MIN_HEIGHT_VALUE: i16 = 5;

/// `gl_hbox` = `gl_hchar + 3` (`aes/gemgraf.c:412`) for the 16 px system font.
const GL_HBOX: i16 = 19;

/// `gl_rfull.g_h / gl_hchar` (`clamp_height`, `gemmnext.c:90`): the rows of
/// the system font that fit below the bar — `(480 − 19) / 16` = 28.
const MAX_HEIGHT: i16 = (SCREEN_H as i16 - GL_HBOX) / 16;

/// One `menu_settings(flag, &block)` through the LP64 dispatcher. Returns
/// `int_out[0]`; the block is passed by address, so a `get` fills it and a
/// `set` (or a no-op) leaves it to the caller to inspect.
fn menu_settings(pb: &mut Pb, flag: i16, block: &mut MnSet) -> i16 {
    pb.poison();
    pb.intin[0] = flag;
    pb.addrin[0] = block as *mut MnSet as u64;
    pb.call(OP_MENU_SETTINGS, 1, 1, 1, 0);
    pb.intout[0]
}

/// `menu_settings(0)`: what the AES currently holds.
fn current(pb: &mut Pb) -> MnSet {
    let mut block = POISON;
    assert_eq!(
        menu_settings(pb, 0, &mut block),
        1,
        "menu_settings(0) return"
    );
    assert_ne!(block, POISON, "menu_settings(0) wrote nothing");
    block
}

#[test]
fn only_flag_zero_reads_and_only_flag_one_writes() {
    let _guard = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ctx = context();
    assert!(
        SCREEN_H as i16 / 16 >= EMUTOS_DEFAULTS.height,
        "the reference geometry must not clamp the default height"
    );
    let mut pb = Pb::new();

    // ── the power-up block: mnext_init ──────────────────────────────────
    assert_eq!(
        current(&mut pb),
        EMUTOS_DEFAULTS,
        "menu_settings(0) before any set"
    );

    // ── flag 1 writes; flag 0 reads it back ─────────────────────────────
    let written = MnSet {
        display: 111,
        drag: 222,
        delay: 333,
        speed: 444,
        height: 10,
    };
    let mut block = written;
    assert_eq!(
        menu_settings(&mut pb, 1, &mut block),
        1,
        "menu_settings(1) return"
    );
    assert_eq!(
        block, written,
        "menu_settings(1) must not write the caller's block"
    );
    assert_eq!(
        current(&mut pb),
        written,
        "menu_settings(0) after menu_settings(1)"
    );

    // ── every other selector is a no-op: neither side is touched ────────
    for flag in [2i16, -1, 3, i16::MAX, i16::MIN] {
        let foreign = MnSet {
            display: 1000 + flag as i32,
            drag: 2000,
            delay: 3000,
            speed: 4000,
            height: 6,
        };
        let mut block = foreign;
        assert_eq!(
            menu_settings(&mut pb, flag, &mut block),
            1,
            "menu_settings({flag}) return"
        );
        assert_eq!(
            block, foreign,
            "menu_settings({flag}) wrote the caller's block"
        );
        assert_eq!(
            current(&mut pb),
            written,
            "menu_settings({flag}) changed the settings (it has no switch arm)"
        );
    }

    // ── flag 1 ignores negative fields, field by field ──────────────────
    let mut block = POISON;
    assert_eq!(menu_settings(&mut pb, 1, &mut block), 1);
    assert_eq!(
        current(&mut pb),
        written,
        "all-negative set changed the settings"
    );

    let mut block = MnSet {
        display: -5,
        drag: 555,
        delay: -7,
        speed: 666,
        height: -9,
    };
    assert_eq!(menu_settings(&mut pb, 1, &mut block), 1);
    let expected = MnSet {
        display: 111,
        drag: 555,
        delay: 333,
        speed: 666,
        height: 10,
    };
    assert_eq!(current(&mut pb), expected, "mixed-sign set");

    // ── flag 1 clamps height from below (0 is non-negative, so it is set) ─
    for low in [0i16, 1, MIN_HEIGHT_VALUE - 1] {
        let mut block = MnSet {
            display: -1,
            drag: -1,
            delay: -1,
            speed: -1,
            height: low,
        };
        assert_eq!(menu_settings(&mut pb, 1, &mut block), 1);
        assert_eq!(
            current(&mut pb),
            MnSet {
                height: MIN_HEIGHT_VALUE,
                ..expected
            },
            "height {low} must clamp to MIN_HEIGHT_VALUE"
        );
    }
    let mut block = MnSet {
        display: -1,
        drag: -1,
        delay: -1,
        speed: -1,
        height: MIN_HEIGHT_VALUE,
    };
    assert_eq!(menu_settings(&mut pb, 1, &mut block), 1);
    assert_eq!(current(&mut pb).height, MIN_HEIGHT_VALUE);

    // ── flag 1 clamps height from above: a pop-up taller than the desktop
    // area. `max_height = gl_rfull.g_h / gl_hchar` (`gemmnext.c:90`), and
    // `gl_rfull` is the screen below the bar (`gemgraf.c:424`): 30 rows would
    // be 480 px, the desktop area is 461 → 28 (A102). The screen height alone
    // (`480 / 16` = 30) would let the box reach into the bar row.
    assert_eq!(MAX_HEIGHT, 28, "the reference geometry: (480 - 19) / 16");
    for high in [MAX_HEIGHT + 1, SCREEN_H as i16 / 16, 100, i16::MAX] {
        let mut block = MnSet {
            display: -1,
            drag: -1,
            delay: -1,
            speed: -1,
            height: high,
        };
        assert_eq!(menu_settings(&mut pb, 1, &mut block), 1);
        assert_eq!(
            current(&mut pb).height,
            MAX_HEIGHT,
            "height {high} must clamp to gl_rfull.g_h / gl_hchar = {MAX_HEIGHT} (gemmnext.c:90, gemgraf.c:424)"
        );
    }
    let mut block = MnSet {
        display: -1,
        drag: -1,
        delay: -1,
        speed: -1,
        height: MAX_HEIGHT,
    };
    assert_eq!(menu_settings(&mut pb, 1, &mut block), 1);
    assert_eq!(current(&mut pb).height, MAX_HEIGHT, "the maximum itself is not clamped");
    let mut block = MnSet {
        display: -1,
        drag: -1,
        delay: -1,
        speed: -1,
        height: MIN_HEIGHT_VALUE,
    };
    assert_eq!(menu_settings(&mut pb, 1, &mut block), 1);
    assert_eq!(current(&mut pb).height, MIN_HEIGHT_VALUE);

    // ── and an invalid selector still does nothing after all of that ────
    let mut block = MnSet {
        display: 1,
        drag: 2,
        delay: 3,
        speed: 4,
        height: 30,
    };
    assert_eq!(menu_settings(&mut pb, 2, &mut block), 1);
    assert_eq!(
        block,
        MnSet {
            display: 1,
            drag: 2,
            delay: 3,
            speed: 4,
            height: 30
        }
    );
    assert_eq!(
        current(&mut pb),
        MnSet {
            height: MIN_HEIGHT_VALUE,
            ..expected
        }
    );
}
