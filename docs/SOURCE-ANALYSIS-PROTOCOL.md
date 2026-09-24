# Source Analysis Protocol — NewTOS

> **Mandatory reading before any analysis, specification, or implementation work.**
>
> NewTOS is built by extracting the TOS API specification from the EmuTOS
> source code. EmuTOS is our **primary source of truth** for what TOS does
> and how it does it. This protocol defines how that extraction works.

---

## Why EmuTOS is the primary source

EmuTOS is the only complete, executable, testable, fully-source-available
implementation of TOS. Every other source has limitations:

- **Atari Compendium** (Hollis): documentation, with omissions, ambiguities,
  occasional errors, and some "implementation-defined" gaps
- **Atari Profibuch** (Jankowski/Reschke): German, more detail in places,
  but still documentation, not implementation
- **Original Atari TOS**: closed source, only available as disassemblies
  of unclear legality and provenance
- **Period magazine articles, atari-forum posts**: secondhand, often wrong

EmuTOS, by contrast:

- Is the actual code that runs
- Encodes every behavior, including those Compendium leaves undefined
- Compiles, executes, can be tested
- Has 25+ years of bug fixes and compatibility refinements
- Is well-commented (mostly) and reasonably well-organized

Therefore: **the EmuTOS source code is the canonical specification of TOS
for NewTOS purposes.** Documentation sources serve only as cross-references
when EmuTOS code is unclear.

---

## The two analysis tracks

### Track A: TOS API extraction (from EmuTOS)

This is the bulk of the analysis work. Every GEMDOS, BIOS, XBIOS, AES,
and VDI function is extracted from EmuTOS source into our own
specification documents.

**Source hierarchy for Track A:**

1. **EmuTOS source code** — primary, authoritative
2. **EmuTOS internal documentation** (`doc/` directory, header comments,
   inline comments) — primary, authoritative as commentary on the code
2b. **FreeMiNT source code** (`references/freemint/`) — **authoritative
   for MiNT/GEMDOS extensions that EmuTOS does not implement** (e.g.
   `Ssystem`/0x154 + cookie subfunctions, later MiNT syscalls). EmuTOS
   stays primary for everything it covers; reach for FreeMiNT only when
   EmuTOS lacks the function, and extract from its source the same way
   (function numbers, subfunction codes, parameter ABI — never from
   memory). Adopted 2026-06-20 (ADR-012) to run the MiNT software base.
3. **Atari Compendium** — secondary, used to:
   - Cross-check function names, trap numbers, parameter ABIs
   - Resolve ambiguity when EmuTOS code is unclear
   - Identify behaviors that are deliberately incompatible with original TOS
4. **Atari Profibuch** — secondary, used like Compendium
5. **Atari ST Internals** — tertiary, used for system variable layouts and
   undocumented internals
6. **Hatari / ARAnyM behavior** — tertiary, used to validate edge cases
   when both EmuTOS and Compendium are silent

### Track B: Hardware (RPi4 / AArch64)

For hardware programming there is no equivalent "reference implementation"
to extract from. The datasheet is primary.

**Source hierarchy for Track B:**

1. **BCM2711 ARM Peripherals datasheet** — primary, authoritative
2. **ARM Architecture Reference Manual (ARMv8-A)** — primary for CPU/MMU/exceptions
3. **Raspberry Pi 4 boot documentation** — primary for boot sequence
4. **Andre Richter, `rust-raspberrypi-OS-tutorials`** — secondary, verified
   working patterns
5. **Linux kernel source** — tertiary, cross-reference only (not authority)
6. **U-Boot, circle, rpi-userland** — tertiary, peripheral-specific quirks

---

## The Extraction Workflow (Track A)

This is the workflow for analyzing EmuTOS and producing the NewTOS
specification. It runs end-to-end as **Phase 00** before any
implementation phase begins.

### Step 1: Map the EmuTOS source layout

Before any function-level work, produce a map of where things live in
the EmuTOS tree:

- `bdos/` — GEMDOS implementation (file system, processes, memory, console)
- `bios/` — BIOS and low-level device handling
- `vdi/` — Virtual Device Interface (graphics primitives)
- `aes/` — Application Environment Services (windows, menus, forms)
- `desk/` — TOS desktop application
- `cli/` — command-line interpreter
- `util/` — internal utilities
- `include/` — shared headers (data structures, type definitions)

Produce: `docs/emutos-map.md` documenting which directories and key files
contain what. This is the navigation index for everything that follows.

### Step 2: Identify all entry points

For each trap (1, 13, 14), find the dispatcher function in EmuTOS source
and extract:

- The function ID → handler function mapping (typically a table)
- The argument unpacking convention
- The return value packing convention
- Any pre/post-processing common to all functions in that trap

Produce: `docs/api/<trap>/_dispatcher.md` documenting the dispatch mechanism.

### Step 3: Per-function deep-read

For each function (Cconin, Fopen, Setscreen, …) the workflow is:

1. **Locate** the function in EmuTOS source (file:line)
2. **Read the full implementation** including all helpers it calls
3. **Identify all inputs** the function consumes:
   - Arguments (with types)
   - Global state (system variables, tables, current process state)
   - Hardware state (rare in EmuTOS itself, more common in BIOS layer)
4. **Identify all outputs** the function produces:
   - Return value (and how it's encoded)
   - Side effects on global state
   - Side effects on hardware
   - Error conditions and how they're signaled
5. **Trace error paths**: what causes each error code? What conditions
   produce undocumented results?
6. **Identify behavior the function depends on** from other functions
   (e.g., Fopen depends on the FAT subsystem state from earlier mounts)
7. **Note version-specific behavior**: EmuTOS sometimes has
   `#ifdef CONF_WITH_TT` or similar conditionals — document which
   variant we're extracting. For "is this compiled into our reference
   build" questions, cite `docs/api/_emutos-config.md` (regenerate via
   `tools/emutos-config.sh`), not a re-read of `config.h`'s `#ifdef
   TARGET_*` nesting by eye — see `TASK/REF01.md`.
8. **Cross-check with Compendium** for the documented ABI; if there's a
   discrepancy, EmuTOS wins, but we record the discrepancy
9. **Write the spec entry** in `docs/api/<trap>/<function>.md` (or grouped)

The spec entry is structured per the template in `docs/api/_template.md`.

### Step 4: Per-data-structure documentation

EmuTOS uses many TOS-defined data structures (DTA, Basepage, BPB, MD,
PD, OSHEADER, etc.). Each is documented separately:

1. Find the structure definition in `include/`
2. Document every field: offset, size, semantics, who writes it, who reads it
3. Note endianness explicitly (m68k = big-endian; relevant when data is on disk
   or passed across the m68k boundary)
4. Document layout-critical constraints (some fields are at fixed offsets
   that programs read directly)

Produce: `docs/api/structures/<name>.md`.

### Step 5: Per-subsystem state documentation

GEMDOS, BIOS, and XBIOS each maintain global state (open file table,
current drive, DTA pointer, exception vectors, etc.). For each subsystem:

1. Identify all global variables
2. Document who initializes them (boot sequence)
3. Document who reads / writes them at runtime
4. Document concurrency assumptions (TOS is mostly single-threaded; some
   things touched in IRQ context)

Produce: `docs/api/<subsystem>/_state.md`.

### Step 6: Cross-reference and consistency check

Once function-by-function extraction is complete:

1. Verify every trap dispatcher entry has a corresponding spec
2. Verify every documented behavior cites a specific EmuTOS source location
3. Verify every spec function lists its dependencies (other functions, state)
4. Build a dependency graph: which functions can be implemented standalone,
   which require which others first

Produce: `docs/api/_dependency-graph.md` — used by later phases to plan
implementation order.

### Step 7: Discrepancy log

Maintain a running log of discrepancies between EmuTOS and Compendium /
Profibuch. Each entry:

- The behavior in question
- What EmuTOS does (with source citation)
- What Compendium says (with page citation)
- What Profibuch says
- NewTOS decision (almost always: follow EmuTOS, document deviation)

Produce: `docs/api/_discrepancies.md`.

---

## Per-function spec entry template

Every function spec lives in `docs/api/<trap>/<id>-<name>.md`
(e.g., `docs/api/gemdos/01-cconin.md`). Format:

```markdown
# Cconin (GEMDOS 0x01)

## EmuTOS reference

- Primary: `bdos/console.c:bdos_1()` (or wherever)
- Helpers: `bios/iorec.c:rwconin()`, `bios/conout.c:cookie_lookup()`
- System variables read: `_iorec[]`, `conterm`
- System variables written: `_iorec[]` (queue head)

## Source extraction summary

(2–4 paragraphs in plain English describing what the function does, based
on reading the EmuTOS code. This is the human-readable understanding, not
yet the formal spec.)

## Trap interface

- Trap: 1 (GEMDOS)
- Function ID: 0x01
- Stack frame layout (m68k caller): word ID at top of stack

## Inputs

(formal list, extracted from code)

## Outputs

(formal list, extracted from code)

## Side effects

(what global state changes, what hardware state changes)

## Error conditions

(extracted from EmuTOS code, not from Compendium)

## Edge cases discovered in source

(things you found in EmuTOS that aren't documented in Compendium —
these are the most valuable findings)

## Dependencies

- Requires: (other functions / state)
- Used by: (which functions call this)

## Cross-references

- Compendium: page X — *(note any discrepancy)*
- Profibuch: section Y — *(note any discrepancy)*

## NewTOS implementation notes

(empty during Phase 00; filled in when the implementation phase starts)
```

---

## What Claude Code does in Phase 00

For each function:

1. Load the EmuTOS source file containing the implementation
2. Read it in full, including all called helpers (depth-first up to
   reasonable depth)
3. Read related header files for type definitions
4. Read related comments and `doc/` entries
5. Write the spec entry per the template, citing **every** source location
6. Cross-check with Compendium and Profibuch where available
7. Record discrepancies in `_discrepancies.md`
8. Add the function to the dependency graph

**Forbidden in Phase 00:**

- Writing implementation code
- "Inferring" behavior from function names without reading the source
- Skipping helpers ("they don't matter") — they often do
- Trusting comments over code (comments lie; code doesn't)
- Trusting Compendium over EmuTOS — EmuTOS is the spec for our purposes
- Producing a spec entry without an EmuTOS source citation (file:line)

**Acceptable in Phase 00:**

- Marking a function "needs deeper analysis" if a helper is genuinely
  too deep to follow inline; come back to it
- Flagging EmuTOS code that looks buggy or unclear for human review
- Grouping closely related functions into a single spec file (e.g., all
  the trivial Cprn* console functions together) for readability

---

## Track B (hardware) — how it differs

Hardware analysis is shorter because there's no source code to extract
from. The workflow is:

1. Read the BCM2711 datasheet section for the peripheral
2. Read the ARM ARM section for the CPU feature
3. Cross-check with Andre Richter's tutorial (does verified Rust code
   exist for this peripheral? if yes, study it)
4. Produce a register / behavior reference in `docs/hardware/<peripheral>.md`
5. Implement against the reference

Track B happens **per-implementation-phase**, not as a Phase 00. The HAL
for the UART is documented during Phase 01, the SD card during Phase 04,
etc. There's no benefit to documenting all RPi4 peripherals up front —
we only need each when we implement against it.

---

## Source acquisition

EmuTOS is fetched as a Git submodule into `references/emutos/`. The
specific commit hash is recorded in `docs/MEMORY.md` ADR-NNN so we have
a stable analysis target. (EmuTOS development continues; we pin to a
commit.)

Compendium and Profibuch are not redistributable; contributors acquire
them independently. `references/README.md` lists what's required.

A Phase 00 prerequisite check verifies:

- `references/emutos/` is present and at the pinned commit
- Compendium PDF exists at the documented path (optional but recommended)
- Profibuch is at the documented path (optional)

If EmuTOS is missing → Phase 00 cannot start. Hard stop.
If Compendium / Profibuch are missing → Phase 00 can start, with a noted
limitation that cross-checks for those two are deferred until they're
available.