# PHASE-00: EmuTOS Source Analysis & API Extraction

## Goal

Read the entire EmuTOS source code and produce a complete, structured
specification of the TOS API that NewTOS will implement. The output of
this phase **is** the specification we build against — it replaces the
need for any other "what does TOS do" reference during implementation
phases.

This phase is the foundation for everything that follows. It produces
no code. It produces documentation: hundreds of spec entries describing
every GEMDOS, BIOS, XBIOS, AES, and VDI function as EmuTOS implements it.

## Why this is its own phase

Without a complete extracted API specification, every implementation
phase would have to do partial extraction inline, leading to:

- Inconsistent depth of analysis between functions
- Discovered cross-dependencies forcing rework of "completed" functions
- Drift between what we think TOS does and what EmuTOS proves it does
- Implementation decisions made on partial information

By doing the analysis once, completely, up front, every later phase
operates from the same authoritative reference.

## Inputs

- `CLAUDE.md`
- `docs/SOURCE-ANALYSIS-PROTOCOL.md` — **the workflow for this phase**
- EmuTOS source tree at `references/emutos/` (pinned commit, recorded in MEMORY.md)
- Atari Compendium (optional but recommended for cross-check)
- Atari Profibuch (optional, for German-side cross-check)
- Atari ST Internals (optional, for system variables)

## Deliverables

### D1: EmuTOS source layout map

`docs/emutos-map.md` — navigation index of the EmuTOS tree:

- Per directory: what subsystem lives here, what files are entry points,
  what files are utilities
- Per key file: top-level summary
- The dispatcher locations for each trap
- The build-time configuration flags that affect API surface (CONF_WITH_*
  symbols)

This is produced first because every later deliverable references it.

### D2: Trap dispatcher specifications

For each of trap 1 (GEMDOS), trap 13 (BIOS), trap 14 (XBIOS):

- `docs/api/<trap>/_dispatcher.md` documenting:
  - Where the dispatcher lives (file:line)
  - The function ID → handler mapping (full table)
  - The stack frame layout on entry
  - The argument unpacking pattern
  - The return value handling
  - Any common pre/post processing

### D3: GEMDOS function specifications (complete)

Every GEMDOS function (0x00 through whatever the highest implemented one
is in EmuTOS) gets its own spec entry per the template in
`docs/api/_template.md`.

Organized by group:

- `docs/api/gemdos/console/` — 0x01–0x0B, 0x10
- `docs/api/gemdos/process/` — 0x00, 0x4B, 0x4C, 0x31, 0x4D
- `docs/api/gemdos/memory/` — 0x44, 0x48, 0x49, 0x4A
- `docs/api/gemdos/file/` — 0x3C–0x46, 0x57
- `docs/api/gemdos/directory/` — 0x0E, 0x19, 0x1A, 0x2F, 0x36, 0x39–0x3B,
  0x47, 0x4E, 0x4F
- `docs/api/gemdos/datetime/` — 0x2A–0x2D
- `docs/api/gemdos/misc/` — everything else

Each entry includes the EmuTOS source citation, the extracted behavior,
data structures involved, error conditions found in code, and edge cases
not in Compendium.

### D4: BIOS function specifications (complete)

Every BIOS function (0x00 through highest implemented) per the same
template:

- `docs/api/bios/` with one file per function or function group

### D5: XBIOS function specifications (complete)

Every XBIOS function:

- `docs/api/xbios/` with one file per function or function group

Note: XBIOS has many functions tied to Atari hardware (MFP timers, ACSI,
Floppy, IKBD-direct, Sound). For each, document **what EmuTOS does**;
the question of whether NewTOS will implement it for RPi4 is deferred to
the relevant implementation phase.

### D6: Data structure specifications

Every TOS-defined data structure used by the API:

- `docs/api/structures/dta.md` — Disk Transfer Address
- `docs/api/structures/basepage.md` — Process basepage
- `docs/api/structures/bpb.md` — BIOS Parameter Block
- `docs/api/structures/md.md` — Memory Descriptor
- `docs/api/structures/pd.md` — Process Descriptor
- `docs/api/structures/osheader.md` — OS Header
- `docs/api/structures/iorec.md` — Input record
- … and any others discovered during D3–D5

Each documents: field offsets, sizes, semantics, who reads, who writes,
endianness considerations, layout-critical constraints.

### D7: Subsystem state specifications

For each subsystem, document the global state:

- `docs/api/gemdos/_state.md` — open file table, current drives, DTAs, ...
- `docs/api/bios/_state.md` — exception vectors, device tables, ...
- `docs/api/xbios/_state.md` — keyboard table, screen base, palette, ...

Each state variable: where it lives in EmuTOS source, who initializes it,
who reads, who writes, IRQ vs thread context.

### D8: AES specifications

(Larger than GEMDOS+BIOS+XBIOS combined. May warrant its own sub-phase
PHASE-00b if the volume becomes unmanageable in a single pass.)

`docs/api/aes/` covering the full AES function set:
- Application services (appl_*)
- Event services (evnt_*)
- Object services (objc_*)
- Form services (form_*)
- Menu services (menu_*)
- Window services (wind_*)
- Resource services (rsrc_*)
- Graphic services (graf_*)
- Scrap services (scrp_*)
- File selector (fsel_*)
- Shell services (shel_*)

### D9: VDI specifications

(Also large.)

`docs/api/vdi/` covering:
- Control / inquiry functions
- Drawing primitives (line, polyline, polygon, polymark, text, fill area)
- Attribute functions
- Raster operations
- Bezier curves (TOS 4 / GDOS extensions)
- Workstation management

### D10: Line-A specifications

`docs/api/linea/` — the direct-poke graphics primitives that real TOS
programs use for performance:
- Line-A variable layout (the "linea" structure)
- Line-A entry points and their semantics
- Function code mapping (Line-A 0–0xF)

### D11: Discrepancy log

`docs/api/_discrepancies.md` — every discrepancy found between EmuTOS
behavior and Compendium / Profibuch documentation. Format:

```markdown
## D-NNN: <function or behavior>

**EmuTOS:** (file:line) does X
**Compendium:** page Y, says Z
**Profibuch:** section W, says ...
**Decision:** NewTOS follows EmuTOS (or otherwise, with rationale)
```

### D12: Dependency graph

`docs/api/_dependency-graph.md` — for every function, what it depends on.
Used by later phases to plan implementation order. Format:

- Adjacency list or DAG visualization
- Identification of strongly-connected components (mutually dependent
  groups that must be implemented together)
- Identification of "leaf" functions that can be implemented standalone

### D13: Phase 00 completion report

`docs/api/_completion-report.md` — at the end of Phase 00:

- Total functions documented (by trap and subsystem)
- Total data structures documented
- Total state variables documented
- Total discrepancies recorded
- Functions deferred / skipped, with reasons
- Recommended sub-grouping for implementation phases (informs phase
  planning for 02, 05, 06, 07, 08, 12)

## Exit criteria

- **EC1**: Every function in EmuTOS's GEMDOS, BIOS, and XBIOS dispatch
  tables has a corresponding spec entry in `docs/api/<trap>/`.
- **EC2**: Every spec entry cites at least one specific EmuTOS source
  location (file:line).
- **EC3**: Every data structure used in the API has a spec entry.
- **EC4**: Discrepancy log is non-empty and reflects actual found
  differences (not zero — if zero, the cross-check wasn't done).
- **EC5**: Dependency graph identifies at least the obvious leaf functions
  (Cconout, Bconout) and the obvious deep dependencies (Pexec, GEM init).
- **EC6**: A second contributor (or fresh Claude Code session) can pick
  up the spec for any single function and understand what to implement
  without reading EmuTOS themselves.
- **EC7**: AES + VDI documented to the same standard, OR explicitly
  marked as Phase 00b deferred (with date and trigger for resumption).

## Out of scope

- Implementation code (that's Phase 01+)
- NewTOS-specific design decisions (deferred until implementation phase)
- Hardware abstraction (Phase 01+)
- m68k ABI marshaling design details (Phase 10 — but the m68k ABI for
  each function **is** documented here, since EmuTOS knows it)
- Performance analysis
- Optimization

## Risks

- **R1 — Scope explosion.** EmuTOS is large (>200k lines of C). A
  thorough analysis could easily take longer than implementing the kernel.
  Mitigation: rigorously time-box per function. Most functions take
  15-60 minutes of focused reading. If a function takes hours, it's a
  signal to flag it for human review, document what's clear, and move on.
- **R2 — Analysis paralysis.** The temptation to "understand everything
  before writing anything" can stall the project. Mitigation: ship D1–D7
  first (GEMDOS+BIOS+XBIOS). AES/VDI (D8/D9) can be deferred to PHASE-00b
  if needed without blocking PHASE-01.
- **R3 — EmuTOS bugs.** EmuTOS has bugs. Documenting them as "the spec"
  would propagate them. Mitigation: cross-check critical functions with
  Compendium/Profibuch; mark suspected bugs in the discrepancy log; raise
  with EmuTOS upstream if confirmed.
- **R4 — Configuration variants.** EmuTOS has CONF_WITH_TT, CONF_WITH_FALCON,
  CONF_WITH_VIDEL, etc. Different configurations expose different APIs.
  Mitigation: pick a target configuration up front (recommend
  CONF_WITH_TT) and document the choice in MEMORY.md ADR.
- **R5 — Comment quality.** EmuTOS comments are mostly good but
  occasionally misleading or outdated. Mitigation: the code is the source
  of truth, not the comments. Comments are flagged in the spec entry as
  "comment says X, code does Y" when they diverge.

## Claude Code instructions

### Working mode

This phase is sustained, methodical reading. The work pattern is:

1. Pick a function from a working list (start with leaf functions)
2. Open the EmuTOS source file containing it
3. Read the function in full, including helpers
4. Read header files for involved types
5. Read inline comments and any `doc/` cross-references
6. Cross-check with Compendium/Profibuch if available
7. Write the spec entry citing every source location
8. Update the dependency graph with newly identified dependencies
9. Update the discrepancy log if anything found
10. Move to the next function

### Working list management

A `docs/api/_working-list.md` file tracks:

- Functions completed (with date and commit hash)
- Functions in progress
- Functions blocked (and on what)
- Functions deferred (with reason)

Update this file at the start and end of each session so progress is
visible.

### What "complete" means for a function

A spec entry is complete when:

- Every input is documented with type and source
- Every output is documented
- Every error condition is traced to specific code paths
- Every dependency on other functions / state is listed
- The EmuTOS source citation is precise (file:line)
- Compendium/Profibuch entries (if available) are referenced and any
  discrepancy is logged

### Forbidden in Phase 00

- Writing implementation code (no exceptions)
- Spec entries without EmuTOS source citations
- Inferring behavior from function names — read the code
- "I'll come back to that helper later" without a tracked TODO
- Skipping the cross-check with documentation when it's available

### Recommended order

Within GEMDOS, work in this order to minimize rework:

1. The trivial console functions (Cconout, Cnecin, Crawio) — easy wins,
   exercise the workflow
2. Memory functions (Malloc, Mfree, Mshrink) — small surface, important deps
3. Date/time (Tgetdate, Tsetdate) — trivial, builds completion momentum
4. Process control (Pexec, Pterm) — deep, but central
5. File functions (Fopen, Fread, Fwrite, ...) — large group, share helpers
6. Directory functions (Dgetpath, Fsfirst, Fsnext, ...) — share state
   with file group

Within BIOS: similar pattern — trivial first (Bconout), complex last (Rwabs).

Within XBIOS: many are RPi4-irrelevant; document them anyway for completeness
but flag with "hardware not present on target" so implementation phases
know to either stub-with-error or skip.

### Time discipline

If a function consumes more than 90 minutes of analysis without producing
a complete spec entry: stop, document what's clear, mark the gaps,
move on. Return to it after surrounding context is built up — often the
same function is much easier after adjacent functions are understood.
