# `Kbrate` (XBIOS 0x23)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/ikbd.c:439` (`kbrate`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the `< 0` query per parameter, the packed return
> (old initial in the high byte, old repeat in the low byte), the `& 0xFF`
> store. Reference: the `kb_ticks` repeat timer.

## EmuTOS source

**Primary location:** `bios/ikbd.c:439` — `WORD kbrate(WORD initial, WORD
repeat)`, slot 0x23 of `xbios_vecs[]` (`bios/xbios.c:1338`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:** none.

**Headers / type definitions used:**
- `include/portab.h` — `MAKE_UWORD(hi, lo) = ((hi) << 8) | (lo)`.
- `include/sysconf.h:68-69` — `KB_INITIAL = 15`, `KB_REPEAT = 2`.

**System variables / globals read:**
- `kb_initial`, `kb_repeat` (`bios/ikbd.c:425-426`).

**System variables / globals written:**
- `kb_initial` (if `initial >= 0`), `kb_repeat` (if `repeat >= 0`)
  (`bios/ikbd.c:443-446`).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Kbrate` gets and/or sets the keyboard auto-repeat timing. It first computes
the return value from the *current* settings — `MAKE_UWORD(kb_initial,
kb_repeat)`, i.e. the old initial delay in the **high** byte and the old
repeat interval in the **low** byte. Then, for each parameter that is `>= 0`,
it stores it masked to `& 0x00FF` into the corresponding global; a parameter
`< 0` (e.g. `-1`) is a **query** and leaves that setting unchanged. So the
common idioms are `Kbrate(-1, -1)` (read only) and `Kbrate(newInit,
newRepeat)` (set both, get old).

Both values are in units of ~20 ms (the key-repeat timer tick). Power-up
defaults are `kb_initial = 15` (≈300 ms) and `kb_repeat = 2` (≈40 ms). The
repeat itself is driven by a separate `kb_ticks` countdown in the key-repeat
handler.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x23
- **Stack frame on entry (m68k):** `Kbrate(initial, repeat)` =
  `xbios_w_ww(35, a, b)` (`include/xbiosbind.h:53`):
  ```
  +4: repeat  (word)
  +2: initial (word)
   0: function_id (word) = 0x0023
  ```
- **Total parameter bytes:** 6 (W + W).
- **Native (AArch64) ABI:** ID `0x23` in `x8`; args in `x0..x1`; the WORD
  result **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name      | Type / size |
  |------------|-------------|-----------|-------------|
  | +2         | `x0`        | `initial` | `WORD` (16) — `< 0` = query |
  | +4         | `x1`        | `repeat`  | `WORD` (16) — `< 0` = query |

---

## Inputs

| Position | Name      | Type   | Size | Source location    | Notes |
|----------|-----------|--------|------|--------------------|-------|
| arg0     | `initial` | `WORD` | 2    | `bios/ikbd.c:443`  | new initial delay (20 ms units); `< 0` = keep; stored `& 0xFF` |
| arg1     | `repeat`  | `WORD` | 2    | `bios/ikbd.c:445`  | new repeat interval (20 ms units); `< 0` = keep; stored `& 0xFF` |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** WORD
- **Encoding:** `MAKE_UWORD(old_kb_initial, old_kb_repeat)` — old initial in
  bits 8-15, old repeat in bits 0-7.
- **Source location:** `bios/ikbd.c:441,448`.

### Side effects on global state

- `kb_initial = initial & 0xFF` when `initial >= 0`; `kb_repeat = repeat &
  0xFF` when `repeat >= 0` (`bios/ikbd.c:443-446`).

### Side effects on hardware

None (software repeat timer; the IKBD hardware repeat is not used).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

---

## Verification criteria

- **Query:** `Kbrate(-1, -1)` returns the current packed `(initial<<8)|repeat`
  and changes nothing. At power-up this is `(15<<8)|2 = 0x0F02`.
- **Set both, get old:** `Kbrate(20, 3)` returns the previous packed value and
  makes the next `Kbrate(-1,-1)` return `(20<<8)|3 = 0x1403`.
- **Set one:** `Kbrate(30, -1)` updates only `kb_initial`, leaves `kb_repeat`.
- **Masking:** `Kbrate(0x1FF, -1)` stores `kb_initial = 0xFF` (masked).

---

## Edge cases discovered in source

1. **Old value computed before the update.** The return uses the *previous*
   `kb_initial`/`kb_repeat`, even when both are being set (`bios/ikbd.c:441`).
2. **Per-parameter query via `< 0`.** Only the parameters that are `>= 0` are
   written, so you can set one and query the other in a single call.
3. **Stored masked to a byte.** Values above 255 are truncated with `& 0xFF`
   (`:444,446`); the packed return is a byte per field.
4. **Defaults are 15/2, not 25/5.** Power-up `KB_INITIAL = 15`, `KB_REPEAT =
   2` (`include/sysconf.h:68-69`). The old `_state.md` said 25/5 — corrected
   (D-042).

---

## Dependencies

### Requires (must be initialized / functional)

- `kb_initial`/`kb_repeat` initialised by `kbd_init` (`bios/ikbd.c:1092-1093`).
- The key-repeat timer (`kb_ticks`) for the effect to be observable.

### Used by

- Control panel / setup CPX, accessibility tools, games disabling repeat.

---

## Related data structures

- `docs/api/xbios/_state.md §2` — `kb_initial`/`kb_repeat` and the defaults.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the packed
  old-value return and the `-1` query; classic-TOS defaults are often quoted
  differently — EmuTOS uses 15/2.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Defaults `KB_INITIAL = 15` / `KB_REPEAT = 2` in this build; classic TOS
  values differ but the ABI (packed return, `-1` query, `& 0xFF`) is the same.

---

## NewTOS implementation notes

(Empty during Phase 00.)

`Kbrate` reads/writes the two byte fields with the same packed return
(`tos-api/src/xbios/system.rs`); the key-repeat handler
(`kernel/src/key_repeat.rs`, KEYS01 2026-10-03) consumes them — see §"Pi /
BCM2711 binding" for where it runs.

## Pi / BCM2711 binding

> Re-points the *hardware reference* (the `kb_ticks` software repeat timer,
> driven in EmuTOS by every 4th 200 Hz timer-C interrupt, i.e. 50 Hz —
> `bios/vectors.S:521`, `bios/ikbd.c:530-536`; an earlier revision said "the
> ~50 Hz VBL", which was wrong) at the NewTOS profile. The
> **contract is unchanged** if adopted — the packed
> `(old_initial << 8) | old_repeat` return, the `< 0` per-parameter query,
> the `& 0xFF` store, defaults `15`/`2`. Peripheral + driver: see
> `docs/HARDWARE-PROFILE.md` §3.3, §3.5.

- **Timing base.** The 20 ms unit `kb_initial`/`kb_repeat` count is the
  system tick; on the Pi the `_hz_200` clock ticks 200 Hz and `Tickcal` = 20
  ms (`HARDWARE-PROFILE.md` §3.5, `tos-api/src/bios/mod.rs` `TICKCAL_MS=20`),
  so the defaults `15`/`2` are ≈300 ms initial / ≈40 ms repeat, exactly as
  on the Atari.
- **Auto-repeat source on the Pi.** The USB-HID boot keyboard does **not**
  repeat: it reports changes only (FreeMiNT even sets its idle rate to
  "infinite", `references/freemint/sys/usb/src.km/udd/hid/keyboard/usb_keyboard.c:599`),
  and `kernel/src/drivers/usb/hid.rs` (`keyboard_report`) turns a report into
  a make only for a *new* usage and a break only for a *vanished* one. (An
  earlier revision of this entry said "USB-HID does its own key-hold repeat"
  — that was wrong; corrected 2026-10-03, KEYS01.) The repeat is therefore the
  BIOS software repeat of EmuTOS (`bios/ikbd.c:395-536`), reimplemented with
  the same delay/rate semantics in `kernel/src/key_repeat.rs` and driven from
  `kernel/src/ikbd.rs` (`poll_repeat`). It reads the **same** two byte fields
  this call sets (`tos_api::xbios::system::kb_rate`): `kb_initial` at each
  make, `kb_repeat` at each repeat, exactly as EmuTOS reads `kb_initial`/
  `kb_repeat`. So `Kbrate(-1,-1)` reports the values actually in use, and a
  set changes the timing of the next make / next repeat.
- **Where the repeat runs — sanctioned deviation.** EmuTOS runs
  `do_key_repeat` from the 50 Hz half of the timer-C interrupt. NewTOS runs
  it in the polled input pump, after each USB poll, crediting the 50 Hz steps
  `_hz_200` crossed since the previous poll, at most **one** repeat per poll
  (burst guard). Reason: on the Pi the break arrives only through polling, so
  an interrupt-driven repeat would emit ghost keys after the release. Full
  semantics and the burst-guard rationale: `docs/api/xbios/_state.md`
  §"Key-repeat handler"; listed in `docs/COMPATIBILITY.md` §"Sanctioned
  deviations".
- **Wired.** `Kbrate` (XBIOS `0x23`) is in the dispatcher
  (`tos-api/src/xbios/mod.rs`, `KBRATE => system::kbrate`), byte-identical to
  `bios/ikbd.c:439-449` (packed old value, `< 0` query, `& 0xFF` store).
- **Datasheet.** No peripheral register (software repeat timer; the ARM
  generic timer / BCM system timer provides the tick — §3.5).
