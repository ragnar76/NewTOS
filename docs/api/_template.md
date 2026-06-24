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

**Primary location:** `<path>:<line>` — the dispatched handler function

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