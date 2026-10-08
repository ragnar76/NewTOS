//! Software key auto-repeat — the state machine of EmuTOS' BIOS key repeat
//! (`references/emutos/bios/ikbd.c:395-536`, start/stop in `kbd_int`
//! `:777-909`), reimplemented for a **polled** input path.
//!
//! Spec: `docs/api/xbios/_state.md` §"Key-repeat handler" and
//! `docs/api/xbios/23-kbrate.md` §"Pi / BCM2711 binding".
//!
//! # EmuTOS behaviour reproduced
//!
//! * A make of a non-modifier key records it as the repeating key
//!   (`kb_last`, `kb_last_actual`) and arms the countdown with the **current**
//!   initial delay (`kb_ticks = kb_initial`, `:875,891,898-903`). Only the last
//!   pressed key repeats.
//! * A break of that key — or of Home / Insert, unconditionally — stops the
//!   repeat (`:807-817`). Modifier makes/breaks do not (`:778-801,825-870`).
//! * Every 50 Hz tick (`do_key_repeat`, `:451-528`): with `conterm & 2` clear
//!   the countdown is **frozen** (return before the decrement, `:475-479`);
//!   otherwise it counts down, and on reaching zero the last key word is
//!   emitted again — re-converted first if the shift state changed
//!   (`:502-508`) — and the countdown is re-armed with the **current** repeat
//!   interval (`kb_ticks = kb_repeat`, `:527`).
//! * Before all of that, a tick with a key repeating stops the repeat if the
//!   `ikbdsys` vector differs from the one recorded at the make
//!   (`kb_last_ikbdsys`, `:453-472`, recorded `:902`, refreshed on
//!   re-conversion `:507`) — even with `conterm & 2` clear. On NewTOS the
//!   value is the running process's `ikbdsys` slot (`xbios::kbdvecs`, KEYS02
//!   D4); it is only compared, never called.
//!
//! # Where it runs — the sanctioned deviation
//!
//! EmuTOS calls `do_key_repeat` from the timer-C interrupt. On the Pi the
//! break of a held key reaches the decoder only when the USB-HID keyboard is
//! *polled*, so NewTOS runs the repeat in the input pump instead: the caller
//! first decodes the pending input (a break stops the repeat), then calls
//! [`KeyRepeat::poll`] with the current `_hz_200`, which credits the 50 Hz
//! steps — the multiples of 4 that `_hz_200` crossed, the same phase as
//! EmuTOS' `timer_c_sieve` — since the previous poll. (INPUT01's
//! timer-interrupt pump decodes pointer motion only and never runs the
//! repeat — key bytes stay held for the trap-side pump, `crate::input_gate`.)
//!
//! **Burst guard:** one poll emits **at most one** repeat; the remaining
//! elapsed steps are discarded and the next repeat is due `repeat` steps after
//! that poll. A late poll cannot tell when within the gap the key was
//! released, so crediting the gap in full would queue repeats for time the key
//! may not have been held — ghost keys. See the spec for the full rationale.
//!
//! Pure logic with no kernel dependencies, so the host test suite
//! (`tools/xtask`, which includes this file by path) runs its tests.

/// IKBD scancode of Home (`bios/ikbd.c:88`). Its break stops any repeat.
pub const SC_HOME: u8 = 0x47;
/// IKBD scancode of Insert (`bios/ikbd.c:87`). Its break stops any repeat.
pub const SC_INSERT: u8 = 0x52;

/// `conterm` bit 1: key repeat enabled (`bios/ikbd.c:475`).
pub const CONTERM_REPEAT: u8 = 0x02;

/// The repeat state — EmuTOS `kb_ticks` / `kb_last` / `kb_last_actual`, plus
/// the `_hz_200` value of the previous poll (the polled stand-in for the
/// timer-C sieve).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRepeat {
    /// 50 Hz steps until the next repeat; `0` = no key is repeating.
    ticks: u16,
    /// The key word last emitted, `scancode << 8 | ascii` (`kb_last`).
    word: u16,
    /// Shift state the word was converted with (`kb_last.k.shifty`).
    shifty: u8,
    /// Raw make scancode of the repeating key (`kb_last_actual`).
    actual: u8,
    /// `_hz_200` at the previous [`poll`](Self::poll) or make.
    last_hz: u32,
    /// The `ikbdsys` vector in force at the make (`kb_last_ikbdsys`).
    ikbdsys: u64,
}

impl KeyRepeat {
    /// Power-up state: nothing repeating (`kbd_init`, `bios/ikbd.c:1091`).
    pub const fn new() -> Self {
        Self {
            ticks: 0,
            word: 0,
            shifty: 0,
            actual: 0,
            last_hz: 0,
            ikbdsys: 0,
        }
    }

    /// `true` while a key is repeating (or waiting for its first repeat).
    pub fn active(&self) -> bool {
        self.ticks != 0
    }

    /// A non-modifier key went down and `word` was queued for it.
    ///
    /// `code` is the raw make scancode, `shifty` the shift state the word was
    /// converted with, `initial` the current `Kbrate` initial delay, `now_hz`
    /// the current `_hz_200`, `ikbdsys` the `ikbdsys` vector in force. EmuTOS
    /// `kbd_int` `:875,891,898-903`.
    pub fn make(
        &mut self,
        code: u8,
        word: u16,
        shifty: u8,
        initial: u8,
        now_hz: u32,
        ikbdsys: u64,
    ) {
        self.actual = code;
        self.ikbdsys = ikbdsys;
        self.ticks = u16::from(initial);
        self.word = word;
        self.shifty = shifty;
        // Steps before the make belong to the previous key, which this make
        // replaces; count the new delay from now.
        self.last_hz = now_hz;
    }

    /// A key went up (`code` without the release bit). Stops the repeat for
    /// the repeating key and, unconditionally, for Home / Insert
    /// (`bios/ikbd.c:807-817`). Modifier breaks never match the repeating key
    /// (a modifier make does not record one), so they leave it running.
    pub fn brk(&mut self, code: u8) {
        if code == SC_HOME || code == SC_INSERT || code == self.actual {
            self.ticks = 0;
        }
    }

    /// Credit the 50 Hz steps elapsed since the previous poll and return the
    /// key word to queue, if a repeat is due — at most one per call.
    ///
    /// `conterm` is the running process's `$484`, `shifty` the current shift
    /// state, `repeat` the current `Kbrate` repeat interval, `ikbdsys` the
    /// `ikbdsys` vector in force now. `convert` maps the raw scancode to a key
    /// word under the current shift state; it is called only when the shift
    /// state changed since the word was made (`bios/ikbd.c:502-508`).
    pub fn poll(
        &mut self,
        now_hz: u32,
        conterm: u8,
        shifty: u8,
        repeat: u8,
        ikbdsys: u64,
        convert: impl FnOnce(u8) -> u16,
    ) -> Option<u16> {
        let steps = sieve_steps(self.last_hz, now_hz);
        self.last_hz = now_hz;
        if steps != 0 && self.ticks > 0 && ikbdsys != self.ikbdsys {
            // The `ikbdsys` vector changed while a key repeats: stop now
            // (`:453-472`), ahead of the `conterm` test.
            self.ticks = 0;
            return None;
        }
        if steps == 0 || conterm & CONTERM_REPEAT == 0 || self.ticks == 0 {
            // No tick, repeat globally off (countdown frozen, `:475-479`), or
            // nothing repeating (`:481-485`).
            return None;
        }
        if steps < u64::from(self.ticks) {
            // `steps` is below a u16 here.
            self.ticks -= steps as u16;
            return None;
        }
        // The countdown reached zero within this poll: emit once, re-arm,
        // and drop the rest of the elapsed steps (burst guard).
        if shifty != self.shifty {
            self.word = convert(self.actual);
            self.shifty = shifty;
            self.ikbdsys = ikbdsys; // `kb_last_ikbdsys = kbdvecs.ikbdsys` (`:507`)
        }
        self.ticks = u16::from(repeat);
        Some(self.word)
    }
}

impl Default for KeyRepeat {
    fn default() -> Self {
        Self::new()
    }
}

/// Number of 50 Hz sieve ticks — multiples of 4 of the 200 Hz counter —
/// in the half-open interval `(last, now]`, wrap-safe (`2^32` is a multiple
/// of 4, so the residues survive the wrap). EmuTOS runs the 50 Hz work on
/// every 4th timer-C interrupt (`bios/vectors.S:521`, sieve `0x1111`,
/// `bios/bios.c:463`).
fn sieve_steps(last: u32, now: u32) -> u64 {
    (u64::from(last % 4) + u64::from(now.wrapping_sub(last))) / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    const UP: u8 = 0x48; // up arrow
    const A: u8 = 0x1E;
    const W_UP: u16 = 0x4800;
    const W_A: u16 = 0x1E61;
    const W_SHIFT_A: u16 = 0x1E41;
    /// The `ikbdsys` vector in force throughout, unless a test changes it.
    const IK: u64 = 0xA00;

    fn no_convert(_: u8) -> u16 {
        panic!("convert must not be called without a shift change")
    }

    /// Poll once per 200 Hz tick from `from` to `to` inclusive and collect
    /// the hz values at which a repeat came out.
    fn run(r: &mut KeyRepeat, from: u32, to: u32, conterm: u8, repeat: u8) -> Vec<u32> {
        let mut out = Vec::new();
        for hz in from..=to {
            if r.poll(hz, conterm, 0, repeat, IK, no_convert).is_some() {
                out.push(hz);
            }
        }
        out
    }

    #[test]
    fn delay_then_rate_with_defaults() {
        // Make at hz 1000 (a sieve boundary), defaults 15 / 2.
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 1000, IK);
        let hits = run(&mut r, 1001, 1200, 7, 2);
        // First repeat 15 sieve ticks later = 60 hz ticks = 300 ms, then
        // every 2 sieve ticks = 8 hz ticks = 40 ms.
        assert_eq!(hits[0], 1060);
        assert_eq!(hits[1], 1068);
        assert!(hits.windows(2).all(|w| w[1] - w[0] == 8));
        assert_eq!(hits.len(), 1 + (1200 - 1060) / 8);
    }

    #[test]
    fn first_repeat_follows_the_sieve_phase() {
        // Make 3 hz ticks before a boundary: the first of the 15 steps comes
        // 3 hz ticks later, so the first repeat is 59 hz ticks (295 ms) after
        // the make, on the same sieve tick as a make at 1000 — EmuTOS'
        // sub-tick phase.
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 1001, IK);
        let hits = run(&mut r, 1002, 1100, 7, 2);
        assert_eq!(hits[0], 1060);
    }

    #[test]
    fn repeated_word_is_the_make_word() {
        let mut r = KeyRepeat::new();
        r.make(A, W_A, 0, 1, 0, IK);
        assert_eq!(r.poll(4, 7, 0, 2, IK, no_convert), Some(W_A));
    }

    #[test]
    fn break_of_the_key_stops_the_repeat() {
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        assert_eq!(run(&mut r, 1, 100, 7, 2).len(), 6); // 60, 68, …, 100
        r.brk(UP);
        assert!(!r.active());
        assert!(run(&mut r, 101, 2000, 7, 2).is_empty());
    }

    #[test]
    fn break_seen_before_the_step_gives_no_ghost() {
        // The pump decodes first, then polls: a release that arrives in the
        // same pump as the due step must not emit.
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        r.brk(UP);
        assert_eq!(r.poll(60, 7, 0, 2, IK, no_convert), None);
    }

    #[test]
    fn break_of_another_key_does_not_stop() {
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        r.brk(A);
        r.brk(0x2A); // left shift
        assert_eq!(run(&mut r, 1, 60, 7, 2), vec![60]);
    }

    #[test]
    fn home_and_insert_breaks_always_stop() {
        for code in [SC_HOME, SC_INSERT] {
            let mut r = KeyRepeat::new();
            r.make(UP, W_UP, 0, 15, 0, IK);
            r.brk(code);
            assert!(!r.active(), "break {code:#x} must stop the repeat");
        }
    }

    #[test]
    fn last_pressed_key_wins() {
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        // Second key at hz 100 (first one still held).
        assert_eq!(run(&mut r, 1, 99, 7, 2).len(), 5);
        r.make(A, W_A, 0, 15, 100, IK);
        let mut words = Vec::new();
        for hz in 101..=200 {
            if let Some(w) = r.poll(hz, 7, 0, 2, IK, no_convert) {
                words.push(w);
            }
        }
        assert!(!words.is_empty());
        assert!(words.iter().all(|&w| w == W_A));
        // Releasing the first key (no longer the repeating one) changes
        // nothing; releasing the second stops it.
        r.brk(UP);
        assert!(r.active());
        r.brk(A);
        assert!(!r.active());
    }

    #[test]
    fn conterm_bit1_clear_freezes_the_countdown() {
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        // 10 of 15 steps with repeat on.
        assert!(run(&mut r, 1, 40, 7, 2).is_empty());
        // Repeat off for a long time: nothing, and the countdown does not move.
        assert!(run(&mut r, 41, 4000, 7 & !CONTERM_REPEAT, 2).is_empty());
        assert!(r.active());
        // Back on: the remaining 5 steps.
        assert_eq!(run(&mut r, 4001, 4020, 7, 2), vec![4020]);
    }

    #[test]
    fn kbrate_values_are_read_at_make_and_at_repeat() {
        // initial 5 -> first repeat after 5 steps; repeat 3 -> then every 3.
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 5, 0, IK);
        assert_eq!(run(&mut r, 1, 40, 7, 3), vec![20, 32]);
        // A changed repeat interval applies from the next re-arm on.
        assert_eq!(run(&mut r, 41, 60, 7, 1), vec![44, 48, 52, 56, 60]);
    }

    #[test]
    fn zero_initial_means_no_repeat_and_zero_rate_means_one() {
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 0, 0, IK);
        assert!(run(&mut r, 1, 400, 7, 2).is_empty());

        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        assert_eq!(run(&mut r, 1, 400, 7, 0), vec![60]);
        assert!(!r.active());
    }

    #[test]
    fn long_gap_emits_at_most_one() {
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        // One poll 5 s later: 250 steps elapsed — one repeat, not dozens.
        assert_eq!(r.poll(1000, 7, 0, 2, IK, no_convert), Some(W_UP));
        // The cadence restarts at that poll: next due 2 steps (8 hz) later.
        assert_eq!(r.poll(1004, 7, 0, 2, IK, no_convert), None);
        assert_eq!(r.poll(1008, 7, 0, 2, IK, no_convert), Some(W_UP));
    }

    #[test]
    fn shift_change_reconverts_the_word() {
        let mut r = KeyRepeat::new();
        r.make(A, W_A, 0, 1, 0, IK);
        let mut seen = None;
        let w = r.poll(4, 7, 0x02, 2, IK, |code| {
            seen = Some(code);
            W_SHIFT_A
        });
        assert_eq!(seen, Some(A), "re-conversion starts from the raw scancode");
        assert_eq!(w, Some(W_SHIFT_A));
        // Same shift state again: no further conversion, same word.
        assert_eq!(r.poll(12, 7, 0x02, 2, IK, no_convert), Some(W_SHIFT_A));
    }

    #[test]
    fn idle_polls_and_counter_wrap() {
        // Nothing repeating: polls never emit, however long.
        let mut r = KeyRepeat::new();
        assert_eq!(r.poll(u32::MAX - 2, 7, 0, 2, IK, no_convert), None);
        // A key held across the 32-bit wrap of _hz_200.
        r.make(UP, W_UP, 0, 15, u32::MAX - 3, IK); // a sieve boundary
        let mut hits = Vec::new();
        let mut hz = u32::MAX - 3;
        for _ in 0..80 {
            hz = hz.wrapping_add(1);
            if r.poll(hz, 7, 0, 2, IK, no_convert).is_some() {
                hits.push(hz);
            }
        }
        // 60 hz ticks after the make, across the wrap.
        assert_eq!(hits.first().copied(), Some((u32::MAX - 3).wrapping_add(60)));
    }

    #[test]
    fn ikbdsys_change_stops_the_repeat() {
        // EmuTOS `do_key_repeat` `:453-472`: a different `ikbdsys` than at the
        // make stops a running repeat at the next tick — before the `conterm`
        // test, so even with repeat disabled — and it stays stopped.
        const OTHER: u64 = 0x1_2340;
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, IK);
        assert!(run(&mut r, 1, 40, 7, 2).is_empty());
        // Same vector, no tick yet: nothing happens.
        assert_eq!(r.poll(41, 7, 0, 2, OTHER, no_convert), None);
        assert!(r.active(), "the check runs on a tick, not on every poll");
        // Next tick with the changed vector: stopped, nothing emitted.
        assert_eq!(r.poll(44, 7 & !CONTERM_REPEAT, 0, 2, OTHER, no_convert), None);
        assert!(!r.active());
        // Putting the old vector back does not restart it.
        let mut hits = Vec::new();
        for hz in 45..=400 {
            if r.poll(hz, 7, 0, 2, IK, no_convert).is_some() {
                hits.push(hz);
            }
        }
        assert!(hits.is_empty());
    }

    #[test]
    fn ikbdsys_is_recorded_per_make() {
        // A make under the new vector repeats normally under it
        // (`kb_last_ikbdsys` is taken at each make, `:902`).
        const OTHER: u64 = 0x1_2340;
        let mut r = KeyRepeat::new();
        r.make(UP, W_UP, 0, 15, 0, OTHER);
        let mut hits = Vec::new();
        for hz in 1..=60 {
            if r.poll(hz, 7, 0, 2, OTHER, no_convert).is_some() {
                hits.push(hz);
            }
        }
        assert_eq!(hits, vec![60]);
    }

    #[test]
    fn ikbdsys_unchanged_without_a_repeat_is_harmless() {
        // `kb_ticks > 0` guards the check (`:453`): no key repeating, no effect.
        let mut r = KeyRepeat::new();
        assert_eq!(r.poll(4, 7, 0, 2, 0xDEAD, no_convert), None);
        r.make(A, W_A, 0, 1, 4, 0xDEAD);
        assert_eq!(r.poll(8, 7, 0, 2, 0xDEAD, no_convert), Some(W_A));
    }

    #[test]
    fn sieve_steps_counts_boundaries() {
        assert_eq!(sieve_steps(0, 0), 0);
        assert_eq!(sieve_steps(0, 3), 0);
        assert_eq!(sieve_steps(0, 4), 1);
        assert_eq!(sieve_steps(3, 4), 1);
        assert_eq!(sieve_steps(4, 7), 0);
        assert_eq!(sieve_steps(5, 13), 2);
        assert_eq!(sieve_steps(u32::MAX, 0), 1);
    }
}
