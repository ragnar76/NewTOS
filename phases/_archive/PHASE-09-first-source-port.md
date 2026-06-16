# PHASE-09: Native Source Port — First Tool

## Goal

Take a small TOS C program (text editor, calculator, or shell-like tool)
whose sources compile against TOS headers, port it minimally to AArch64
NewTOS by recompiling, and verify it runs as a native ARM "PRG" via Phase
06's loader.

This is the first end-user-visible proof that the API is real.

## Inputs

- `CLAUDE.md`, Phases 01–08 complete
- A TOS source candidate. Suggestions in priority order:
  1. **micro-emacs** (uemacs/em variants)
  2. **stevie** (vi clone for Atari)
  3. A custom minimal `cat`/`type` clone written for the occasion
- TOS C compiler header equivalents (`<tos.h>`, `<gemdos.h>`) — write a
  thin Rust-side header set or a C compatibility crate

## Deliverables

### D1: AArch64 `tos.h` equivalent
A C header set that maps TOS function names to AArch64 syscall stubs
(`Cconout` → SVC #1 with x8=0x02, etc.). Either:
- (a) Plain C header + small assembly stubs, compiled with `aarch64-none-elf-gcc`
- (b) Rust crate exposing `extern "C"` versions of the TOS API

Decision: start with (a), keep (b) as future option.

### D2: Toolchain documentation
Document the cross-build toolchain in `docs/TOOLCHAIN.md`:
- `aarch64-none-elf-gcc` from ARM developer site
- Linker script for NewTOS PRG format
- `nm`, `readelf` invocations to verify output

### D3: Port the chosen tool
Port micro-emacs (or fallback). Acceptable changes:
- Replace direct hardware register access with TOS XBIOS calls
- Replace any non-TOS syscalls with errors or stubs
- Adjust integer width assumptions where the original was 16-bit

Forbidden changes:
- Anything that touches the program's logic
- Replacing TOS calls with NewTOS-specific ones

### D4: PRG image
Final output is a single `.prg`-format binary that the Phase 06 loader
accepts.

## Exit criteria

- **EC1**: The tool starts, displays its UI on the framebuffer, accepts
  keyboard input, and performs its core function.
- **EC2**: Saving and loading files works (via Phase 05 filesystem).
- **EC3**: The same source code, with no NewTOS-specific patches, also
  builds for actual TOS using Pure C or vbcc (cross-verify on Hatari).
- **EC4**: A second, smaller program (say, a `dir` clone) is also ported
  to demonstrate the toolchain isn't bespoke to one program.

## Out of scope

- Performance optimization
- Anything involving GEM/AES/VDI (Phase 12)
- Sound

## Risks

- **R1**: `int` width. Atari TOS C compilers used 16-bit `int` by default
  (Pure C, GCC with `-mshort`). Modern AArch64 compilers default to 32-bit.
  Code that does `int x = 32768;` will silently behave differently. Either
  use `-mshort` (if supported on aarch64-none-elf-gcc — likely not) or
  audit.
- **R2**: Stack size assumptions. TOS programs often assumed small stacks.
  Make sure the loader allocates enough.
- **R3**: The chosen tool may use ANSI escape sequences or VT52 codes for
  cursor positioning. NewTOS console must handle VT52 (the Atari standard)
  per Compendium. Verify Phase 02's console understands at least VT52
  basics — if not, add as a Phase 09 prerequisite.

## Claude Code instructions

- This phase is **integration**, not new kernel code. Time spent in the
  kernel during Phase 09 means something was wrong in earlier phases —
  resist the urge to "fix it quickly" without an ADR.
- Document every patch applied to the upstream tool in a single
  `patches/<toolname>.patch` file. Keep the patch minimal.
- If the tool requires a syscall NewTOS doesn't yet implement, the answer
  is **not** to add a stub. The answer is: stop, document the gap in
  `docs/STATUS.md`, and decide whether to add the syscall now (extending
  Phase 02/05/07/08 retroactively with an ADR) or pick a different tool.
