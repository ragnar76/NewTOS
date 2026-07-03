# `Cconws` (GEMDOS 0x09)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:459` (`xconws`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, NUL-terminated output with tab expansion, the Ctrl-S
> pause / Ctrl-C terminate. The return value is a **deliberate NewTOS
> divergence** — see D-009. Reference: `prt_line`/`tabout`/`conout`.

## EmuTOS source

**Primary location:** `bdos/console.c:459` — `void xconws(char *p)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x09` =
`xconws`, `stdio_typ = 0x81` (stdout), `wparms = 2` (one LONG pointer)
(`bdos/bdosmain.c:164`). When stdout is redirected (`run->p_uft[1] > 0`) the
dispatcher writes the whole string with `xwrite(h, strlen(p), p)` and returns
`0`, **without** tab expansion — see Edge case 3.

**Helpers called:**
- `bdos/console.c:468` — `prt_line(h,p)` — loop `tabout` over the string.
- `bdos/console.c:292` — `tabout(h,ch)` — tab expansion, else `conout`.
- `bdos/console.c:263` — `conout(h,ch)` — `conbrk` + `Bconout` + column track.

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM(h) = 3+h`.

**System variables / globals read:**
- `run->p_uft[1]` (`bdos/console.c:461`) — stdout device.
- `glbcolumn[2]` (for tab stops).

**System variables / globals written:**
- `glbcolumn[2]` (via `conout`).

**Build configuration affecting this function:** none directly.

---

## Source extraction summary

`Cconws` writes a NUL-terminated string to the console output with tab
expansion. `xconws` resolves stdout from `run->p_uft[1]` (default `H_Console` →
BIOS handle 2) and calls `prt_line(h, p)`, which loops calling `tabout` for each
byte until it reaches the terminating NUL. Each byte goes through the same
cooked path as `Cconout`: tabs expand to the next multiple-of-8 column, `conbrk`
runs before every byte (so Ctrl-S pauses and Ctrl-C terminates mid-string), and
`glbcolumn` is tracked. There is no length limit — the loop trusts the caller's
NUL terminator.

The C signature is `void`. The trap-1 dispatcher returns whatever the compiler
left in D0, so the EmuTOS return value is **undefined ABI scratch**. NewTOS
deliberately diverges here and returns the byte count actually written, matching
what Compendium/Profibuch document and enabling a cheap self-test — see D-009.
The redirected-stdout path returns `0` (`bdos/bdosmain.c:530-533`).

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x09
- **Stack frame on entry (m68k):**
  ```
  +2: p (LONG, pointer)
   0: function_id (word) = 0x0009
  ```
- **Total parameter bytes:** 6 (function word + LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x09` in `x8`; `p` in `x0` (full 64-bit
  pointer, not sign-extended); result in `x0` (see return note).

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `p`  | `char *` (ptr) — NUL-terminated |

---

## Inputs

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| arg0     | `p`  | `char *` | 4 | `bdos/console.c:459` | NUL-terminated string; no length limit |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** EmuTOS: **undefined** (`xconws` is `void`; D0 is ABI scratch).
  NewTOS: the number of characters written (deliberate divergence, D-009).
- **Source location:** `bdos/console.c:459` (void); NewTOS decision in D-009.

### Side effects on global state

- Advances/resets `glbcolumn[h]` per byte.
- May consume input and terminate via `conbrk` (Ctrl-S/Q/X/C).

### Side effects on hardware

- Writes the (tab-expanded) bytes to the console via BIOS `Bconout`.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

**Error paths not in Compendium:** none. A missing NUL runs off the end of the
buffer (no validation).

---

## Verification criteria

- **Nominal:** `Cconws("AB")` writes `AB` to the console; NewTOS returns `2`.
- **Tab expansion:** `Cconws("\tX")` from column 0 writes 8 spaces then `X`.
- **Empty string:** `Cconws("")` writes nothing; NewTOS returns `0`.
- **Ctrl-C mid-string:** a pending Ctrl-C causes `conbrk` to terminate the
  process partway through the string.
- **Redirected stdout:** with `run->p_uft[1]` forced to a file, `strlen(p)`
  bytes are written to the file **without** tab expansion; the call returns `0`.

---

## Edge cases discovered in source

1. **EmuTOS return is undefined; NewTOS returns the count.** `xconws` is `void`
   (`bdos/console.c:459`); NewTOS returns the byte count — deliberate divergence
   (D-009).

2. **No length validation.** The loop runs until it finds a NUL
   (`bdos/console.c:470`); a missing terminator faults / runs wild.

3. **Redirected stdout loses tab expansion.** The dispatcher `xwrite`s the raw
   string via `strlen` (`bdos/bdosmain.c:530-533`) — a 0x09 goes to the file as
   a literal tab.

4. **Ctrl-S/Ctrl-C act per byte.** Because every byte goes through `conout` →
   `conbrk`, a Ctrl-S pauses and a Ctrl-C terminates mid-string.

---

## Dependencies

### Requires (must be initialized / functional)

- The full `Cconout` dependency set: `Bconout(2,…)`, `Bconstat`/`Bconin`
  (for `conbrk`), `glbcolumn[2]`, `Pterm` via `terminate()`.
- `stdhdl_init()` (sets `p_uft[1] = H_Console`).

### Used by

- Most C programs (`puts`-equivalent), the EmuCON shell, AES status lines.

---

## Related data structures

- `docs/api/bios/03-bconout.md` — the underlying byte-output BIOS call.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the return as
  "number of characters output"; EmuTOS source leaves it undefined, NewTOS
  chooses the count — D-009.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Return value is the one intentional NewTOS deviation for this group (D-009);
  everything else is EmuTOS behaviour.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-02: `Cconws` returns the written byte count by design (D-009), asserted by
the Phase-02 self-test.
