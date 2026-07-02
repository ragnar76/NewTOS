# `Keytbl` — XBIOS 0x10

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/ikbd.c:122` (`keytbl`).

## NewTOS status: **out-of-scope**

NewTOS translates scancodes to ASCII with a native table inside `crate::ikbd`,
driven by the IKBD path (ST keyboard on a UART / USB-HID → `ikbd::feed`). It
does not expose the Atari `KEYTAB` structure, so `Keytbl` is **not exposed** as
a trap; the XBIOS dispatcher's `EINVFN` catch-all is the intended, documented
behaviour (not a stub). See `docs/STATUS.md` (2026-07-02) and the "Keyboard via
Atari IKBD" decision.

## EmuTOS source

```c
LONG keytbl(const UBYTE *norm, const UBYTE *shft, const UBYTE *caps)
{
    if (norm != (UBYTE*)-1) current_keytbl.norm = norm;
    if (shft != (UBYTE*)-1) current_keytbl.shft = shft;
    if (caps != (UBYTE*)-1) current_keytbl.caps = caps;
    return (LONG) &current_keytbl;
}
```

`current_keytbl` is a 3-pointer struct (`keytbl`) consulted by the
IKBD scancode-to-ASCII translator on every keypress.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x10` |
| WORD args | 6 (LONG × 3) |
| Stack frame | `+10: caps (L)`, `+6: shft (L)`, `+2: norm (L)`, `+0: 0x0010` |
| Vector | `xbios_vecs[16]` |

## Inputs

| Pos | Name   | Type            | Range / meaning                                      |
|-----|--------|-----------------|------------------------------------------------------|
| 0   | `norm` | const UBYTE *   | 128-byte normal table, `-1` = leave alone, `0` = clear |
| 1   | `shft` | const UBYTE *   | shifted table, `-1` = leave alone                     |
| 2   | `caps` | const UBYTE *   | caps-lock table, `-1` = leave alone                   |

Each table maps scancode (0..127) to an 8-bit ASCII / extended
code.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (`struct keytbl *`) |

Pointer to the now-active `current_keytbl` struct. Programs that
want only to **query** the current tables call
`Keytbl(-1, -1, -1)` and dereference the result.

## Errors

None.

## Edge cases

### 1. `(UBYTE *)-1` sentinel

All three "no change" guards check against `-1` cast to a pointer.
Address `0xFFFFFFFF` cannot be a valid table — but `0` (NULL)
**is** treated as "clear / no table" and will crash the IKBD
handler on the next keypress. Programs querying the current
tables must use `-1`, not `0`.

### 2. Table layout is not validated

EmuTOS does not check the pointers' validity, alignment, or the
content. A bogus table corrupts every subsequent keystroke until
`Bioskeys` (XBIOS 0x18) restores the ROM defaults.

### 3. Atomicity vs IKBD ISR

The three pointer assignments are non-atomic. An IKBD interrupt
arriving between assignments dereferences a half-updated state.
EmuTOS does not guard. NewTOS must either disable IRQ around the
update or update via a single-pointer-swap to a new struct.

### 4. Country-table quirk

EmuTOS distinguishes _modifiable user table_ from _ROM country
default_. `Bioskeys` re-installs the country default. Programs
that allocate three new 128-byte buffers, fill them, and call
`Keytbl(b1, b2, b3)` survive `Bioskeys` only if they re-call
`Keytbl` after.

### 5. Extended scancodes

Some country variants need scancodes > 127 (dead keys, AltGr
combinations). The 128-byte limit forces those to be encoded as
side-effect state in the IKBD handler, not in the table itself.
NewTOS extends to 256 bytes via a NewTOS-extension entry point.

## Dependencies

- `current_keytbl` (BIOS BSS).
- IKBD interrupt handler (uses tables).

Used by: locale switchers, AES locale change, custom keyboard
layouts, dead-key emulators.

## Cross-references

- **Compendium:** "Keytbl" — agrees.
- **Profibuch:** "Keytbl (0x10)" — agrees, lists ROM default
  pointer mechanism.
- `docs/api/xbios/18-bioskeys.md`

## Configuration

- `CONF_LANG` — chooses default country.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: tables stay byte-for-byte compatible. NewTOS adds an
extension call `KeytblEx` (reserved XBIOS slot) that takes a
pointer to a 4-pointer struct including a 256-byte AltGr table.
