# Phase 10 — BIOS Screen

> **Status:** pending. Display side of EmuTOS' `bios/screen.c`.

## Goal

Provide a framebuffer the way EmuTOS' `bios/screen.c` does:
`Logbase`/`Physbase`/`Setscreen`/`Vsetmode`/`VsetRGB`/etc., on top
of the platform display:

- QEMU: `virtio-gpu` (or `ramfb` for the simplest path).
- RPi4: VC4 mailbox property tag for framebuffer alloc, then HDMI.

## EmuTOS sources to read

- `bios/screen.c` — public API + the resolution table.
- `bios/videl.c`  — Falcon-style video controller (we don't emulate
  it, but the API surface is what programs expect).
- `bios/vt52.c`   — terminal-emulator escape sequences via Cconout
  once we have a framebuffer console (optional).

## Phase 00 prerequisites

- `docs/api/xbios/screen/*.md` (Phase 00 D5 XBIOS Batch 1).

## Deliverables

D1 Framebuffer allocator (mailbox/virtio).
D2 `Logbase`/`Physbase` (return current framebuffer).
D3 `Setscreen` (logbase, physbase, mode).
D4 Resolution-table compatibility shim (VGA-ish 640×480×4-plane).
D5 VT52-on-framebuffer text console behind Phase 02 `Cconout`
   (optional but useful).

## Exit criteria

EC1 `Cconout` writes glyphs that appear on the HDMI output / QEMU
   display window.
EC2 `Setscreen(-1, new_phys, -1)` flips to a user-supplied buffer.
