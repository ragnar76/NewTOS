# `Kbshift` (BIOS 0x0B)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/ikbd.c:145` (`kbshift`).

## NewTOS status: **out-of-scope (not exposed)**

NewTOS handles the keyboard natively (real ST keyboard on a UART, or USB-HID →
`crate::ikbd::feed`); modifier state lives in the `ikbd` decoder, not behind
this trap. `Kbshift` is therefore not exposed and the dispatcher's
out-of-range fall-through (returning the function number) is the intended,
documented behaviour — **not** the answer this spec describes, which is the
EmuTOS contract kept for reference and for the Phase-10 m68k emulator. See
`docs/STATUS.md` (2026-07-02) and the "Keyboard via Atari IKBD" decision.

---

## EmuTOS source

**Primary location:** `bios/ikbd.c:145` — `LONG kbshift(WORD flag)`

**Helpers called:** none.

**Headers / type definitions used:**
- `bios/ikbd.h:30` — `extern UBYTE shifty;` (the modifier-state byte)
- `include/biosdefs.h:90-100` — `MODE_RSHIFT`/`MODE_LSHIFT`/`MODE_CTRL`/
  `MODE_ALT`/`MODE_SCA`/`MODE_CAPS`/`MODE_HOME`/`MODE_INSERT`
- `bios/ikbd.h:26,28` — `MODE_SHIFT`, `HOTSWITCH_MODE` (private combinations)

**System variables / globals read/written:**
- `shifty` (`bios/ikbd.c:147,150`) — a `UBYTE` maintained by the IKBD scancode
  ISR; also published via `os_kbshift` in the OSHEADER
  (`include/biosdefs.h:59`) and pointed to from `bios/startup.S:109`.

**Build configuration affecting this function:** none — `kbshift` itself is
configuration-independent. (Whether `shifty` is updated depends on the IKBD/
ACIA being present, i.e. `CONF_ATARI_HARDWARE`/keyboard init.)

---

## Source extraction summary

`Kbshift` gets or sets the keyboard modifier-state byte `shifty`. It saves the
current value, and — only if `flag >= 0` (a **signed** `WORD` test) — stores
`flag` into `shifty`; it always returns the saved (previous) value. So
`flag == -1` (0xFFFF, negative) is the "query only" idiom.

Note the width mismatch: the parameter is a `WORD` but `shifty` is a `UBYTE`.
Storing `flag` into `shifty` (`bios/ikbd.c:150`) **truncates to 8 bits** — any
value in bits 8..15 of `flag` is silently discarded. The return is
`WORD oldshifty = shifty;` (zero-extended from the byte), delivered as a LONG
by the dispatcher (upper bits 0).

`shifty` is updated in interrupt context by the IKBD scancode handler
(`bios/ikbd.c:780-839`): key-down sets a bit, key-up clears it; CapsLock is
toggled on key-down.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x0B
- **Stack frame on entry (m68k):**
  ```
  +2: flag (word)
   0: function_id (word) = 0x000B
  ```
- **Total parameter bytes:** 4
- **Native (AArch64) ABI:** ID `0x0B` in `x8`; `flag` in `x0`; the previous
  (zero-extended) modifier byte in `x0`.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `flag` | `WORD` (16, signed test) |

---

## Inputs

| Position | Name   | Type   | Size | Source location   | Notes |
|----------|--------|--------|------|-------------------|-------|
| arg0     | `flag` | `WORD` | 2    | `bios/ikbd.c:145` | new state if `>= 0`; `-1` = query. Only low 8 bits are stored (truncated into a `UBYTE`). |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (zero-extended from the `UBYTE` `shifty`)
- **Encoding:** the **previous** `shifty` value in bits 0–7, bits 8–31 zero.
  Returned unconditionally.
- **Source location:** `bios/ikbd.c:147,152`.

### `shifty` bit layout (`include/biosdefs.h:90-100`)

| Bit | Mask | Constant      | Meaning |
|----:|-----:|---------------|---------|
| 0   | 0x01 | `MODE_RSHIFT` | Right Shift down |
| 1   | 0x02 | `MODE_LSHIFT` | Left Shift down |
| 2   | 0x04 | `MODE_CTRL`   | Control down |
| 3   | 0x08 | `MODE_ALT`    | Alternate down |
| 4   | 0x10 | `MODE_CAPS`   | CapsLock engaged (toggle) |
| 5   | 0x20 | `MODE_HOME`   | Clr/Home down (only while Alt held) |
| 6   | 0x40 | `MODE_INSERT` | Insert down (only while Alt held) |
| 7   | 0x80 | (reserved)    | unused |

Combinations: `MODE_SHIFT = MODE_RSHIFT|MODE_LSHIFT = 0x03`;
`MODE_SCA = 0x0F` (Shift+Ctrl+Alt); `HOTSWITCH_MODE = MODE_LSHIFT|MODE_ALT =
0x0A` (`bios/ikbd.h:26,28`). Per `biosdefs.h:96-98`, `MODE_HOME`/`MODE_INSERT`
are only set while Alt is already down, and stay set until their own key is
released even if Alt is released first.

### Side effects on global state

- `shifty` overwritten with `(UBYTE)flag` when `flag >= 0`
  (`bios/ikbd.c:150`). No other state.

### Side effects on hardware

None (does not command the keyboard controller).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (none — cannot fail) | `bios/ikbd.c:145` | previous `shifty` |

---

## Verification criteria

- **Query:** `Kbshift(-1)` returns the current `shifty` and does not change it.
- **Set + return old:** `Kbshift(MODE_CAPS)` returns the previous value and
  then `Kbshift(-1) == MODE_CAPS` (0x10).
- **Truncation:** `Kbshift(0x0104)` stores only `0x04` into `shifty`
  (`Kbshift(-1)` afterwards returns `0x04`); the high byte is dropped.
- **Always zero-extended:** the return never has bits 8–31 set.

---

## Edge cases discovered in source

1. **`flag` is tested as signed; stored as a byte.** `flag >= 0`
   (`bios/ikbd.c:149`) treats `-1` (0xFFFF) as query; a stored value is
   truncated to 8 bits because `shifty` is a `UBYTE` (`bios/ikbd.c:150`). Prior
   spec noted the truncation correctly.

2. **You can set states the keyboard isn't in.** `Kbshift(MODE_CAPS)` makes the
   system believe Caps Lock is on even if the key is up. EmuTOS does not
   reconcile the written state against physical key positions.

3. **Race with the IKBD ISR.** The scancode handler updates `shifty` from
   interrupt context (`bios/ikbd.c:780-839`); a `Kbshift` write racing with a
   key event can lose the event's bit change. On Atari the ISR runs at high IPL
   so the window is small.

4. **`os_kbshift` aliases `&shifty`.** Programs can read modifier state without
   trapping by dereferencing `os_kbshift` (`include/biosdefs.h:59`); both paths
   see the same byte.

5. **CapsLock is a sticky toggle** set on key-down (not key-up), so it persists
   until the next CapsLock press.

6. **No NumLock.** The Atari keyboard has none; the numpad is numeric unless
   Alt selects cursor mode.

---

## Dependencies

### Requires (must be initialized / functional)

- `shifty` maintained by the IKBD scancode handler (`kbd_init` + ACIA IRQ).

### Used by

- AES `evnt_keybd` / key-state queries; GEMDOS console reads that report shift
  state; `bios_init` itself (`kbshift(-1)` probed for Ctrl/Alt at boot,
  `bios/bios.c:517,812`).

---

## Related data structures

- `docs/api/structures/osheader.md` — `os_kbshift` field aliasing `shifty`
- `docs/api/bios/_state.md` — `shifty` in the BIOS state catalog

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: `flag = -1`
  query, same bit layout.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `MODE_HOME`/`MODE_INSERT` (bits 5,6) are TT/Falcon-era modifiers; on a plain
  ST keyboard they are never set. `kbshift` itself is version-independent.

---

## NewTOS implementation notes

(Empty during Phase 00.)

Not exposed as a trap in NewTOS (see status banner). If a future phase needs
it for the m68k emulator, the USB-HID / IKBD decoder must publish a
`shifty`-equivalent byte with this exact bit layout and mirror it into the
`os_kbshift` slot.

---

## Pi / BCM2711 binding

> Re-points the modifier-state source from the **IKBD 6301 / ACIA** (`shifty`
> maintained by the ACIA scancode ISR) to the **USB-HID keyboard over
> xHCI/PCIe** decoded into an IKBD `shifty`-equivalent. `Kbshift` the **trap**
> is out-of-scope (see the status banner); this binding pins where the
> modifier byte actually lives on the Pi. See `docs/HARDWARE-PROFILE.md` §3.3.

### The modifier byte exists — but not behind this trap

`Kbshift` (BIOS 0x0B) is **not wired** in the BIOS dispatcher
(`tos-api/src/bios/mod.rs` has no `0x0B` entry), so a native `Kbshift` call hits
the dispatcher's out-of-range fall-through (returns the function number, `0x0B`)
— the intended out-of-scope behaviour, **not** the `shifty` contract above.

The modifier state itself *is* maintained: the USB-HID report-modifier byte is
translated into IKBD modifier make/break codes (`drivers/usb/hid.rs`), fed
through `crate::ikbd::feed`, and exposed as a `shifty`-equivalent byte via the
host method `keyboard_shift_state()` → `crate::ikbd::shift_state()`
(`kernel/src/host_env.rs:231-233`). But that byte reaches consumers (the AES /
VDI key-state path) through the host trait, **not** through the `Kbshift` trap
and **not** through an `os_kbshift` low-memory alias.

| Atari reference | Pi-4 model today |
|-----------------|------------------|
| `shifty` UBYTE updated by the ACIA/IKBD scancode ISR | a `shifty`-equivalent kept in the `ikbd` decoder, updated from USB-HID make/break |
| `Kbshift(flag)` trap gets/sets it | trap **not exposed** → dispatcher returns `0x0B` |
| `os_kbshift` aliases `&shifty` for pointer reads | no `os_kbshift` slot; access is `host().keyboard_shift_state()` (EL1 only) |
| IKBD 6301 on ACIA | USB-HID keyboard over xHCI (VL805/PCIe) → `ikbd::feed` |

### Gaps vs. the contract (Step-2 candidates)

- **Trap not exposed (by design).** `Kbshift` returns the function number, not a
  `shifty` byte — the documented out-of-scope status. If the m68k emulator ever
  needs it, wire `0x0B` to a getter/setter over `ikbd::shift_state()` with the
  exact bit layout above (note the Pi decoder currently tracks Ctrl/Shift/Alt/
  Caps per `HARDWARE-PROFILE.md` §3.3; `MODE_HOME`/`MODE_INSERT` bits 5-6 are
  TT/Falcon-era and not produced).
- **No settable path.** The host method is read-only (`keyboard_shift_state()`);
  the `flag >= 0` "set the modifier state" half of the contract has no backing.
