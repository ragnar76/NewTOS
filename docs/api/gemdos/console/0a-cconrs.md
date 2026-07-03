# `Cconrs` (GEMDOS 0x0A)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:533` (`xconrs`) and `:540` (`cgets`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the length-prefixed buffer layout (`buf[0]` = max in,
> `buf[1]` = count out, `buf[2..]` = chars), the editing-key set, CR-terminates,
> the Ctrl-C terminate. Reference: `cgets`/`backsp`/`newline`/`cookdout`.

## EmuTOS source

**Primary location:** `bdos/console.c:533` — `void xconrs(char *p)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x0A` =
`xconrs`, `stdio_typ = 0x80` (stdin), `wparms = 2` (one LONG pointer)
(`bdos/bdosmain.c:165`). When stdin is redirected (`run->p_uft[0] > 0`) the
dispatcher runs a **simplified** CR-terminated read loop against the file, with
**no** line editing — see Edge case 5.

**Helpers called:**
- `bdos/console.c:540` — `cgets(h,maxlen,buf)` — the line editor.
- `bdos/console.c:351` — `getch(h)` — read one char (typeahead / `Bconin`).
- `bdos/console.c:263` — `conout(h,ch)` — cooked echo (`conbrk`+`Bconout`+column).
- `bdos/console.c:312` — `cookdout(h,ch)` — echo, rendering ctrl chars as `^X`.
- `bdos/console.c:494` — `backsp(h,cbuf,retlen,col)` — erase one char visually.
- `bdos/console.c:480` — `newline(h,startcol)` — CR/LF then re-indent to col.
- `terminate()` = `xterm(-32)` (`bdos/console.c:101`).

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM(h) = 3+h`.

**System variables / globals read:**
- `run->p_uft[0]` (`bdos/console.c:535`) — stdin device, used for **both** read
  **and** echo.
- `glbcolumn[2]` (start column captured by `cgets`).
- `con_typeahead` (via `getch`).

**System variables / globals written:**
- `glbcolumn[2]` (via echo), `con_typeahead.remove`, and the caller's buffer
  (`p[1]` = count, `p[2..]` = characters).

**Build configuration affecting this function:** none directly.

---

## Source extraction summary

`Cconrs` reads an edited line from the console into a caller-supplied,
**length-prefixed** buffer. The argument `p` points at a record:

| Byte    | Direction | Meaning |
|---------|-----------|---------|
| `p[0]`  | IN        | maximum length (0-255), the caller's limit |
| `p[1]`  | OUT       | actual number of characters read (written by `Cconrs`) |
| `p[2…]` | OUT       | the characters themselves (caller-allocated, **not** NUL-terminated) |

`xconrs` calls `cgets(HXFORM(p_uft[0]), (unsigned char)p[0], &p[2])` and stores
the returned length in `p[1]`. `cgets` loops reading with `getch` and echoing
with `cookdout`/`conout` on the **same** stdin device `h`, until the user
presses CR or LF, or until `retlen == maxlen`. On CR/LF it echoes a CR (only —
no LF) and returns the accumulated length.

The editing keys handled by `cgets` (`bdos/console.c:548-579`):

| Key                    | Action |
|------------------------|--------|
| CR (0x0D) / LF (0x0A)  | echo CR, finish, return `retlen`. |
| BS (0x08) / RUB (0x7F) | delete the last character (visual `backsp`). |
| Ctrl-C (0x03)          | `terminate()` — process exits. |
| Ctrl-X (0x18)          | erase the whole line (repeat `backsp` to empty). |
| Ctrl-U (0x15)          | echo `#`, newline, restart the line (`retlen = 0`). |
| Ctrl-R (0x12)          | echo `#`, newline, redisplay the current buffer; continue. |
| any other byte         | store in `buf[retlen++]`, echo via `cookdout`. |

`cookdout` renders control bytes below 0x20 as a caret pair (`^` then
`ch | 0x40`), except tab, which it tab-expands; printable bytes echo verbatim.
`backsp` recomputes the target column from the whole buffer (accounting for the
width-2 `^X` control echo and tab stops), then emits `BS SP BS` sequences until
the cursor lands there.

The output buffer is **not** NUL-terminated; the caller must use `p[1]` as the
length. `maxlen == 0` makes the loop body never run — `cgets` returns 0 and
`p[1]` is set to 0.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x0A
- **Stack frame on entry (m68k):**
  ```
  +2: p (LONG, pointer to the length-prefixed buffer)
   0: function_id (word) = 0x000A
  ```
- **Total parameter bytes:** 6 → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x0A` in `x8`; `p` in `x0` (full 64-bit
  pointer); no meaningful return (see below).

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `p`  | `char *` (ptr) — `[max, len, buf…]` |

---

## Inputs

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| arg0     | `p`  | `char *` | 4 | `bdos/console.c:533` | line-input record; `p[0]` = max length in |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** **undefined** — `xconrs` is `void`; the result is delivered
  through the output buffer (`p[1]` = count). D0 holds ABI scratch.
- **Source location:** `bdos/console.c:533` (void).

### Side effects on global state

- Writes `p[1]` (count) and `p[2..]` (characters) in the caller's buffer.
- Advances `glbcolumn[h]` and consumes typeahead/BIOS input during editing.

### Side effects on hardware

- Reads keyboard input and writes echo bytes to the console via BIOS.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

Ctrl-C terminates the process. There is no error code; the buffer's `p[1]` is
the only result.

**Error paths not in Compendium:** none.

---

## Verification criteria

- **Nominal:** with `p[0] = 10` and the user typing `AB` then Enter, after the
  call `p[1] == 2`, `p[2] == 'A'`, `p[3] == 'B'`; a CR (no LF) is echoed.
- **Max length reached:** with `p[0] = 2` and the user typing three characters,
  the line finishes at 2 characters without needing Enter; `p[1] == 2`.
- **`maxlen == 0`:** the call returns immediately with `p[1] == 0`.
- **Backspace:** typing `A`, BS, `B`, Enter yields `p[1] == 1`, `p[2] == 'B'`.
- **Ctrl-X:** clears the whole line; a subsequent Enter yields `p[1] == 0`.
- **Ctrl-C:** terminates the process; `Cconrs` does not return.
- **Not NUL-terminated:** `p[2 + p[1]]` is untouched by the call.
- **Redirected stdin:** with `run->p_uft[0]` forced to a file, the dispatcher
  reads up to `p[0]` bytes, stops at a CR (and eats the following LF), stores
  the count in `p[1]` — **no** BS/Ctrl-U/Ctrl-R editing (`bdos/bdosmain.c:535-553`).

---

## Edge cases discovered in source

1. **Length-prefixed, not NUL-terminated.** `p[0]` = max in, `p[1]` = count out,
   `p[2..]` = data. The caller must honour `p[1]`; no NUL is appended.

2. **Echo target is the stdin device, not stdout.** `xconrs` passes
   `HXFORM(p_uft[0])` for both read and echo (`bdos/console.c:535`). Redirecting
   stdout does not move the echo.

3. **CR is echoed without LF.** On finish, `cgets` emits only CR
   (`bdos/console.c:552`); the caller must print LF itself for a clean newline.

4. **Buffer overrun is prevented by construction.** `retlen` is bounded by
   `maxlen` before each store; BS/Ctrl-X only decrement `retlen` (never below 0)
   (`bdos/console.c:546,578`).

5. **Redirected stdin uses a simplified loop.** The dispatcher's `case 10`
   reads byte-by-byte, terminates on CR (consuming the trailing LF), and does no
   line editing (`bdos/bdosmain.c:535-553`). The editing keys are a console-only
   feature.

6. **Ctrl-R / Ctrl-U reset the visible line** via `newline` + optional redisplay
   with `cookdout`, but Ctrl-R keeps `retlen`, whereas Ctrl-U resets it to 0.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console I/O `Bconin(2)`, `Bconstat(2)`, `Bconout(2,…)`.
- `stdhdl_init()` (sets `p_uft[0] = H_Console`); `glbcolumn[2]`.
- `Pterm` (via `terminate()`) — for Ctrl-C.
- `con_typeahead`.

### Used by

- BASIC-style `INPUT`, command-line interpreters, configuration prompts.

---

## Related data structures

- `docs/api/bios/02-bconin.md`, `03-bconout.md`.
- `docs/api/gemdos/_state.md §7` — `conterm` (not read here).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the
  length-prefixed buffer and the editing keys — consistent with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Editing-key set and the CR-only echo are EmuTOS behaviour matching Atari TOS.

---

## NewTOS implementation notes

(Empty during Phase 00. Blocked on `Pterm` — implement the leaf console
functions first; defer the line editor until `xterm(-32)` exists.)
