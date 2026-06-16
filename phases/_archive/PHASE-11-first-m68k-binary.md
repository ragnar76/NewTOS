# PHASE-11: First m68k Binary Runs

## Goal

Take a real, unmodified m68k TOS .PRG (originally compiled for Atari ST/TT)
and run it on NewTOS via the Phase 10 emulator. This is the symbolic milestone
where the project becomes useful as a TOS-software preservation platform.

## Inputs

- `CLAUDE.md`, Phase 10 complete
- A pool of candidate m68k PRGs, in increasing complexity:
  1. A custom hello-world built with vbcc (controlled environment)
  2. A small public-domain TOS utility (e.g., a directory listing tool)
  3. A simple GEMDOS-only program (e.g., a calculator)
  4. A program using XBIOS for screen modes (Phase 12 territory if it
     uses GEM, otherwise here)

## Deliverables

### D1: Test corpus
Curate `test/m68k-binaries/` with at least 5 PRGs spanning:
- Pure GEMDOS console
- GEMDOS file I/O
- BIOS character I/O
- XBIOS without GEM
- A program that uses Mxalloc with TT-RAM mode (to test the documented-and-
  ignored mode flag)

### D2: Per-binary verification
For each binary, document expected behavior, run on NewTOS, compare against
running on Hatari (cycle-accurate) or actual hardware where available.

### D3: Failure triage
For binaries that don't work, classify the failure:
- Missing TOS API function → spec entry + retroactive phase update
- Endianness/marshaling bug → fix in Phase 10's shim
- m68k core inaccuracy → file against RuSTari
- Hardware-direct access (legitimate) → document; will work only with
  Phase 12 GEM/Line-A, or never (if direct register access)

### D4: Documentation
Write `docs/COMPATIBILITY.md` listing what works, what doesn't, and why.
This is the user-facing "what TOS software runs?" answer.

## Exit criteria

- **EC1**: At least 3 of the 5 corpus binaries run correctly.
- **EC2**: Failure modes for the others are diagnosed and recorded with
  reproducible reproductions.
- **EC3**: `docs/COMPATIBILITY.md` is published.

## Out of scope

- Achieving 100% TOS software compatibility — explicitly not a goal
- Performance optimization beyond Phase 10 baseline
- GUI software (Phase 12)

## Risks

- **R1**: Frustration. Many TOS programs poke registers directly. They
  won't work without Phase 12 (or ever). Set expectations.
- **R2**: License clarity for test corpus. Use only public-domain or
  explicitly redistributable software in the repo. Document sources.

## Claude Code instructions

- Phase 11 is mostly testing and triage. It produces few code changes
  but lots of documentation and a few targeted bug fixes in earlier
  phases.
- When a TOS program fails, **do not** patch the program or invent
  workarounds. Either fix the kernel/emulator/spec correctly, or document
  the incompatibility. Modifying user programs to "make them work" is
  forbidden.
