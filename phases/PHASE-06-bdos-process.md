# Phase 06 — BDOS Process

> **Status:** pending. Depends on Phase 04 (memory) and Phase 05 (ABI).

## Goal

Implement the BDOS process layer: BASEPAGE, `Pexec` family, `Pterm`
family, `Pgetpid`, `Pwait`, signals, and the ownership-driven
memory cleanup (`free_all_owned`) that ties Phase 04's `m_own`
attribution to process lifetime.

## EmuTOS sources to read

- `bdos/proc.c` — `xexec` (`Pexec`), `xterm` (`Pterm`), `xtermres`
  (`Ptermres`), `xtermprocess`, `xprocess`, `xkeepm`,
  `free_all_owned`, basepage construction.
- `bdos/bdosmain.c` — `run`, `nextrun` globals; trap entry context.
- `include/biosdefs.h` — `BASEPAGE`.
- `bios/aciavecs.S` — exception save/restore conventions
  (already replaced by AArch64 path in Phase 05).

## Phase 00 prerequisites

- `docs/api/gemdos/process/00-pterm0.md`, `4c-pterm.md`,
  `26-super.md`, `4b-pexec.md`, `2b-pgetpid.md`, etc.
- `docs/api/structures/basepage.md`.

## Deliverables

| Id | Title                                                     |
|----|-----------------------------------------------------------|
| D1 | `BASEPAGE` Rust type matching EmuTOS layout               |
| D2 | Process table + `run`/`nextrun` analogues                 |
| D3 | `Pexec(PE_LOADGO)` (load + go) for ELF binaries (NewTOS)  |
| D4 | `Pexec(PE_LOAD)` (load only) returning basepage           |
| D5 | `Pexec(PE_GO)` (run pre-loaded basepage)                  |
| D6 | `Pterm0` / `Pterm` with `free_all_owned`                  |
| D7 | `Ptermres` (TSR-style terminate, keeps `keepcnt` resident)|
| D8 | Per-process page table install on context switch          |
| D9 | `m_own` wired to running process (Phase 04 D12 finalised) |

> **D7 correction (2026-05-07)**: original draft listed
> `Pgetpid` / `Pgetppid` / `Psetlimit` — those functions **do not
> exist in EmuTOS** (verified against `bdos/bdosmain.c` GEMDOS
> function table at commit `1a7484ab`). The classic GEMDOS process
> group has exactly four functions: `Pterm0` (0x00), `Ptermres`
> (0x31), `Pexec` (0x4B), `Pterm` (0x4C). D6 covered three of them;
> D7 picks up the remaining `Ptermres`. Sensible implementation
> blocks on D3 + D4 (BDOS-pool-allocated basepages — `Mshrink` is
> a no-op against the static `__PROCESS_USER_PAGES` arena that the
> Phase-06 D3 selftest uses).

## Exit criteria

- **EC1**: A trivial NewTOS-native ELF executes via `Pexec`,
  prints "hello" through `Cconws`, returns through `Pterm(0)`.
- **EC2**: After child `Pterm`, every MD with `m_own == child` has
  been freed; pool counts identical to before `Pexec`.
- **EC3**: `Pexec(PE_LOAD)` + `Mshrink` + `Pexec(PE_GO)` idiom
  (the standard shell pattern) works.
- **EC4**: Cooperative round-robin between two processes via
  voluntary `Pwait`/`Cconin`-block hand-off.

## Out of scope

- m68k binaries — removed from project scope.
- File-system-backed loader (uses RAM-resident test ELFs until
  Phase 09).
- Preemptive scheduling — cooperative only for now.

## Risks

- **R1 — ELF parser surface**: keep it minimal; only PT_LOAD
  segments, fixed entry point, no dynamic linking.
- **R2 — basepage layout drift**: any change vs. EmuTOS breaks
  source ports. Mitigation: byte-compatible struct, validated
  against `references/emutos/` headers.

## Implementation notes

- TOS used `.PRG` files (m68k a.out-like). NewTOS uses ELF
  AArch64. Source ports are recompiled, not retargeted.
