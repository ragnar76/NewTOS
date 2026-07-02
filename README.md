# NewTOS

A clean-room, **TOS-API-compatible operating system for AArch64** — a native
bare-metal reimplementation of the Atari TOS API (GEMDOS / BIOS / XBIOS / VDI /
AES) that boots on the Raspberry Pi 4 (BCM2711) and under QEMU.

NewTOS is written in Rust (`#![no_std]`) and is **not** an emulator: the TOS
API is reimplemented natively on ARM64, aiming for bit-exact compatibility
(trap numbers, parameter ABI, error codes) so that C programs written against
the TOS headers can be recompiled and run.

> **Status:** work in progress (Phase 14 — GEM desktop). It boots, runs a GEM
> desktop on the HDMI framebuffer, reads FAT16 SD cards, drives USB keyboard +
> mouse on real hardware, and loads/runs `.PRG` programs. Expect rough edges.

## What works today

- **Boot** on a real Raspberry Pi 4 (HDMI framebuffer) and in QEMU
  (`raspi4b` / `aarch64-virt`).
- **GEMDOS / BIOS / XBIOS** — console I/O, the FAT16 file system (drives
  C:–F:), memory management (`Malloc`/`Mxalloc`), process loading (`Pexec`),
  the clock, and more.
- **VDI** — drawing primitives, fills, text, the mouse cursor sprite.
- **AES** — the full GEM object/event model: windows, menus (hover
  drop-downs), dialogs (`form_do` with radio buttons, checkboxes and editable
  fields), the file selector, alerts with icons, and eight mouse cursor forms.
- **GEM desktop** — menu bar, drive/trash/printer icons, directory windows
  (text + icon views, sorting, vertical scrolling), rename/preferences
  dialogs, and program launching.
- **USB HID** — keyboard and mouse via the VL805 xHCI controller behind the
  Pi 4's PCIe root complex (real hardware) and the QEMU xHCI model.
- **Keyboard layouts** — data-driven, TOS-conform tables; US and German
  built in, selectable via `NEWDESK.INF` (`#K de`) or a loadable
  `C:\KEYBOARD.KBD` file (see [`tools/mkkbd.py`](tools/mkkbd.py)).

See [`docs/STATUS.md`](docs/STATUS.md) for the detailed, dated project state.

## Building

Requires the pinned Rust nightly (installed automatically via
`rust-toolchain.toml`).

```sh
# Build the kernel image for the Raspberry Pi 4
cargo run -p xtask -- build --board rpi4 --release
# → kernel8.img  (copy to the boot partition of an SD card)

# Build for QEMU
cargo run -p xtask -- build --board qemu-virt --release
```

## Running

```sh
# Boot in QEMU (serial → stdout)
cargo run -p xtask -- run --board qemu-virt --release

# Boot in QEMU with the framebuffer window (raspi4b model)
cargo run -p xtask -- run --board rpi4 --release --window
```

On real hardware, copy `kernel8.img` (plus the Raspberry Pi firmware:
`start4.elf`, `fixup4.dat`, the device tree, and a `config.txt`) to a FAT
boot partition and boot the Pi 4.

## Repository layout

```
kernel/      bare-metal kernel core (boot, MMU, exceptions, drivers, desktop)
tos-api/     the TOS API layer (gemdos / bios / xbios / vdi / aes)
tos-types/   shared TOS types (DTA, BPB, error codes, …)
tools/       host-side tools (xtask build/run driver, mkkbd, …)
keyboards/   generated .KBD layout files (us, german)
docs/        architecture, requirements, the extracted TOS API spec, STATUS
phases/      phase-by-phase implementation guidelines
```

## Specification & references

The TOS API behaviour is specified by the **EmuTOS** source, analysed and
extracted into `docs/api/` during Phase 00; later phases implement against
that spec. Raspberry Pi hardware not covered by the BCM2711 datasheet (the
PCIe root complex, the VL805 xHCI) is cross-referenced against **Circle**.
Neither EmuTOS nor Circle source is redistributed here (see
[`references/README.md`](references/README.md)).

## License

NewTOS is licensed under the **GNU General Public License, version 2 or (at
your option) any later version** — see [`LICENSE`](LICENSE).

Some data tables and bitmaps (keyboard layouts, mouse cursor forms, alert
icons, dither patterns) are derived verbatim from EmuTOS (GPL v2+); see
[`NOTICE`](NOTICE) for the full attribution.
