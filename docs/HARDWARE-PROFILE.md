# NewTOS Hardware Profile — the machine definition

> **What this is.** EmuTOS supports many Atari (and non-Atari) machines,
> selected at build time by `CONF_WITH_*` switches; every hardware-parametric
> value (screen geometry, palette, timer rate, memory map, cookies, system
> variables) is resolved per machine. NewTOS targets exactly **one hardware
> family — the Raspberry Pi 4 (BCM2711)** — with a second, QEMU-only test
> target. This document is NewTOS's equivalent of that machine config: the
> single place that pins every hardware-parametric answer the TOS-API spec in
> `docs/api/` leaves open.
>
> **Two-step plan.** (1) *This document + per-spec "Pi/BCM2711 binding"
> sections* — adapt the EmuTOS-derived contract to the real Pi hardware.
> (2) *later* — validate the code against the docs, subsystem by subsystem.
> The two are separate.

> **Verification status.** Values below are extracted from the NewTOS drivers
> and the BCM2711 peripherals datasheet. Entries marked **(TBV)** still need a
> code/datasheet cross-check in the binding pass; entries marked **(plan)** are
> designed but not yet implemented.

---

## 1. Target boards — the "switch"

The board is selected by a Cargo feature (exactly one must be set), the
NewTOS analogue of EmuTOS's `MACHINE_*`:

| Board feature      | Machine            | Role                          |
|--------------------|--------------------|-------------------------------|
| `board-rpi4`       | Raspberry Pi 4 / BCM2711 | Primary (real hardware) |
| `board-qemu-virt`  | QEMU `aarch64/virt`      | Debuggable test vehicle |

Everything below is the **`board-rpi4`** profile unless a `virt:` note gives
the QEMU difference. The two share the TOS-API layer and the VDI/AES; they
differ only in the peripheral drivers (`kernel/src/drivers/bcm2711/*` vs.
`kernel/src/drivers/qemu_virt/*`).

---

## 2. Peripheral map — subsystem → hardware → driver

| TOS subsystem        | Atari hardware (reference) | Pi-4 hardware (contract)          | NewTOS driver |
|----------------------|----------------------------|-----------------------------------|---------------|
| Screen / framebuffer | Shifter + video RAM        | VideoCore mailbox framebuffer     | `bcm2711/framebuffer.rs` · `bcm2711/mailbox.rs` (virt: `qemu_virt/ramfb.rs`) |
| Console / serial     | MFP 68901 / ACIA           | PL011 UART0                       | `bcm2711/pl011.rs` (virt: `qemu_virt/pl011.rs`), `hal/uart.rs` |
| Keyboard / mouse     | IKBD (6301) on ACIA        | USB-HID over xHCI/PCIe (+ IKBD-on-UART2 **(plan)**) | `usb/xhci`, `usb/hid.rs`, `ikbd.rs`, `keymap.rs` |
| Block device (drives)| WD1772 FDC / ACSI-SCSI HD  | EMMC2 SD-card controller          | `bcm2711/emmc.rs`, `hal/block.rs` |
| System timer         | MFP timer C (200 Hz)       | ARM generic timer / BCM system timer | `arch/aarch64/*` |
| Interrupt controller | MFP / GLUE                 | GIC-400 (GICv2)                   | `drivers/gic.rs` |
| DRAM                 | ST-RAM (+ TT-RAM board)    | LPDDR4 (uniform), split logically | `bios/machine/layout.rs`, `mm/*` |
| RTC / clock          | IKBD RTC / MC146818        | none on-board → seeded epoch **(TBV)** | `bios/…/time`, XBIOS `clock` |
| Sound                | YM2149 PSG                 | **absent** (no PSG) → Dosound no-op | — |
| Blitter              | BLiTTER                    | **absent** → Blitmode reports off | — |
| Printer / MIDI       | Centronics / MIDI ACIA     | **absent** → EUNDEV / no-op stubs | — |

---

## 3. Hardware-parametric values

### 3.1 Screen / framebuffer

- **Geometry:** 640 × 480, **32 bpp truecolor** physical framebuffer
  (`framebuffer.rs` `WIDTH=640, HEIGHT=480, DEPTH_BPP=32`). `virt:` same via
  `ramfb`.
- **VDI logical model:** the VDI presents a **16-colour, 4-plane** raster over
  the truecolor buffer (`RasterBackend` maps 16 PEL indices → RGB).
- **Screen base:** the mailbox-allocated framebuffer physical address; a single
  buffer (no separate physical/logical). `Physbase`/`Logbase` return it;
  `Setscreen` re-pointing is a no-op / bounded (single buffer). **(TBV)**
- **`Getrez`:** a fixed NewTOS resolution code for 640×480×16 — **decide the
  value** (ST high = 2 is monochrome; this is a 4-plane mode, so pick/publish a
  NewTOS code). **(TBV — the VDI DEV_TAB open question)**
- **Palette / colour:** 16 CLUT entries emulated over 32bpp; `Setcolor`/
  `Setpalette`/`vs_color` set the PEL→RGB map. VDI colour mask is the standard
  0..1000 per component.
- **VDI `DEV_TAB`/`INQ_TAB` answers (the framebuffer profile):**
  `DEV_TAB[0,1]` = 639, 479 (max x/y); `DEV_TAB[13]` = 16 (colours);
  `DEV_TAB[35]` = 1 (>1 plane); `INQ_TAB[4]` = 4 (planes); pixel size in
  microns (`DEV_TAB[3,4]`) — report truthfully from the HDMI mode or leave at
  the ROM default. **(TBV — pin these in the VDI workstation binding)**
- **`NUM_VDI_HANDLES` / `MAX_VERTICES`:** pick the NewTOS profile values (the
  EmuTOS default is 128 / 1024; the small profile is 64 / 512). **(TBV)**

### 3.2 Console / serial (PL011 UART)

- **Device:** PL011 UART0 (`bcm2711/pl011.rs`); the BIOS char device and the
  XBIOS `Rsconf` operate on it. Bit rate / line control per PL011 registers.
- **`Bconmap`:** single UART build → returns `0x2c` (unavailable), per the
  XBIOS spec. Handles: CON/AUX map to PL011; PRN/MIDI/IKBD are absent (stubs).
- **RX IRQ caveat:** on real Pi-4 the PL011 RX IRQ is not delivered through
  GIC-400 as in QEMU; console input uses a direct UART0 poll (see
  `docs/MEMORY.md`). `virt:` RX IRQ works.

### 3.3 Keyboard / mouse

- **Current:** USB-HID keyboard + mouse over the xHCI controller on the PCIe
  bus (`usb/xhci`, `usb/hid.rs`); HID scancodes are translated to Atari IKBD
  scancodes (`ikbd.rs`, `keymap.rs`), feeding the console key ring and the VDI
  mouse/AES event path.
- **Planned:** a native Atari ST keyboard on a second UART at 7812.5 baud
  (`docs/MEMORY.md`); the IKBD parser exists, UART2 pin wiring pending. **(plan)**
- **Keyboard layout:** built-in US + DE, plus a loadable `.KBD` override;
  `_AKP` cookie / `Bioskeys` reflect it.
- **XBIOS `Kbdvbase`/`Iorec`/`Initmous`:** the IKBD vector table + input record
  model is emulated over the HID/UART path (contract-level; the ISR internals
  are NewTOS-native — see the "adapter, not 1:1 port" ADR).

### 3.4 Block device / drives (EMMC SD)

- **Device:** EMMC2 SD-card controller (`bcm2711/emmc.rs`); `virt:` QEMU's
  raspi4b/SD (the SD driver is QEMU-debuggable). `Rwabs`/`Getbpb`/`Mediach`/
  `Drvmap` operate on it via `hal/block.rs`.
- **Card layout (real HW):** MBR + **4 × FAT16 partitions → C: D: E: F:**
  (verified on real Pi-4). `virt:` a raw FAT16 image built by
  `tools/make-sd-image.sh` (MBR, type 0x06) → C:.
- **Filesystem scope:** FAT16 hard-disk; FAT12/floppy out of scope; exFAT is a
  Phase-09 goal (`docs/MEMORY.md`). `Mediach` treats the card as non-removable.

### 3.5 System timer

- **Rate:** 200 Hz `_hz_200` system clock; `Tickcal` = **20 ms** per tick
  (50 Hz VBL-equivalent), per `tos-api/src/bios/mod.rs` `TICKCAL_MS=20`.
- **Source:** ARM generic timer / BCM system timer (`arch/aarch64`). **(TBV —
  confirm CNTFRQ vs the BCM system timer 1 MHz path)**

### 3.6 DRAM / memory map

- **Physical:** uniform LPDDR4 (rpi4) / QEMU DRAM (virt), but presented as **two
  additive logical pools** to preserve `Mxalloc` mode semantics
  (`bios/machine/layout.rs`):
  - **ST-RAM slot:** fixed **16 MiB** at the low end of DRAM (14 MiB usable;
    top 2 MiB reserved), matching the Atari ST address window.
  - **TT-RAM (alt-RAM) pool:** ~**256 MiB** above it (registered via `Maddalt`).
  - **Total managed:** ~272 MiB (not 256), both pools first-fit `Malloc`.
- **`Mxalloc` modes 0..3** select ST vs alt-RAM as on a real TT/Falcon; on
  AArch64 there is no DMA-zone distinction, so the split is a pure API contract
  (ADR-010; pending ADR-005 for uniform-RAM mode handling).
- **MPB / `Getmpb`:** the Memory Partition Block over the pools.

### 3.7 Cookie jar (the Pi machine identity)

Seeded by `tos_api::cookie::seed_defaults` (`kernel/src/lib.rs`):

| Cookie | Value        | Meaning |
|--------|--------------|---------|
| `NTOS` | `0x00000100` | NewTOS identity + version 0x100 |
| `_MCH` | `0x00050000` | machine id — a **NewTOS-specific, non-Atari** marker (not ST/STe/TT/Falcon) |
| `_IDT` | `0x0000112F` | date/time format: `/` separator, DD-MM-YY, 24-hour |
| `_AKP` | `0x00000000` | keyboard/language: US |

Atari cookies NewTOS deliberately does **not** advertise (no matching
hardware): `_VDO` (video shifter), `_SND` (PSG/DMA sound), `_FDC`, `_FLK`,
`_CPU`/`_FPU` (could be added — ARM), `_SWI`. **(TBV — decide `_CPU`/`_SND`.)**

### 3.8 System variables

The documented low-memory globals keep their **values/semantics** as the
contract (`_hz_200`, `_bootdev`, `_v_bas_ad` = the framebuffer base, `drvbits`
= the mounted-drive bitmap, the `hdv_*` disk vectors, `etv_*` exception
vectors, `_dumpflg`, etc.) but live in a kernel struct, not at the literal
m68k addresses — access is via the BIOS/XBIOS calls, not a direct poke (see
`docs/api/bios/_state.md`, `docs/api/xbios/_state.md`, `docs/api/gemdos/_state.md`).

### 3.9 Absent Atari hardware → NewTOS behaviour

| Atari feature | NewTOS |
|---------------|--------|
| YM2149 PSG / DMA sound | absent → `Giaccess`/`Dosound`/`Ongibit` no-op |
| BLiTTER | absent → `Blitmode` reports "no blitter" |
| WD1772 FDC / floppy | absent → floppy XBIOS = `EUNDEV`; FAT12 out of scope |
| MIDI ACIA | absent → `Midiws`/MIDI iorec no-op |
| Centronics printer | absent → `Cprnout`/printer XBIOS = no-op / busy |
| Shifter video modes | replaced by the single 640×480×16 framebuffer mode |
| MFP interrupts | replaced by GIC-400 + the software vector shadow (`Setexc`) |

---

## 4. CONF-equivalent feature matrix

Mapping the EmuTOS `CONF_WITH_*` switches (which the specs cite) to the fixed
NewTOS-Pi profile — the "is this present?" answer per switch:

| EmuTOS switch | NewTOS-Pi | Notes |
|---------------|-----------|-------|
| `CONF_WITH_ALT_RAM` / TT-RAM | **on** | the 256 MiB alt-RAM pool (§3.6) |
| `CONF_WITH_3D_OBJECTS` | **on** | 3D AES gadgets (user-confirmed look) |
| `CONF_WITH_COLOUR_ICONS` | (TBV) | G_CICON — 16-colour icons possible |
| `CONF_WITH_MENU_EXTENSION` | (TBV) | popups/submenus — bring-up-order |
| `CONF_WITH_GDOS` | **off** | no external fonts at bring-up |
| `CONF_WITH_VDI_16BIT` / Videl | **off** | 16-colour path only (though fb is 32bpp) |
| `CONF_WITH_SHUTDOWN` | (TBV) | real RPi-4 power-off = a design decision |
| `CONF_WITH_EXTENDED_MOUSE` | (TBV) | USB wheel/extra buttons available |
| PSG / Blitter / FDC / MIDI / printer | **off** | no hardware (§3.9) |

**(TBV)** rows are the concrete decisions the binding pass + Step-2 validation
will settle; each is also flagged as an open question in the relevant spec.

---

## 5. How the per-spec binding sections reference this

Each hardware-touching spec in `docs/api/` (the BIOS block/char/vector calls,
the XBIOS screen/input/serial/time calls, the VDI workstation/mouse/backend,
the GEMDOS time + memory-pool calls) gains a **"Pi / BCM2711 binding"**
subsection that: (a) names the peripheral + driver from §2, (b) gives the
concrete value from §3 (or defers to it), and (c) states what the Atari
reference maps to on the Pi. The spec's *contract* (ABI, return values,
error codes) is unchanged; only the *hardware reference* is re-pointed from
Atari silicon to the BCM2711.
