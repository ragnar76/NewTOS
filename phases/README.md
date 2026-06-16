# Phases – NewTOS Implementation Roadmap

NewTOS is a clean-room, native AArch64 reimplementation of TOS that
follows the **EmuTOS module layout**. Every implementation phase maps
to a concrete EmuTOS subsystem, so the source-of-truth chain is

```
EmuTOS source  →  docs/api/<group>/<id>-<name>.md  (Phase 00 spec)
                                 →  phases/PHASE-XX-…  (this directory)
                                                →  Rust implementation
```

## Phase index

| #  | Title                                  | EmuTOS source                                | Status        |
|----|----------------------------------------|----------------------------------------------|---------------|
| 00 | EmuTOS Source Analysis & Extraction    | (all)                                        | done          |
| 01 | Boot + Early Console                   | `bios/startup.S`, `bios/conout.c`, `bios/kprint.c` | done    |
| 02 | GEMDOS Console (Trap 1, console group) | `bdos/console.c`                             | 12/15         |
| 03 | BIOS HW Layer                          | `bios/machine.c`, `bios/kprint.c` + AArch64 MMU/cache | in progress (D1–D5 done) |
| 04 | BDOS Memory Layer                      | `bdos/mem.c`, `bdos/umem.c`, `bdos/iumem.c`  | pending       |
| 05 | High-Half Migration & Process ABI      | (AArch64-specific)                            | pending       |
| 06 | BDOS Process                           | `bdos/proc.c`, `bdos/bdosmain.c`             | pending       |
| 07 | BDOS Misc                              | `bdos/time.c`, RX completion in `bdos/console.c` | pending   |
| 08 | BIOS Block Devices (Trap 13)           | `bios/blkdev.c`, `bios/disk.c`, `bios/floppy.c`, `bios/mfp.c` | pending |
| 09 | BDOS File System                       | `bdos/fshand.c`, `bdos/fsdir.c`, `bdos/fsfat.c`, `bdos/fsglob.c`, `bdos/fsbuf.c`, `bdos/fsdrive.c` | pending |
| 10 | BIOS Screen                            | `bios/screen.c`, `bios/videl.c`              | pending       |
| 11 | XBIOS (Trap 14)                        | `xbios/*`                                    | pending       |
| 12 | VDI (Trap 2 sub-protocol)              | `vdi/*`                                      | pending       |
| 13 | AES (Trap 2 sub-protocol)              | `aes/*`                                      | pending       |
| 14 | Desktop                                | `desk/*`                                     | pending       |
| 15 | First C Source Port                    | (TOS-source-compatible test program)         | pending       |

> **Removed from scope:** m68k user-mode emulator and m68k binary
> support. NewTOS is a host for natively-compiled, source-ported GEM
> programs. m68k emulation, if ever pursued, becomes a follow-on
> project that consumes NewTOS as a host. The historical Phase 10/11
> docs are preserved under `phases/_archive/`.

## Module mapping (EmuTOS → Rust workspace)

The Rust workspace mirrors the EmuTOS layer split inside the
`tos-api/` crate (which is the BDOS / XBIOS / VDI / AES façade). The
`kernel/` crate keeps Rust-idiomatic names, with each module pointing
at its EmuTOS counterpart in module-level docs.

```
EmuTOS                        NewTOS
────────────────────────────  ────────────────────────────────
bios/startup.S, processor.S   kernel/src/arch/aarch64/boot.s
bios/machine.c                kernel/src/bios/machine/      (TBD: refactor)
bios/conout.c, kprint.c       kernel/src/console.rs (early), kernel/src/bios/kprint.rs (TBD)
bios/screen.c, videl.c        kernel/src/bios/screen/       (Phase 10)
bios/blkdev.c, disk.c         kernel/src/bios/blkdev/       (Phase 08)
bios/mfp.c (timer, IKBD)      kernel/src/drivers/{gic,timer,ikbd}.rs

bdos/console.c                tos-api/src/bdos/console/     (Phase 02)
bdos/mem.c, umem.c, iumem.c   tos-api/src/bdos/mem/         (Phase 04)
bdos/proc.c, bdosmain.c       tos-api/src/bdos/proc/        (Phase 06)
bdos/time.c                   tos-api/src/bdos/time/        (Phase 07)
bdos/fs*.c                    tos-api/src/bdos/fs/          (Phase 09)

xbios/*                       tos-api/src/xbios/            (Phase 11)
vdi/*                         tos-api/src/vdi/              (Phase 12)
aes/*                         tos-api/src/aes/              (Phase 13)
desk/*                        userland crate `desk/`        (Phase 14)
```

The first refactor (moving `kernel/src/mm/{layout,frames,heap}.rs` →
`kernel/src/bios/machine/` and laying out the new `tos-api/src/bdos/`
tree) is tracked in Phase 03 D9 and Phase 04 D0 respectively.

## Phase rules

- Phases are sequential within their dependency chain. Out-of-phase
  work requires user approval.
- Each phase document lists **deliverables (D1, D2, …)** and **exit
  criteria (EC)**.
- A phase is complete only when every deliverable is merged AND every
  exit criterion is observably met.
- Status updates go to `docs/STATUS.md` with date and commit hash.
- **Spec before code.** A TOS-API function is implemented only after
  its `docs/api/<group>/<id>-<name>.md` entry exists (Phase 00 rule;
  see CLAUDE.md §1).

## Phase document template

Every `PHASE-XX-*.md` follows this structure:

1. **Goal** — one sentence.
2. **EmuTOS sources** — exact files (and functions) to read first.
3. **Phase 00 prerequisites** — which `docs/api/…` specs must exist.
4. **Deliverables (D1, D2, …)** — numbered, each producing one artifact.
5. **Exit criteria (EC)** — observable, testable.
6. **Out of scope** — to prevent scope creep.
7. **Risks** — known traps, gotchas, hardware quirks.
8. **Implementation notes** — task-specific overrides of CLAUDE.md.
