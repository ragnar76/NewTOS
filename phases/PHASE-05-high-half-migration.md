# Phase 05 — High-Half Migration & Process ABI Foundation

> **Status:** ✅ Phase 05 closed (2026-05-07, QEMU virt verified).
> D1–D6 + D8 + EC4 complete. D4 (TTBR0 clear) was rolled into Phase 06
> as a per-process TTBR0 install + EPD0 clear sequence — see
> "Implementation notes" for the QEMU-vs-ARM-ARM-K11.5 reasoning.
> **D7 deferred** (no stub) until ADR-Supexec resolves the
> AArch64-substitute-for-m68k-supervisor-flip question; Phase 06 will
> demand a decision before its own D5/D6 land.

## Goal

Move the kernel from identity mapping in TTBR0 to a proper high-half
mapping in TTBR1, freeing TTBR0 for per-process user mappings.
Establish the ABI a Phase 06 process exec'd through `Pexec` will see:
register save/restore on trap entry, kernel/user stack split,
`Super`-mode bookkeeping.

This phase has no EmuTOS counterpart — EmuTOS runs everything in
m68k supervisor mode with one flat address space. AArch64 needs the
EL0/EL1 split and per-process page tables to host concurrent GEM
applications.

## EmuTOS sources to read

- `bdos/proc.c` — what `Pexec` expects to set up before calling the
  child: BASEPAGE-relative SP, `usp`/`ssp` split, return path.
- `bios/processor.S` — `regsav`, `Super`. Conceptual; the AArch64
  equivalents are register-banked between EL0 and EL1.
- `include/processor.h` — process / supervisor structures.

## Phase 00 prerequisites

- `docs/api/gemdos/process/26-super.md` (Super)
- `docs/api/structures/basepage.md`

## Deliverables

| Id | Title                                                       |
|----|-------------------------------------------------------------|
| D1 | Linker scripts: VA base in `0xFFFF_0000_0000_0000` half     |
| D2 | `boot.s` trampoline: enable MMU then absolute jump high     |
| D3 | TTBR1_EL1 setup, `TCR_EL1.EPD1=0`, `T1SZ=16`                |
| D4 | TTBR0_EL1 emptied (per-process mappings later)              |
| D5 | EL0 stack region carved out (low-half, per process)         |
| D6 | Trap entry: save x0..x30, sp_el0, elr_el1, spsr_el1         |
| D7 | `Super` (0x20): EL switch / supervisor-flag emulation       |
| D8 | Phase 02 + 03 + 04 selftests pass after migration           |

## Exit criteria

- **EC1**: Kernel runs at VAs above `0xFFFF_0000_0000_0000`,
  verified via `__kernel_image_start` symbol value at boot.
- **EC2**: TTBR0_EL1 = 0 after boot; setting it to a per-process
  table provides a different low-half view per CPU context.
- **EC3**: All Phase 02/03/04 selftests still green (Cconws,
  EINVFN, RX-IRQ, frame allocator, kernel heap, BDOS memory).
- **EC4**: Pseudo-process test: switch TTBR0 to a hand-built table
  containing one mapped page; an EL0 load from that page works,
  a load outside the mapping triggers a sync exception with
  correct ELR pointing at the offending instruction.

## Out of scope

- `Pexec`/`Pterm` (Phase 06).
- BASEPAGE construction (Phase 06).
- File-backed user images (Phase 06 + Phase 09).

## Risks

- **R1 — boot trampoline**: the absolute jump must be PC-relative
  before MMU enable, then absolute after. Wrong order = silent
  death. Mitigation: cite ARM ARM D5.2 examples line-by-line.
- **R2 — TTBR0 emptied too early**: pre-MMU code still runs in
  identity. Mitigation: TTBR1 takeover happens before TTBR0 is
  cleared; both valid for one window.
- **R3 — KASLR temptation**: skip. Not in scope. Record as future
  work if needed.

## Implementation notes

- This phase exists because identity mapping (Phase 03 path A) was
  chosen deliberately to keep Phase 03 small. The decision and its
  mandatory follow-up are documented in `docs/STATUS.md` (Recent
  Decisions, 2026-05-03; Blocked).
