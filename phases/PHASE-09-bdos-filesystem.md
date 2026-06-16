# Phase 09 — BDOS File System

> **Status:** pending. Depends on Phase 08.

## Goal

Implement the BDOS file system: GEMDOS file (`Fcreate`/`Fopen`/
`Fread`/`Fwrite`/…), directory (`Dgetpath`/`Dsetpath`/
`Dcreate`/`Ddelete`), and drive (`Dfree`/`Dgetdrv`/`Dsetdrv`)
calls, on top of a FAT12/16/32 driver layered on Phase 08.

## EmuTOS sources to read

- `bdos/fshand.c` — handle table.
- `bdos/fsdir.c` — directory traversal.
- `bdos/fsfat.c` — FAT structures + cluster chains.
- `bdos/fsglob.c` — glob/path parsing.
- `bdos/fsbuf.c` — sector cache.
- `bdos/fsdrive.c` — drive table.
- `bdos/fsmain.c` — init / mount.
- `bdos/fsio.c` — read/write helpers.

## Phase 00 prerequisites

- `docs/api/gemdos/file/*.md`
- `docs/api/gemdos/directory/*.md`
- `docs/api/gemdos/drive/*.md`
- `docs/api/gemdos/dta/*.md`

## Deliverables

D1 FAT BPB / FAT-table parser (read only).
D2 Sector cache (replaces `fsbuf.c`).
D3 Path parser + DTA structure.
D4 Read-side: `Fopen`/`Fread`/`Fseek`/`Fclose`/`Fattrib`.
D5 Directory: `Fsfirst`/`Fsnext`/`Dsetpath`/`Dgetpath`.
D6 Drive: `Dgetdrv`/`Dsetdrv`/`Dfree`.
D7 Write-side: `Fcreate`/`Fwrite`/`Fdelete`/`Frename`/`Fdatime`.
D8 `Mediach` invalidation hook.

## Exit criteria

EC1 `Fopen("c:/test.txt", FO_READ)` returns a valid handle for an
   existing file; `Fread` returns the bytes; `Fclose` succeeds.
EC2 `Fsfirst("c:\\*.*", 0)` enumerates the root directory.
EC3 `Fcreate` + `Fwrite` + `Fclose` produces a file readable from
   the host after unmount.

## Out of scope

- Long-file-name (VFAT) extensions — TOS has none, ports must use
  8.3 names. We add VFAT later if a port really needs it.
- Network file systems.
