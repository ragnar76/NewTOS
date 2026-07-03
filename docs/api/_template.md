# TOS API Specification — Entry Template

> Every TOS function gets a spec entry following this template.
> The spec entry is produced by **reading the EmuTOS source code**, per
> `docs/SOURCE-ANALYSIS-PROTOCOL.md`. Every claim in this document must
> trace to a specific EmuTOS source location.

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../COMPATIBILITY.md).
> The **Trap interface**, **Inputs**, **Outputs**, **Error conditions** and
> **Related data structures** sections are the **compatibility contract** —
> they must match TOS *exactly* (IDs, ABI, return/error codes, struct
> layouts, observable behaviour). The **EmuTOS source / Helpers / globals**
> and **Source extraction** sections are the **behaviour reference** — they
> cite EmuTOS internals so we understand *what* the call must do; NewTOS
> reimplements the *how* freely. **Do not treat an EmuTOS internal** (a
> helper like `ffit`, a private struct field, an algorithm, an internal
> table) **as contract** just because it appears in the reference sections.

---

# `<Function name>` (`<trap>` 0x<id>)

## EmuTOS source

> **Cite, never paste.** Reference EmuTOS as `path:line` or
> `file:function()`. Do **not** paste C (or asm) source bodies into a spec
> entry — the spec describes the contract in prose + structured fields, not
> the implementation. (Constant/flag/`#define` value tables and byte-exact
> struct layouts are fine; algorithm code is not.)

**Primary location:** `<path>:<line>` — the trap-level dispatched handler

**Installed handler / vector:** if the trap forwards through an indirect
vector (`hdv_bpb`/`hdv_rw`/`hdv_mediach`, `bconin_vec[]`, …), name the vector
**and** the function it points at (`file:function()`), which usually lives in
a different file than the trap entry. (`(none)` if the trap handles it
inline.)

**Helpers called:**
- `<path>:<line>` — `<helper_name>()` — purpose
- `<path>:<line>` — `<helper_name>()` — purpose

**Headers / type definitions used:**
- `include/<file>.h` — `<type_name>` definition
- `include/<file>.h` — `<constant_name>` definition

**System variables / globals read:**
- `<variable>` (`<path>:<line>`) — purpose

**System variables / globals written:**
- `<variable>` (`<path>:<line>`) — purpose

**Build configuration affecting this function:**
- `CONF_WITH_<feature>` — what changes when enabled/disabled
- (none, if the function is configuration-independent)

---

## Source extraction summary

A 2–4 paragraph plain-English description of what the function does,
written from reading the EmuTOS code. This is the human understanding;
the formal spec sections below extract this into structured form.

Example:
> `Cconin` reads a single character from the console input. EmuTOS
> implements this by calling `bconin(2)` after handling Ctrl-S/Ctrl-Q
> flow control via the `conterm` system variable. The character is
> echoed to console output if echo is enabled (bit 3 of `conterm`).
> The returned LONG packs the ASCII byte in bits 0–7 and the keyboard
> scancode in bits 16–23.

---

## Trap interface

- **Trap number:** 1 (GEMDOS) / 13 (BIOS) / 14 (XBIOS)
- **Function ID:** 0x<id>
- **Stack frame on entry (m68k):**
  ```
  +N: <param-N>
  …
  +2: <param-1>
   0: function_id (word)
  ```
- **Total parameter bytes:** N (for stack cleanup)
- **Native (AArch64) ABI:** how NewTOS receives the call — function ID in
  `x8`, arguments in `x0..x6` (in declaration order, one register each).
  Result in `x0`, per result shape:
  - **WORD / LONG** → sign-extended into `x0`.
  - **pointer** (e.g. `BPB *`) → occupies the full 64-bit `x0`, **not**
    sign-extended from 32 bits.
  - **`void`** → no return value; the result is delivered via an
    output-pointer argument (say which).

  Map each m68k stack parameter to its `x` register:

  | m68k stack | AArch64 reg | Name     | Type / size |
  |------------|-------------|----------|-------------|
  | +2         | `x0`        | `<name>` | `<type>`    |
  | +4         | `x1`        | `<name>` | `<type>`    |

### Parameter-block variant (VDI / AES)

The VDI and AES do **not** use the m68k stack frame + `x`-register form
above. They enter through the GEM trap `svc #2` with a **selector in `x0`**
(**115 = VDI**, **200 = AES**) and a **parameter-block pointer in `x1`**;
arguments and results travel in the block's WORD/pointer arrays, not in
registers. For those functions replace the two subsections above with:

- **VDI** — opcode in `CONTRL[0]`, sub-opcode in `CONTRL[5]`; inputs in
  `INTIN[]`/`PTSIN[]` (counts in `CONTRL[3]`/`CONTRL[1]`); outputs in
  `INTOUT[]`/`PTSOUT[]` (counts in `CONTRL[4]`/`CONTRL[2]`); `CONTRL[6]` =
  device handle. Give the slot→meaning table for the used slots.
- **AES** — opcode in `control[0]`; `int_in[]`/`addr_in[]` inputs,
  `int_out[]`/`addr_out[]` outputs (`int_out[0]` = the return code); counts
  in `control[1..4]`. Give the slot→meaning table.

---

## Inputs

Extracted from the EmuTOS implementation, not from documentation:

| Position | Name      | Type    | Size | Source location | Notes |
|----------|-----------|---------|------|-----------------|-------|
| arg0     | `<name>`  | `<type>`| 2/4  | `<file>:<line>` | …     |

For each input: where in the EmuTOS code it's read from the stack frame,
how it's validated (or not), what type the implementation treats it as
(may differ from documentation).

---

## Outputs

### Return value

- **Register:** D0 (m68k convention)
- **Type:** WORD / LONG / pointer
- **Encoding:** how the value is packed (e.g., "ASCII in bits 0–7,
  scancode in bits 16–23, zero elsewhere")
- **Source location:** where the return value is computed in EmuTOS

### Side effects on global state

What variables / tables the function modifies, with EmuTOS source
citations.

### Side effects on hardware

What hardware state changes (if any). Most GEMDOS functions touch no
hardware directly; BIOS/XBIOS functions often do.

---

## Error conditions

Extracted from EmuTOS code (every `return E_*;` and the conditions that
lead to it):

| Condition                              | Source location  | Returned value    |
|----------------------------------------|------------------|-------------------|
| (description from code, not docs)      | `<file>:<line>`  | `EFILNF` (-33)    |
| (next condition)                       | `<file>:<line>`  | `EACCDN` (-36)    |

**Error paths not in Compendium:** if EmuTOS has error returns that
Compendium doesn't document, list them here explicitly.

---

## Verification criteria

At least one concrete, testable pair per function — the measuring stick for
the implementation and the seed for its `#[cfg(test)]` cases. Derived from
the documented behaviour above, not invented:

- **Nominal:** given `<documented input>` (registers set to …) → returns
  `<documented output>` in `x0`, side effects `<…>`.
- **Error:** given `<documented error input>` → returns `<error code>`
  (e.g. `EFILNF` = -33) and leaves `<state>` unchanged.

List every distinct observable outcome that a conforming implementation
must reproduce (one row per behaviour).

---

## Edge cases discovered in source

Behaviors found in EmuTOS that are not in standard documentation. These
are the most valuable extraction outputs, because they're invisible
without reading the code.

Example:
> `Cconin` returns immediately with `0` if `conterm` bit 0 is set and
> no character is available, despite Compendium stating the function
> always blocks. (`bdos/console.c:147`)

---

## Dependencies

### Requires (must be initialized / functional)

- `<other function>` — why
- `<state variable>` — why

### Used by

- `<other function>` — how it calls this one

This populates the dependency graph in `docs/api/_dependency-graph.md`.

---

## Related data structures

References to structure spec files:

- `docs/api/structures/<name>.md` — used as input/output

---

## Cross-references

### Atari Compendium

- Page X: documented behavior
- **Discrepancy from EmuTOS:** (if any) — documented in
  `docs/api/_discrepancies.md` entry D-NNN

### Atari Profibuch

- Section Y: documented behavior
- **Discrepancy from EmuTOS:** (if any)

### Atari ST Internals

- (if relevant for system variable layout)

---

## Configuration / version notes

How this function differs across:

- TOS versions (1.x, 2.x, 3.x, 4.x) — based on EmuTOS conditional code
- EmuTOS build configurations
- Hardware variants (ST, STe, TT, Falcon)

---

## NewTOS implementation notes

(Empty during Phase 00. Filled in when the corresponding implementation
phase begins, recording NewTOS-specific decisions and deviations.)