# `Kbrate` — XBIOS 0x23

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/ikbd.c:439` (`kbrate`).

## NewTOS status: **out-of-scope**

`Kbrate` sets the Atari key-repeat delay/rate. NewTOS takes keyboard input
natively via the IKBD path (ST keyboard on a UART / USB-HID → `crate::ikbd`);
key repeat is owned by that input source, not this trap, so `Kbrate` is **not
exposed**; the XBIOS dispatcher's `EINVFN` catch-all is the intended, documented
behaviour (not a stub). See `docs/STATUS.md` (2026-07-02) and the "Keyboard via
Atari IKBD" decision.

## EmuTOS source

```c
WORD kbrate(WORD initial, WORD repeat)
{
    WORD ret = MAKE_UWORD(kb_initial, kb_repeat);

    if (initial >= 0)  kb_initial = initial & 0x00ff;
    if (repeat  >= 0)  kb_repeat  = repeat  & 0x00ff;

    return ret;
}
```

`kb_initial` and `kb_repeat` are byte-sized intervals (in 50 Hz
VBL ticks) consumed by `do_key_repeat()` from the VBL handler.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x23` |
| WORD args | 2 (WORD + WORD) |
| Stack frame | `+4: repeat (W)`, `+2: initial (W)`, `+0: 0x0023` |
| Vector | `xbios_vecs[35]` |

## Inputs

| Pos | Name      | Type | Range / meaning                                           |
|-----|-----------|------|-----------------------------------------------------------|
| 0   | `initial` | WORD | initial delay before repeat starts (0..255 VBLs), `-1` = query |
| 1   | `repeat`  | WORD | inter-repeat delay (0..255 VBLs), `-1` = query             |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended WORD) |

Packed as `(prev_kb_initial << 8) | prev_kb_repeat` — the previous
values, suitable for save-and-restore.

## Errors

None.

## Edge cases

### 1. Defaults

EmuTOS defaults: `kb_initial = 25` (≈500 ms at 50 Hz),
`kb_repeat = 5` (≈100 ms). Atari TOS uses 25/5 too.

### 2. `0` value behaviour

`kb_initial = 0`: every keypress repeats from the moment of press,
no initial delay. `kb_repeat = 0`: repeats once per VBL — the
fastest possible.

### 3. High-byte truncation

`& 0x00ff` discards the high byte. `Kbrate(0x0100, 0x0500)`
quietly clamps to `0/5` — the high byte is silently dropped.

### 4. Global vs per-process

`kb_initial` / `kb_repeat` are global. AES does not save/restore
on process switch; the last-set values persist across program
invocations. Programs altering them should restore on exit.

### 5. Disabled vs slow

Setting `conterm` bit 1 = 0 disables key repeat entirely (in
`do_key_repeat()`). Programs that want "no repeat" should toggle
`conterm` rather than set huge `kb_initial`/`kb_repeat`.

## Dependencies

- `kb_initial`, `kb_repeat` (BIOS BSS).
- VBL key-repeat handler (`do_key_repeat`).
- `conterm` global (XBIOS 0x10 OS variable for repeat enable).

Used by: AES control panel, accessibility programs, fast-typing
games.

## Cross-references

- **Compendium:** "Kbrate" — agrees.
- **Profibuch:** "Kbrate (0x23)" — agrees, notes 50 Hz unit.
- `docs/api/xbios/_state.md` (pending) — `conterm` enable flag.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS keeps the 50 Hz VBL unit for ABI compatibility
even though native HDMI is 60 Hz — translate by multiplying by
60/50 internally before driving the USB-HID repeat timer.
