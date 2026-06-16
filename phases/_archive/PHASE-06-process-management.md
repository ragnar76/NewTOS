# PHASE-06: Process Management

## Goal

Implement TOS-style processes: process control blocks, `Pexec` (load + go
variants), basic process address spaces, and a cooperative scheduler
(TOS is single-tasking; Phase 12 may add MultiTOS-style cooperation).

## Inputs

- `CLAUDE.md`, Phases 01–05 complete
- `docs/api/gemdos.md` — entries for `Pexec` (0x4B), `Pterm` (0x4C),
  `Pterm0` (0x00), `Ptermres` (0x31), `Cconws` already existing — verify
  process-context usage
- Atari Compendium, GEMDOS chapter, "Process Control"
- TOS PRG/TOS file format documentation (TPA layout, BSS, GEMDOS header)
- EmuTOS: `bdos/proc.c`, `bdos/pgmldr.c`

## Deliverables

### D1: Process Control Block (PCB)
Owned per-process state: PID, parent PID, base page (BP), TPA range,
file handle table (from Phase 05 D4), current DTA, current path per drive,
exit code, basepage pointer.

### D2: Address space setup
Each process gets a TTBR0_EL1 page table mapping its TPA. Kernel space
(TTBR1_EL1) shared. Switch on context change.

### D3: TOS executable loader
Parse the TOS PRG header (28 bytes), allocate TPA, copy text/data, zero
BSS, apply relocations, build basepage per Compendium.

### D4: `Pexec` modes
- Mode 0: Load and go
- Mode 3: Load only (return basepage)
- Mode 4: Just go (re-execute previously loaded basepage)
- Mode 5: Create basepage only
- Mode 6: Load, return, do not execute (parent gets pointer)

### D5: Native ARM "PRG" format
TOS PRGs are m68k. Native ARM source ports need an executable format.
Decision: extend the PRG header with a magic marker for ARM (`'NTOS'` instead
of `0x601A`), keep basepage layout identical. ELF as alternative
deliberately rejected (too heavy, too "Linux-flavored"). Document in ADR.

### D6: Cooperative single-task scheduler
For Phase 06, only one user process runs at a time. `Pexec(0, ...)` blocks
the parent until the child terminates. This matches TOS 1.x behavior.
Preemption / multitasking deferred.

### D7: Process termination
`Pterm`, `Pterm0`, `Ptermres` per spec. Free TPA, close handles, return
control to parent.

## Exit criteria

- **EC1**: A native ARM "PRG" can be loaded from SD via `Pexec`, runs,
  calls `Cconws`, returns via `Pterm0`, parent observes correct exit code.
- **EC2**: A process that allocates 1 MB via `Malloc`, runs, terminates —
  the kernel reclaims the memory (verified via `heap_stats`).
- **EC3**: Crashing the user process (deliberate `svc #0xFF`) does not
  crash the kernel; control returns to the parent with an error code.
- **EC4**: `Pexec` mode 3 returns a basepage that can subsequently be used
  by mode 4 or 6.
- **EC5**: Spec entries for all `P*` functions complete.

## Out of scope

- Preemptive multitasking (later, possibly never — TOS is cooperative)
- IPC primitives (TOS doesn't have them; deferred unless GEM needs them)
- Signal-like mechanisms beyond Ctrl-C (already in Phase 02)
- m68k binary loading (Phase 10–11)

## Risks

- **R1**: TOS basepage layout is bit-precise. Programs read fields by
  offset. Get it wrong and nothing works. Compendium has the table.
- **R2**: Address space switch on AArch64 requires TLB invalidation
  (`tlbi`) and ISB. Forgetting causes random data corruption.
- **R3**: PRG relocation table format is m68k-style. For native ARM
  binaries we define our own (ADR required).
- **R4**: Per-process CWD per drive. TOS tracks the current directory
  separately for each drive letter. Don't merge into a single CWD.
- **R5**: User stack location. Allocate within TPA per Compendium; do not
  hand the user a kernel stack.

## Claude Code instructions

- Resist Linux-isms. NewTOS is not Unix. There are no fork-exec semantics,
  no signals, no PIDs in the POSIX sense.
- TOS PRG format must be implemented to the byte. Test with a known-good
  minimal TOS .PRG built with vbcc or PureC if available, even though we
  can't execute m68k code yet — at least verify the loader reads the header
  correctly.
- The ADR for the native ARM executable format is critical. Get user
  approval **before** implementing the format. Suboptimal choices here
  lock in pain for years.
