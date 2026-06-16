# Phase 08 — BIOS Block Devices (Trap 13)

> **Status:** pending. Depends on Phase 06.

## Goal

Implement BIOS trap 13: `Bconin`/`Bconout`/`Bconstat`/`Bcostat`
(per-device variants), `Rwabs`, `Mediach`, `Getbpb`, `Drvmap`.
Provide an SD-card driver on RPi4 (EMMC2) and a virtio-blk driver
on QEMU.

## EmuTOS sources to read

- `bios/bios.c` — trap 13 dispatcher.
- `bios/blkdev.c` — block device abstraction, `Rwabs`, `Mediach`.
- `bios/disk.c` — drive A:/B:/C: routing.
- `bios/floppy.c` — floppy controller (skip; we have no floppy).
- `bios/mfp.c` — keyboard ACIA = our PL011 RX (already in Phase 02).
- `bios/ikbd.c` — IKBD; defer to Phase 10 / 11.

## Phase 00 prerequisites

- `docs/api/bios/*.md` (Phase 00 D2 BIOS group).

## Deliverables

D1 BIOS dispatcher on Trap 13 (analogous to Phase 02 GEMDOS).
D2 `Bconin`/`Bconout` per device id (PL011 = handle 1).
D3 Block-device trait + virtio-blk (QEMU) and EMMC2 (RPi4) impls.
D4 `Rwabs` / `Mediach` / `Getbpb` — sector-level I/O.
D5 BIOS-cookie-jar plumbing (`_MCH`, `_CPU`, `_FRB`, …).

## Exit criteria

EC1 `Rwabs(0, buf, 1, 0, 0)` reads sector 0 of drive A: into `buf`
   and returns `E_OK`.
EC2 BPB returned by `Getbpb` matches the on-disk FAT BPB.

## Out of scope

- File-system layer (Phase 09).
- Removable-media hot-plug (`Mediach` flags non-removable for now).
