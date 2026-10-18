# `Cnecin` (GEMDOS 0x08)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:414` (`xnecin`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, blocking read with **no echo**, the Ctrl-S pause /
> Ctrl-C terminate. Reference: `conbrk`/`getch` and the typeahead buffer.

## EmuTOS source

**Primary location:** `bdos/console.c:414` — `long xnecin(void)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x08` =
`xnecin`, `stdio_typ = 0x80` (stdin), `wparms = 0` (`bdos/bdosmain.c:163`).
When stdin is redirected (`run->p_uft[0] > 0`) the dispatcher reads one byte
from the file and this body is not entered — see Edge case 3.

**Helpers called:**
- `bdos/console.c:206` — `conbrk(h)` — pre-drain input, honour Ctrl-S/Q/X,
  terminate on Ctrl-C.
- `bdos/console.c:351` — `getch(h)` — dequeue typeahead, else `Bconin(h)`
  (blocking).
- `terminate()` = `xterm(-32)` (`bdos/console.c:101`).

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM(h) = 3+h`.
- `include/portab.h:142` — `LOBYTE(x)`.

**System variables / globals read:**
- `run->p_uft[0]` (`bdos/console.c:419`) — stdin device.
- `con_typeahead` (via `conbrk`/`getch`).

**System variables / globals written:**
- `con_typeahead.add`/`.remove` (via `conbrk`/`getch`).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Cnecin` reads and returns one character from the console input **without
echoing it**, blocking until one is available. It is identical to `Cconin`
except for the missing echo: `xnecin` calls `conbrk(h)` then `getch(h)`
directly, whereas `xconin` calls `conin(h)` (which is `getch` **plus** an echo
via `conout`).

`conbrk` pre-drains pending input, acting on Ctrl-S (pause until Ctrl-Q),
Ctrl-X (flush buffer), and Ctrl-C (flush + `terminate()`), queuing the rest into
the typeahead buffer. `getch` then returns the next character (typeahead first,
else blocking `Bconin`). After the read, `xnecin` checks the low byte: if it is
Ctrl-C (0x03) it calls `terminate()` and does not return. So, as with `Cconin`,
Ctrl-C can end the process from the pre-drain or as the read character.

The returned LONG carries the scancode in bits 16-23 and the ASCII byte in bits
0-7 (console handle). Typical use: password entry and single-key prompts where
the keystroke must not appear on screen.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x08
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0008
  ```
  No parameters.
- **Total parameter bytes:** 2 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x08` in `x8`; no argument registers; LONG
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
- **Source location:** `bdos/console.c:421` (forwards `getch`).

### Side effects on global state

- Consumes one typeahead or BIOS input character.
- **No echo** — `glbcolumn` is not advanced by this call.

### Side effects on hardware

- Reads keyboard/ACIA input when the typeahead buffer is empty.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

Ctrl-C terminates the process rather than returning an error.

**Error paths not in Compendium:** none.

---

## Verification criteria

- **Nominal:** with 'A' typed, `Cnecin()` returns a LONG whose low byte is
  `0x41` and it is **not** echoed to the console.
- **Blocking:** with no input, the call does not return until a character
  arrives.
- **Ctrl-C:** typing Ctrl-C terminates the process; `Cnecin` does not return.
- **Ctrl-S pause:** a pending Ctrl-S blocks inside the `conbrk` pre-drain until
  Ctrl-Q.
- **Redirected stdin:** with `run->p_uft[0]` forced to a file, one byte is read
  from the file; at EOF the call returns `EOF_INDICATOR` (`0x0000ff1a`).

---

## Edge cases discovered in source

1. **No echo.** The single difference from `Cconin`: `xnecin` calls `getch`
   directly, not `conin` (`bdos/console.c:421`).

2. **Ctrl-C fires from two places** — the `conbrk` pre-drain and the post-read
   low-byte check (`bdos/console.c:422-423`).

3. **Redirected stdin bypasses this body.** `run->p_uft[0] > 0` →
   `xread(h,1,&ctmp)`; at EOF returns `EOF_INDICATOR` (`bdos/bdosmain.c:509-519`).

4. **Typeahead-full during pre-drain rings the bell.** If `conbrk` cannot queue
   a byte (buffer full) it emits `Bconout(h,7)` (`bdos/console.c:239`).

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console input `Bconin(2)`, `Bconstat(2)`, `Bconout(2,…)` (bell).
- `stdhdl_init()` (sets `p_uft[0] = H_Console`).
- `Pterm` (via `terminate()`); `con_typeahead`.

### Used by

- Password / single-key prompts. Note: the `Cconrs` line editor `cgets` reads
  via `getch` **directly**, not via `Cnecin` — the two share the same `getch`
  path but `cgets` does not call `xnecin`.

---

## Related data structures

- `docs/api/bios/02-bconin.md` — the underlying byte-input BIOS call.
- `docs/api/gemdos/_state.md §7` — `conterm` (not read here).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents a blocking,
  no-echo single-char read with Ctrl-C abort — consistent with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Configuration-independent.

---

## NewTOS implementation notes

(Empty during Phase 00.)

TASK/CON01 (2026-10-18): `conbrk(h)` then `getch(h)` (BDOS typeahead first, else
`Bconin(h)`), no echo, Ctrl-C → `Pterm(-32)` (`bdos/console.c:414-426`;
`tos-api/src/gemdos/console.rs` `cnecin`).
