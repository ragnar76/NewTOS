# PHASE-05: GEMDOS Filesystem

## Goal

Implement the GEMDOS file I/O syscalls on top of the read-write FAT
driver. After this phase, a TOS source-port can `Fopen`/`Fread`/`Fwrite`/
`Fclose` files on the SD card.

## Inputs

- `CLAUDE.md`, Phases 01–04 complete
- `docs/api/gemdos.md` — entries for the entire `0x3C..0x4F` range plus
  `Fsfirst`/`Fsnext` must be drafted before code starts
- Atari Compendium, GEMDOS chapter, "File Functions" and "Directory Functions"
- EmuTOS: `bdos/fshand.c`, `bdos/fsmain.c`, `bdos/fsdir.c`

## Deliverables

### D1: FAT write path
Add write support to the FAT driver from Phase 04: cluster allocation,
FAT chain extension, dirty sector flushing.

### D2: GEMDOS file syscalls
- `Fcreate` (0x3C), `Fopen` (0x3D), `Fclose` (0x3E)
- `Fread` (0x3F), `Fwrite` (0x40)
- `Fdelete` (0x41)
- `Fseek` (0x42)
- `Fattrib` (0x43)
- `Fdatime` (0x57)
- `Fdup` (0x45), `Fforce` (0x46)

### D3: GEMDOS directory syscalls
- `Dgetdrv` (0x19), `Dsetdrv` (0x0E)
- `Dgetpath` (0x47), `Dsetpath` (0x3B)
- `Dcreate` (0x39), `Ddelete` (0x3A)
- `Dfree` (0x36)
- `Fsfirst` (0x4E), `Fsnext` (0x4F)
- `Fsetdta` (0x1A), `Fgetdta` (0x2F)

### D4: Per-process file handle table
Process-scoped FD table with the standard handles (stdin, stdout, stderr,
aux, prn) pre-mapped per Compendium. Note: requires Phase 06's process
infrastructure — coordinate ordering with Phase 06.

### D5: DTA (Disk Transfer Address) handling
Per-process current DTA pointer, default DTA, `Fsfirst`/`Fsnext` writing
through the DTA pointer per spec.

## Exit criteria

- **EC1**: Round-trip test: create a file, write 64 KB of pattern data,
  close, reopen, read back, verify byte-identical.
- **EC2**: `Fsfirst` + `Fsnext` enumerate a directory matching host `ls`.
- **EC3**: Pulling the SD card after `Fclose` shows the file present and
  correctly readable on a host PC.
- **EC4**: Fault injection (write to a full disk) returns `ENOSPC`-equivalent
  TOS error code, does not crash.
- **EC5**: All `docs/api/gemdos.md` entries for this group are complete.

## Out of scope

- LFN (long file names) — keep 8.3 unless trivial
- File locking
- Network filesystems
- Symbolic links (TOS doesn't have them)
- FAT12 (not used on SD cards in this size class)

## Risks

- **R1**: Write ordering and crash safety. A power cut mid-write may
  corrupt the FAT. Acceptable risk for Phase 05; record an ADR for future
  journaling consideration.
- **R2**: TOS path semantics. Drive letter, colon, backslashes, no leading
  slash means relative. Different from POSIX. Read Compendium carefully.
- **R3**: Endianness. FAT structures are little-endian on disk and AArch64
  is little-endian, so no swap needed — but for m68k callers (Phase 10)
  parameters in memory may be big-endian. The TOS-API layer handles this
  at the boundary.
- **R4**: Per-process FD table requires Phase 06. If Phase 06 isn't ready,
  use a single global table as a placeholder **with a clearly marked TODO
  and an ADR**. This is a documented limitation, not a stub.

## Claude Code instructions

- File I/O is the largest API group. Implement in batches; do not try to
  land all syscalls in one PR.
- The temptation to "improve on TOS" (e.g., add `O_APPEND`-style flags) is
  forbidden. We implement the spec, nothing more.
- Test against an SD card image, not just QEMU virtio-blk, before claiming
  exit criteria.
