# `Cconout` (GEMDOS 0x02)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:279` (`xconout`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the return value `1` (D-001), tab-to-8-column
> expansion, the Ctrl-S pause / Ctrl-C terminate side effect. Reference: the
> `tabout`/`conout`/`conbrk` helpers and `glbcolumn[]`.

## EmuTOS source

**Primary location:** `bdos/console.c:279` — `long xconout(int ch)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x02` =
`xconout`, `stdio_typ = 0x81` (stdout), `wparms = 1` (`bdos/bdosmain.c:157`).
The `0x81` flag means when stdout is redirected to a file
(`run->p_uft[1] > 0`) the dispatcher writes the raw byte with `xwrite` and
returns `0` **without** entering `xconout` — see Edge case 4.

**Helpers called:**
- `bdos/console.c:292` — `tabout(h,ch)` — expand a tab to spaces, else `conout`.
- `bdos/console.c:263` — `conout(h,ch)` — `conbrk` + `Bconout` + column track.
- `bdos/console.c:206` — `conbrk(h)` — Ctrl-S/Q/X/C on pending input.
- BIOS `Bconout(2,…)` (0x03).

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM`.

**System variables / globals read:**
- `run->p_uft[1]` (`bdos/console.c:281`) — the stdout standard handle.
- `glbcolumn[2]` (`bdos/console.c:54`) — per-device column, for tab stops.

**System variables / globals written:**
- `glbcolumn[2]` (`bdos/console.c:267-272`).

**Build configuration affecting this function:** none directly (transitive VT52
escape handling in the BIOS console driver).

---

## Source extraction summary

`Cconout` writes one character to the console output with tab expansion.
`xconout` resolves stdout from `run->p_uft[1]` (default `H_Console` → BIOS
handle 2) and calls `tabout(h, ch)`. `tabout` special-cases the tab byte
(0x09): it emits spaces via `conout` until `glbcolumn[h] & 7 == 0` (i.e. to the
next multiple of 8); any other byte is passed straight to `conout`.

`conout` first calls `conbrk(h)` to act on any pending input control keys
(Ctrl-S pauses output until Ctrl-Q; Ctrl-C terminates the process), then writes
the byte via `Bconout` and updates the tracked column: `+1` for printable bytes
(`>= ' '`), reset to 0 on CR (0x0D), `-1` on BS (0x08), unchanged for other
control bytes.

`xconout` returns `1` unconditionally (`bdos/console.c:282`). This is the value
a program observes; documentation that claims "characters written" or
"undefined" is wrong for EmuTOS — see D-001.

Only the low 8 bits of `ch` are meaningful; `Bconout` takes a byte. The high
byte of the WORD argument is not inspected by `xconout`/`tabout`.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x02
- **Stack frame on entry (m68k):**
  ```
  +2: ch (word)
   0: function_id (word) = 0x0002
  ```
- **Total parameter bytes:** 4 → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x02` in `x8`; `ch` in `x0`; LONG result `1`
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `ch` | `WORD` (16) — low 8 bits used |

---

## Inputs

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| arg0     | `ch` | `WORD` | 2  | `bdos/console.c:279` | byte to output; low 8 bits |

Tab (0x09) triggers space expansion; CR/BS adjust the column counter; all other
bytes are written verbatim through `Bconout`.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** always `1` (`bdos/console.c:282`).
- **Source location:** `bdos/console.c:282`.

### Side effects on global state

- Advances/resets `glbcolumn[h]` per the byte written.
- May consume queued input via `conbrk` (Ctrl-S/Q/X/C handling), which can
  flush the typeahead buffer or terminate the process.

### Side effects on hardware

- Writes the byte(s) to the console/screen via BIOS `Bconout`.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | `1` always |

**Error paths not in Compendium:** none; the return is a constant `1`.

---

## Verification criteria

- **Nominal:** `Cconout('A')` writes `A` to the console and returns `1`.
- **Tab expansion:** with `glbcolumn == 0`, `Cconout('\t')` writes 8 spaces and
  leaves `glbcolumn == 8`; with `glbcolumn == 3` it writes 5 spaces to reach 8.
- **Column tracking:** `Cconout('\r')` resets the tracked column to 0;
  `Cconout('\b')` decrements it by 1.
- **Ctrl-S pause:** if a Ctrl-S is pending on input when `Cconout` runs, output
  blocks inside `conbrk` until Ctrl-Q arrives.
- **Ctrl-C terminate:** a pending Ctrl-C on input causes `conbrk` to terminate
  the process; `Cconout` does not return.
- **Redirected stdout:** with `run->p_uft[1]` forced to a file, the raw byte
  (tab **not** expanded) is written to the file and the call returns `0`
  (`bdos/bdosmain.c:521-528`).

---

## Edge cases discovered in source

1. **Return is `1`, not the byte or a count.** `bdos/console.c:282`. See D-001.

2. **Tab expands to the next multiple of 8, using the tracked column.**
   `tabout` loops `conout(h,' ')` while `glbcolumn[h] & 7` (`bdos/console.c:294-300`).
   The column counter — not the real cursor — drives this, so control bytes
   other than CR/BS (which leave the counter unchanged) can desynchronise it.

3. **Ctrl-S/Ctrl-C can act during *output*.** `conout` calls `conbrk` first, so
   writing a character is where a queued Ctrl-S pause or Ctrl-C terminate takes
   effect (`bdos/console.c:265`).

4. **Redirected stdout loses tab expansion.** The dispatcher writes the raw
   byte with `xwrite` (`bdos/bdosmain.c:527`) — a 0x09 goes to the file as a
   literal tab, not spaces — and returns `0`, not `1`. So the observable return
   value differs between the console path (`1`) and the redirected path (`0`).

5. **High byte of `ch` is ignored.** Only the low byte reaches `Bconout`.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console output `Bconout(2,…)`; `Bconstat`/`Bconin` for `conbrk`.
- `stdhdl_init()` (sets `p_uft[1] = H_Console`).
- `glbcolumn[2]`; `Pterm` via `terminate()` for the Ctrl-C path.

### Used by

- `Cconws` (0x09), `Cconrs` (0x0A) echo path, C-library `putchar`/`puts`/
  `printf`.

---

## Related data structures

- `docs/api/bios/03-bconout.md` — the underlying byte-output BIOS call.
- `docs/api/gemdos/_state.md §7` — `conterm` (not read here).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Return value documented
  inconsistently; EmuTOS returns `1` — D-001.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Tab expansion and column tracking are EmuTOS behaviour matching Atari TOS.
- The redirected-stdout `0` return is the MiNT/TOS filesystem path.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-02: `Cconout` returns `1` (D-001/D-011); an early implementation drift to
`0`/the byte was corrected — see D-011.
