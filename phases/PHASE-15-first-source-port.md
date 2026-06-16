# Phase 15 — First C Source Port

> **Status:** pending. Originally Phase 09 in the old plan; renumbered
> to sit at the end of the implementation roadmap (the system is
> usable before this; this phase proves the source-port story).

## Goal

Recompile a small TOS C program against NewTOS' headers and run it
end-to-end without source modification beyond the build system. This
is the canonical proof that "GEM applications run on NewTOS via
clean source port" — which is the whole project rationale.

## Candidate programs (to choose at phase open)

- A minimal `cat`-style tool (uses `Fopen`/`Fread`/`Cconout`).
- `dmesg`-style: read BIOS cookies, print system inventory.
- A small VDI demo (line/text). Picks at least Phase 12.
- A small AES app (Hello-window). Picks Phase 13.

## Deliverables

D1 A `tos-headers` crate exposing C-callable shims for
   `Cconws`/`Malloc`/etc.
D2 An AArch64-targeted C toolchain config (`gcc-aarch64-linux-gnu`
   + freestanding settings).
D3 Build script that produces a NewTOS ELF from the original C
   source.
D4 Boot path that `Pexec`s the produced ELF.

## Exit criteria

EC1 The unmodified C source from a TOS program (post-build-system
   adjustment only) compiles, links, runs, and produces the
   expected output on NewTOS.
EC2 The same source, compiled for an Atari with EmuTOS headers,
   produces matching output on EmuTOS-on-Hatari.
