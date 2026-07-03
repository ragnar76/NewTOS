# `Cconin` (GEMDOS 0x01)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:396` (`xconin`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the packed-LONG result encoding, blocking, the
> unconditional echo, the Ctrl-C deferred terminate. Reference: the typeahead
> buffer, `conbrk`/`getch`/`conin` helpers, `glbcolumn[]`.

## EmuTOS source

**Primary location:** `bdos/console.c:396` — `long xconin(void)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x01` =
`xconin`, `stdio_typ = 0x80` (stdin), `wparms = 0` (`bdos/bdosmain.c:156`). The
`0x80` flag means the GEMDOS dispatcher intercepts the call **when stdin has
been redirected to a file** (`run->p_uft[0] > 0`) and reads the byte from the
file instead of entering `xconin` — see Edge case 5.

**Helpers called:**
- `bdos/console.c:206` — `conbrk(h)` — drains `Bconstat`/`Bconin` into the
  typeahead buffer, honouring Ctrl-S/Ctrl-Q/Ctrl-X and terminating on Ctrl-C.
- `bdos/console.c:384` — `conin(h)` — `getch(h)` then echo via `conout(h,…)`.
- `bdos/console.c:351` — `getch(h)` — dequeue from typeahead, else `Bconin(h)`.
- `bdos/console.c:263` — `conout(h,ch)` — `conbrk` + `Bconout` + column track.
- `terminate()` = `xterm(-32)` (`bdos/console.c:101`).
- BIOS `Bconin(2)` (0x02), `Bconstat(2)` (0x01), `Bconout(2,…)` (0x03).

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console = -1`, `HXFORM(h) = 3+h`.
- `bdos/fs.h:46-47` — `KBBUFSZ = 64`, `KBBUFMASK = 63` (typeahead size).
- `include/portab.h:142` — `LOBYTE(x)`.

**System variables / globals read:**
- `run->p_uft[0]` (`bdos/console.c:401`) — the stdin standard handle.
- `con_typeahead` (`bdos/console.c:45`) — CON typeahead buffer (via `buffer[2]`).
- `glbcolumn[2]` (`bdos/console.c:54`) — screen column, updated by echo.

**System variables / globals written:**
- `con_typeahead.add`/`.remove` (via `conbrk`/`getch`).
- `glbcolumn[2]` (via `conout` during echo).

**Build configuration affecting this function:** none directly. Transitive: the
underlying BIOS device depends on `CONF_WITH_IKBD_ACIA` / VT52 for the console.

> **Note:** `Cconin` does **not** read `conterm`. Echo and Ctrl-C/S/Q handling
> here are **unconditional** in this EmuTOS build. `conterm` affects only the
> BIOS keyboard layer (key click / repeat / bell / shift-status packing), not
> the GEMDOS console layer. See `_state.md §7` and discrepancy D-027.

---

## Source extraction summary

`Cconin` reads and returns one character from the console input, blocking until
one is available. `xconin` resolves the current stdin device from
`run->p_uft[0]` through `HXFORM` (default `H_Console = -1` → BIOS handle 2), then
calls `conbrk(h)`. `conbrk` peeks `Bconstat`; if input is pending it dequeues
via `Bconin`, acting on Ctrl-S (pause), Ctrl-Q (resume), Ctrl-X (flush buffer)
and Ctrl-C (flush buffer + `terminate()`), queuing everything else into the
typeahead buffer. It then calls `conin(h)`, which fetches a character with
`getch` (typeahead first, else `Bconin`) and **echoes it unconditionally** to
the same device `h` via `conout`.

After `conin` returns, `xconin` checks the low byte: if it is Ctrl-C (0x03) it
calls `terminate()` and does not return. So a Ctrl-C can end the process from
two places — inside the `conbrk` pre-drain, or as the freshly-read character.

The returned LONG is the value produced by the console BIOS: for the IKBD
console (handle 2) the low byte is the ASCII value (0 for non-ASCII keys), the
high word (bits 16-23) is the IKBD scancode. `xconin` forwards this LONG
verbatim; it does not repackage it. Echo goes to the **stdin device** `h`, not
to stdout — even if stdout is redirected, typed characters still echo to the
console.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x01
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0001
  ```
  No parameters.
- **Total parameter bytes:** 2 (function word only) → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x01` in `x8`; no argument registers used; the
  packed LONG result **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None. The device is taken from per-process state (`run->p_uft[0]`), not a
parameter.

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| —        | —    | —    | —    | —               | —     |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding (console, handle 2):** bits 0-7 = ASCII (0 if non-ASCII key);
  bits 8-15 = 0; bits 16-23 = IKBD scancode; bits 24-31 = 0. (Whether the
  shift-state byte appears in bits 24-31 is a BIOS `conterm & 8` decision, made
  by `Bconin`, not by `Cconin`.)
- **Source location:** value produced by `getch`/`Bconin`
  (`bdos/console.c:388`), forwarded unchanged by `xconin`.

### Side effects on global state

- Consumes one character from the CON typeahead buffer or the BIOS input queue.
- Echoes the character to device `h`, advancing `glbcolumn[h]`
  (`bdos/console.c:267-272`).

### Side effects on hardware

- Reads the keyboard/ACIA input (via BIOS), writes the echoed byte to the
  screen/console (via BIOS `Bconout`).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

`Cconin` has no error return. A NUL result is ambiguous with a non-ASCII key
whose ASCII byte is 0 (scancode in the high word). Ctrl-C does not return an
error — it terminates the process (`terminate()` = `xterm(-32)`).

**Error paths not in Compendium:** none (no error path exists).

---

## Verification criteria

- **Nominal (ASCII):** with 'A' typed on the console, `Cconin()` returns a LONG
  whose low byte is `0x41` and whose bits 16-23 carry the key's scancode; the
  character is echoed to the console.
- **Nominal (non-ASCII):** with a function/arrow key pressed, the low byte is 0
  and bits 16-23 are the scancode (`(r & 0xFF) == 0 && ((r >> 16) & 0xFF) != 0`).
- **Blocking:** with no input available, the call does not return until a
  character arrives.
- **Echo:** the returned character appears on the console output for that
  device (unconditionally — no `conterm` bit gates it).
- **Ctrl-C:** typing Ctrl-C (0x03) terminates the calling process; `Cconin`
  does not return to the caller.
- **Redirected stdin:** with `run->p_uft[0]` forced to a file handle, one byte
  is read from the file; at EOF the call returns `EOF_INDICATOR`
  (`0x0000ff1a`) instead of a keystroke (`bdos/bdosmain.c:517-519`).

---

## Edge cases discovered in source

1. **Echo goes to the stdin device, not stdout.** `conin` echoes on the same
   handle `h = HXFORM(p_uft[0])` it read from (`bdos/console.c:388`). Redirecting
   *stdout* does not move the echo; redirecting *stdin* moves both read and echo.
   (Correction: an earlier draft claimed the echo targets `p_uft[1]`.)

2. **Echo is unconditional.** There is no `conterm`-bit gate on echo in this
   build; every `Cconin` echoes. (Correction: an earlier draft implied echo is
   `conterm`-controlled — it is not; see D-027.)

3. **Ctrl-C fires from two places.** From the `conbrk` pre-drain (which
   `terminate()`s directly) and from the post-`conin` low-byte check
   (`bdos/console.c:404-405`). A Ctrl-C sitting in typeahead is caught by the
   drain before the read.

4. **Ctrl-S pauses inside the pre-drain.** `conbrk` spins re-reading `Bconin`
   with `stop = 1` until a Ctrl-Q arrives; `Cconin` blocks there. Non-control
   bytes read during the pause are queued into typeahead, not lost.

5. **Redirected stdin bypasses this body.** When `run->p_uft[0] > 0` the
   dispatcher reads one byte from the file (`xread(h,1,&ctmp)`) and returns it;
   `xconin` is not entered. At EOF it returns `EOF_INDICATOR = 0x0000ff1a`
   (MiNT-compatible), not garbage (`bdos/bdosmain.c:509-519`).

6. **Typeahead buffer is 64 entries of LONG.** `KBBUFSZ = 64` (`bdos/fs.h:46`);
   each slot holds a full packed LONG (scancode+ASCII), so the buffered value
   preserves the scancode, not just the ASCII byte.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console input: `Bconin(2)`, `Bconstat(2)`, `Bconout(2,…)`.
- `stdhdl_init()` — sets `run->p_uft[0] = H_Console`.
- `Pterm` (via `terminate()`) — for Ctrl-C.
- CON typeahead buffer (`con_typeahead`) and `glbcolumn[2]`.

### Used by

- C-library `getchar`; interactive prompts; the GEMDOS `Fread` path on a
  console handle uses `conin` directly (`bdos/bdosmain.c:615`).

---

## Related data structures

- `docs/api/gemdos/_state.md §7` — `conterm` (note: not read here).
- `docs/api/bios/02-bconin.md` — the packed-LONG input encoding.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: agrees on
  blocking + the scancode/ASCII packing + Ctrl-C abort.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `GEMDOS_VERSION = 0x2000` in this build (`include/config.h:1590`).
- The echo/Ctrl-handling here is EmuTOS behaviour; classic Atari TOS is the
  same for `Cconin`. Redirection uses the MiNT `EOF_INDICATOR` convention.

---

## NewTOS implementation notes

(Empty during Phase 00.)
