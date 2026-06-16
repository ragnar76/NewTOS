# PHASE-04: Block Storage HAL + FAT Read

## Goal

Implement an SD-card driver for RPi4 (SDHCI/EMMC2) and QEMU virt
(virtio-blk), expose them through a `BlockDevice` HAL trait, and add a
read-only FAT16/FAT32 driver that can list directories and read files from
the boot SD card.

## Inputs

- `CLAUDE.md`, Phases 01–03 complete
- BCM2711 datasheet, "EMMC2" section
- SD Specifications, Part 1 Physical Layer Simplified Specification
- Microsoft FAT specification (FAT: General Overview of On-Disk Format)
- `docs/api/gemdos.md` — no new entries required in this phase
  (filesystem syscalls land in Phase 05)
- EmuTOS source: `bdos/fsdrive.c`, `bdos/fsdir.c` (read-only paths only)

## Deliverables

### D1: `BlockDevice` HAL trait
Sector-addressed read/write interface. 512-byte sectors. Write API present
but optional in this phase (driver may return error).

### D2: BCM2711 EMMC2 driver
SD card initialization (CMD0/CMD8/ACMD41/CMD2/CMD3/CMD7), single-block read,
multi-block read. CRC verification per spec.

### D3: virtio-blk driver for QEMU virt
Minimal MMIO-based virtio-blk implementation. Read-only path is mandatory.

### D4: FAT16/FAT32 read-only driver
- BPB parsing
- Cluster chain traversal
- 8.3 names (LFN deferred to Phase 05 if Compendium permits)
- Directory listing
- File read

### D5: Mount point abstraction
Drives are accessed by drive letter (`A:`, `C:`, …) per TOS convention. SD
card root is `C:`. Document the mapping.

## Exit criteria

- **EC1**: A test reads a known file from the SD card and produces matching
  bytes.
- **EC2**: Directory listing produces the same entries as host `ls` on the
  same SD card image.
- **EC3**: Driver works on both QEMU virt (virtio-blk image) and real RPi4 SD.
- **EC4**: Reading a 1 MB file completes in under 5 seconds on RPi4.

## Out of scope

- Write path (Phase 05 if needed, otherwise later)
- exFAT, NTFS, ext-anything
- Multiple drives (single SD card is enough for phase exit)
- Long file names (LFN) — only if straightforward, otherwise Phase 05

## Risks

- **R1**: SD initialization timing is finicky. Follow the spec sequence
  exactly. Do not rely on Linux driver "shortcuts" without justification.
- **R2**: Cluster size variations across cards. Test with at least two
  different SD cards.
- **R3**: virtio queue setup on bare-metal differs from Linux's path. Use
  the virtio spec, not driver code, as the authority.

## Claude Code instructions

- The block layer is below GEMDOS. Do not introduce any TOS-API calls
  in this phase — it's all internal HAL.
- Resist the urge to add caching at the block layer; FAT-level caching
  comes in Phase 05.
- A read of sector 0 followed by parsing the partition table is **not** a
  stub — it's a real implementation. But if the SD has no partition table
  (unpartitioned FAT volume), handle that case explicitly, do not crash.
