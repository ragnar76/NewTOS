# `Cconis` (GEMDOS 0x0B)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:159` (`xconstat`) and `:143` (`constat`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the truth-valued status return (`-1` available, `0`
> not), and the `0` for a non-CON/AUX handle. Reference: the typeahead buffer.

## EmuTOS source

**Primary location:** `bdos/console.c:159` — `long xconstat(void)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x0B` =
`xconstat`, `stdio_typ = 0x80` (stdin), `wparms = 0` (`bdos/bdosmain.c:166`).
When stdin is redirected to a file (`run->p_uft[0] > 0`) the dispatcher returns
`eof(h) ? 0L : -1L` instead of entering this body — see Edge case 2.

**Helpers called:**
- `bdos/console.c:143` — `constat(h)` — typeahead check, else `Bconstat(h)`.
- BIOS `Bconstat(2)` (0x01).

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM(h) = 3+h`.

**System variables / globals read:**
- `run->p_uft[0]` (`bdos/console.c:161`) — stdin device.
- `con_typeahead` (via `constat`, through `buffer[2]`).

**System variables / globals written:** none.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Cconis` reports whether a character is available on the console input. `xconstat`
resolves stdin from `run->p_uft[0]` (default `H_Console` → BIOS handle 2) and
calls `constat(h)`.

`constat` only recognises handles 1 (AUX) and 2 (CON): for any handle outside
`1..2` it returns `0L` unconditionally (`bdos/console.c:147-148`). For a valid
handle it first checks the typeahead buffer — if `add > remove` it returns
`-1L` without touching hardware — otherwise it returns `Bconstat(h)`, whose
value is the BIOS ready flag (`-1`/non-zero if a byte is waiting, `0` if not).

So the result is truth-valued: non-zero (canonically `-1`) means "input
available", `0` means "none". When stdin has been redirected to a file, the
dispatcher answers with `eof`-based logic instead of this function.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x0B
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x000B
  ```
  No parameters.
- **Total parameter bytes:** 2 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x0B` in `x8`; no argument registers; LONG
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
- **Encoding:** truth-valued —

  | Condition | Value |
  |-----------|-------|
  | CON/AUX typeahead buffer non-empty | `-1L` |
  | typeahead empty, BIOS reports a byte waiting | `Bconstat(h)` (non-zero, canonically `-1`) |
  | nothing available | `0L` |
  | handle not 1 or 2 (e.g. stdin forced to PRN/handle 0/other) | `0L` |
- **Source location:** `bdos/console.c:152`.

### Side effects on global state

- None (a status probe; it does not consume input).

### Side effects on hardware

- May read the BIOS input-ready flag (`Bconstat` briefly disables interrupts on
  the IKBD path).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| stdin resolves to a handle outside `1..2` | `bdos/console.c:147-148` | `0L` |

**Error paths not in Compendium:** the `0L` for an unsupported handle is not an
error code — it is indistinguishable from "no input".

---

## Verification criteria

- **Available:** with a byte waiting on the console, `Cconis()` returns `-1L`.
- **Not available:** with the input idle and typeahead empty, `Cconis()` returns
  `0L`.
- **Typeahead short-circuit:** if the CON typeahead buffer holds a byte, `Cconis`
  returns `-1L` without polling the BIOS.
- **Non-consuming:** two `Cconis` calls with no intervening read both report the
  same availability; a following `Cconin` still returns the byte.
- **Redirected stdin:** with `run->p_uft[0]` forced to a file, `Cconis` returns
  `0L` at EOF and `-1L` otherwise (`bdos/bdosmain.c:555-558`).

---

## Edge cases discovered in source

1. **Only handles 1 and 2 are supported.** `constat` returns `0L` for any other
   handle (`bdos/console.c:147-148`) — so if stdin is redirected to PRN
   (handle 0) or an out-of-range value, `Cconis` always reports "no input".

2. **Typeahead is consulted first.** A non-empty buffer yields `-1L` with no
   hardware access (`bdos/console.c:152`).

3. **Redirected stdin uses `eof`.** The dispatcher returns `0L` at EOF, `-1L`
   before EOF (`bdos/bdosmain.c:555-558`) — file "input" is "available" until the
   file is fully consumed.

4. **The exact non-zero value on the hardware path is `Bconstat`'s.** EmuTOS's
   `Bconstat` returns `-1`/`0`, so the canonical result is `-1L`, but a program
   should test for non-zero, not `== -1`.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console status `Bconstat(2)`.
- `stdhdl_init()` (sets `p_uft[0] = H_Console`).
- `con_typeahead`.

### Used by

- Input-polling loops, EmuCON, the AES keyboard event path.

---

## Related data structures

- `docs/api/bios/01-bconstat.md` — the underlying status BIOS call.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `-1`/`0` status
  — consistent with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Configuration-independent.

---

## NewTOS implementation notes

(Empty during Phase 00.)
