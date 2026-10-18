# `Crawcin` (GEMDOS 0x07)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:375` (`xrawcin`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, **blocking** single-char read, no echo, no Ctrl-key
> handling. Reference: the typeahead buffer and `getch`.

## EmuTOS source

**Primary location:** `bdos/console.c:375` — `long xrawcin(void)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x07` =
`xrawcin`, `stdio_typ = 0x80` (stdin), `wparms = 0` (`bdos/bdosmain.c:162`).
When stdin is redirected (`run->p_uft[0] > 0`) the dispatcher reads one byte
from the file and this body is not entered — see Edge case 3.

**Helpers called:**
- `bdos/console.c:351` — `getch(h)` — dequeue typeahead, else `Bconin(h)`
  (blocking).

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM(h) = 3+h`.

**System variables / globals read:**
- `run->p_uft[0]` (`bdos/console.c:377`) — stdin device.
- `con_typeahead` (via `getch`).

**System variables / globals written:**
- `con_typeahead.remove` (via `getch`, if the buffer had data).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Crawcin` reads and returns one raw character from the console input, **without**
any special-key processing. `xrawcin` resolves stdin from `run->p_uft[0]`
(default `H_Console` → BIOS handle 2) and returns `getch(HXFORM(p_uft[0]))`.

`getch` returns a buffered typeahead character if one is queued, otherwise it
calls `Bconin`, which **blocks** until a character arrives. So `Crawcin` is
blocking — this is the key distinction from `Crawio`'s non-blocking input mode
(0x06 with `0xFF`).

Unlike `Cconin`/`Cnecin`, `Crawcin` performs **no** `conbrk` pre-drain: no
Ctrl-S pause, no Ctrl-C terminate, no echo. A Ctrl-C is returned to the caller
as `0x00000003`, and any Ctrl-S/Ctrl-X bytes previously queued into the
typeahead buffer by an earlier `Cconin`'s `conbrk` are handed back as data.

The returned LONG is `getch`'s value: scancode in bits 16-23, ASCII in bits 0-7
for the IKBD console.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x07
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0007
  ```
  No parameters.
- **Total parameter bytes:** 2 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x07` in `x8`; no argument registers; LONG
  result sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None. The device is `run->p_uft[0]`, not a parameter.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** scancode in bits 16-23, ASCII in bits 0-7 (console handle).
- **Source location:** `bdos/console.c:377` (forwards `getch`).

### Side effects on global state

- Consumes one typeahead or BIOS input character.

### Side effects on hardware

- Reads keyboard/ACIA input when the typeahead buffer is empty.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

**Error paths not in Compendium:** none.

---

## Verification criteria

- **Nominal:** with 'A' available, `Crawcin()` returns a LONG whose low byte is
  `0x41`.
- **Blocking:** with no input, the call does not return until a character
  arrives (distinct from `Crawio(0xFF)`).
- **No Ctrl-C:** typing Ctrl-C returns `0x00000003`; the process is **not**
  terminated.
- **No echo:** the character is not echoed to the console.
- **Redirected stdin:** with `run->p_uft[0]` forced to a file, one byte is read
  from the file; at EOF the call returns `EOF_INDICATOR` (`0x0000ff1a`).

---

## Edge cases discovered in source

1. **Blocking, unlike `Crawio(0xFF)`.** `getch` blocks on `Bconin` when the
   typeahead buffer is empty.

2. **No Ctrl-key processing at all.** No `conbrk` pre-drain → Ctrl-C returned as
   data (not terminate), Ctrl-S ignored, no echo.

3. **Redirected stdin bypasses this body.** `run->p_uft[0] > 0` →
   `xread(h,1,&ctmp)`; at EOF returns `EOF_INDICATOR` (`bdos/bdosmain.c:509-519`).

4. **Consumes leftover typeahead.** Bytes an earlier `Cconin`/`Cnecin` `conbrk`
   queued (including buffered Ctrl-S/Ctrl-X) are returned here verbatim.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console input `Bconin(2)`.
- `stdhdl_init()` (sets `p_uft[0] = H_Console`).
- `con_typeahead`.

### Used by

- Programs implementing a custom line discipline that still want the shared
  typeahead buffer but no special-key processing on this read.

---

## Related data structures

- `docs/api/bios/02-bconin.md` — the underlying byte-input BIOS call.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents a raw,
  no-echo, blocking single-char read — consistent with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Configuration-independent.

---

## NewTOS implementation notes

(Empty during Phase 00.)

TASK/CON01 (2026-10-18): `getch(h)` — BDOS typeahead first, else `Bconin(h)`;
no `conbrk`, no echo (`bdos/console.c:375-378`; `tos-api/src/gemdos/console.rs`
`crawcin`).
