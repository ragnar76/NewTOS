# `Kbshift` — BIOS 0x0B

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/ikbd.c:145` (`kbshift`).

## NewTOS status: **out-of-scope**

NewTOS handles the keyboard natively through the IKBD path — a real ST
keyboard on a UART, or a USB-HID keyboard → `crate::ikbd::feed` — not the
Atari keyboard-table trap ABI. Modifier state is tracked inside the `ikbd`
decoder, not surfaced through this trap, so `Kbshift` is **not exposed** and
the BIOS dispatcher's `EINVFN` catch-all is the intended, documented behaviour
(not a stub). See `docs/STATUS.md` (2026-07-02) and the "Keyboard via Atari
IKBD" decision.

## EmuTOS source

```c
LONG kbshift(WORD flag)
{
    WORD oldshifty = shifty;
    if (flag >= 0)
        shifty = flag;
    return oldshifty;
}
```

`shifty` is a `UBYTE` global (`bios/ikbd.c`) maintained by the
IKBD scancode handler. Every keypress / release of a modifier key
toggles the corresponding bit. The byte is also exposed via
`os_kbshift` in the OSHEADER (see `docs/api/structures/osheader.md`).

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x0B` |
| WORD args | 1 (WORD) |
| Stack frame | `+2: flag (W)`, `+0: 0x000B` |
| Vector entry | `bios_vecs[11]` |

## Inputs

| Pos | Name   | Type | Range / meaning                              |
|-----|--------|------|----------------------------------------------|
| 0   | `flag` | WORD | new shift state, or `-1` for query-only       |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended UWORD) |

The previous value of `shifty`, low byte = bitfield, upper bits
zero. The function **always returns** the old value, regardless of
whether `flag` updated the state.

### `shifty` bits (`include/biosdefs.h:90-100`)

| Bit | Mask | Constant       | Meaning                       |
|----:|-----:|----------------|-------------------------------|
| 0   | 0x01 | `MODE_RSHIFT`  | Right Shift down              |
| 1   | 0x02 | `MODE_LSHIFT`  | Left Shift down               |
| 2   | 0x04 | `MODE_CTRL`    | Control down                  |
| 3   | 0x08 | `MODE_ALT`     | Alternate down                |
| 4   | 0x10 | `MODE_CAPS`    | CapsLock toggled on           |
| 5   | 0x20 | `MODE_HOME`    | Clr/Home down (TT/Falcon)      |
| 6   | 0x40 | `MODE_INSERT`  | Insert down (TT/Falcon)       |
| 7   | 0x80 | (reserved)     | unused                        |

`MODE_SHIFT = MODE_RSHIFT | MODE_LSHIFT = 0x03`
`MODE_SCA = 0x0F` (Shift + Ctrl + Alt mask)
`HOTSWITCH_MODE = MODE_LSHIFT | MODE_ALT = 0x0A`

## Errors

None.

## Edge cases

### 1. `flag` is signed

`flag >= 0` checked as signed comparison: `flag = -1` (0xFFFF) is
negative, treated as query. Any non-negative value (0..32767) is
written into `shifty` — note `shifty` is a `UBYTE` so only bits
0..7 are stored (bits 8..15 of `flag` are silently truncated).

### 2. Writing arbitrary states

`Kbshift(MODE_CAPS)` makes the system **think** Caps Lock is
engaged even if the physical key isn't down. Used by terminal
emulators and screen recorders. EmuTOS does not validate the
resulting state against actual key positions.

### 3. Race with IKBD interrupt

The IKBD scancode handler updates `shifty` from interrupt context.
A `Kbshift(flag)` write that races with a key event may lose the
key event's bit-toggle. Atari's handlers run at IPL ≥ 6 so
interruption is rare during the BIOS call itself.

### 4. `os_kbshift` aliasing

The OSHEADER exposes `&shifty` as `os_kbshift`. Programs can read
the modifier state without trapping by dereferencing
`*(UBYTE*)os_kbshift`. The two access paths return the same value.

### 5. CapsLock as toggle

`MODE_CAPS` is sticky — toggled on key-down, persists until next
press. The IKBD-event handler, not the keyup, toggles it.

### 6. NumLock not represented

The Atari keyboard has no NumLock key. The numpad is always in
"numeric" mode unless `MODE_ALT` is held (cursor mode). NewTOS on
PC keyboards must decide whether NumLock affects `shifty`; the
TOS-compat answer is "ignore it".

## Dependencies

- `shifty` global (`bios/ikbd.c`).

Used by: GEMDOS console reads (when "shift state" upper bits of D0
are populated), AES `evnt_keybd`, screen-key-state queries.

## Cross-references

- **Compendium:** "Kbshift" — agrees on `flag = -1` query.
- **Profibuch:** "Kbshift (0x0B)" — agrees on bit layout.
- `docs/api/bios/02-bconin.md` — embeds shift state in upper word.
- `docs/api/structures/osheader.md` — `os_kbshift` field.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-07**: USB HID keyboard driver computes `shifty` by
mapping HID modifier byte → Atari MODE_* bits. CapsLock toggles on
USB-key-down event. Clr/Home and Insert map to PC's Home and
Insert respectively. The byte is mirrored into the OSHEADER's
`os_kbshift` slot every update so both access paths stay
coherent.
