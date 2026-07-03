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
| Console (CON)        | screen + IKBD              | framebuffer VT52 (out) + USB-HID (in) | `fbcon.rs`, `usb/hid.rs` |
| Serial (AUX) / debug | MFP 68901 / ACIA           | PL011 UART0 — **debug channel only** | `bcm2711/pl011.rs` (virt: `qemu_virt/pl011.rs`), `hal/uart.rs` |
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

- **Geometry (current):** 640 × 480, **32 bpp truecolor** physical framebuffer
  (`framebuffer.rs` `WIDTH=640, HEIGHT=480, DEPTH_BPP=32`). `virt:` same via
  `ramfb`.
- **Geometry (goal):** full native Pi resolution — up to **1080p** and beyond
  — is the target, **later**. It requires querying the HDMI mode at boot and
  making the VDI/AES geometry-independent (currently hard-coded 640×480). For
  now everything below assumes the fixed 640×480 mode; the higher-resolution
  work is a separate phase.
- **VDI logical model:** the VDI presents a **16-colour, 4-plane** raster over
  the truecolor buffer (`RasterBackend` maps 16 PEL indices → RGB).
- **Screen base:** the mailbox-allocated framebuffer, CPU-visible high-half VA
  of the pixel store (the bus address is masked to an ARM physical address and
  offset into the high half; `framebuffer.rs:178-179`,
  `vdi_backend.rs:405-407`). A **single buffer** (no separate physical/
  logical): `Physbase`/`Logbase` both return it. `Setscreen` re-pointing writes
  only the log/phys **shadows** and does **not** move the VideoCore scan-out
  (`xbios/screen.rs:72-77`) — a bounded no-op until double-buffering lands with
  the 1080p work.
- **`Getrez`:** returns **`2` (`ST_HIGH`)** — the no-Atari-video `sshiftmod`
  fallback shadow (`xbios/screen.rs:25,57-62`). This is a compatibility shim,
  **not** a truthful geometry code (`ST_HIGH` is 640×400 mono, whereas the mode
  is 640×480×16-colour; the exact Atari match would be `4`/`TT_MEDIUM`).
  **Decision: keep `2`** — it is what the code returns and the safe "high-res
  desktop" hint; the real device caps come from the VDI `DEV_TAB`/`INQ_TAB`,
  not from `Getrez`. `MAX_REZ = 2` (`xbios/screen.rs:27`), so `Setscreen` only
  accepts `0..2` today. Revisit at the HDMI-mode-query (1080p) phase.
- **Palette / colour:** 16 CLUT entries emulated over 32bpp
  (`vdi_backend.rs:21,26-43`; index 0 = white/background, 1 = black/
  foreground). `vs_color` sets the PEL→RGB map in the standard 0..1000-per-gun
  range (`vdi_backend.rs:142-151`). **Caveat (Step-2):** XBIOS
  `Setpalette`/`Setcolor` write a *separate* ST-format shadow in
  `xbios/screen.rs` that is **not** wired to the VDI CLUT, so they do not yet
  change displayed colours — see the palette-spec bindings.
- **VDI `DEV_TAB`/`INQ_TAB` answers (resolved from `tos-api/src/vdi/`):**
  `DEV_TAB[0,1]` = **639, 479** (max x/y, patched from the live framebuffer);
  `DEV_TAB[13]` = **16** (colours); `DEV_TAB[35]` = **1** (>1 plane);
  `DEV_TAB[39]` = **16** (palette size); `INQ_TAB[4]` = **4** (planes,
  `log2(16)`); `INQ_TAB[5]` = **1** (colour LUT). Pixel size in microns
  (`DEV_TAB[3,4]`) is **left at the ROM default `372`** — not patched from the
  HDMI mode (`workstation.rs:101-106` patches only `[0],[1],[13],[35],[39]`);
  truthful values need the mailbox physical-display-size query, deferred.
  Source: `tos-api/src/vdi/workstation.rs:24-56,100-117,155-160`.
- **`NUM_VDI_HANDLES` / `MAX_VERTICES`:** the **small NewTOS profile** —
  `NUM_VDI_HANDLES` = **16** (`HANDLE_COUNT`, `tos-api/src/vdi/vwk.rs:20`;
  handle 1 = physical, 2..=16 virtual, a static slot table not `Mxalloc`);
  `MAX_VERTICES` = **256** (`workstation.rs:42`, `primitives.rs:115`). Both are
  smaller than the EmuTOS 64/128 handles and 512/1024 verts — the existing
  `v_opnvwk`/`vq_extnd` specs still cite the EmuTOS figures; that text should be
  updated to 16/256 for NewTOS (a Step-2 spec-text fix).

### 3.2 Console (CON) + serial/debug (PL011 UART)

- **Console `CON` (BIOS handle 2 / GEMDOS `Cconin`/`Cconout`):** the console is
  the **screen + keyboard**, not the UART. Output = the framebuffer VT52
  emulator (`fbcon.rs`); input = the USB-HID keyboard (`drivers/usb/hid.rs`,
  §3.3).
- **BIOS device-handle map (resolved, `tos-api/src/bios/chardev.rs`):**
  **CON (2)** = framebuffer VT52 out + USB-HID keyboard in; **IKBD (4)** =
  aliased to the same console keyboard ring (input); **raw-CON (5)** =
  framebuffer out; **AUX (1)** = PL011 (present, but currently a dummy at the
  BIOS char-device layer — returns 0); **PRT (0) / MIDI (3) / 6,7** = absent
  → 0. The MIDI/IKBD `Bcostat` status-slot swap (handle 3↔4) is unobservable
  since both are absent (see `docs/api/bios/08-bcostat.md`). VT52 output today
  covers glyphs + CR/LF/BS/TAB + wrap/scroll; full escape sequences are
  Step-2.
- **UART (PL011) = debug channel only.** The PL011 carries `kprintln!` debug
  output (and can back the `AUX` device / an interactive shell during
  bring-up), but it is **not** the primary TOS console. Target model: `CON`
  never reads/writes the UART. **Step-2 violation (K1):** today the console
  RX ring is fed **both** by the USB-HID keyboard and by the PL011 UART0
  (`console.rs:91-107`), and the blocking/`pump_input` reads poll UART0 on
  real Pi-4 (`host_env.rs:112,120`), so `Cconin`/`Bconin(2)` currently
  return UART bytes — a console-policy violation to fix in Step 2. Console
  *output* also mirrors to the UART (`host_env.rs:17-20`), which is
  acceptable (debug mirror), unlike input.
- **PL011 UART0 concrete values** (device: `bcm2711/pl011.rs`; DS §11 p.144,
  §11.5 p.146; UART0 base `0x7E20_1000` → mapped `0xFE20_1000`,
  `pl011.rs:29`):
  - **Reference clock `FUARTCLK` = 48 MHz** (firmware `init_uart_clock`,
    `pl011.rs:14-18`); `virt:` **24 MHz** (`qemu_virt/pl011.rs:11`).
  - **Default line setting = 115200 8N1**, FIFO enabled (LCRH `WLEN_8 | FEN`,
    no parity, 1 stop bit; `pl011.rs:132`, `hal/uart.rs:11`).
  - **Baud programming:** `BAUDDIV = FUARTCLK/(16·baud)`, split into
    `IBRD` (0x24) integer + `FBRD` (0x28) 6-bit fraction (DS §11.5 p.150). At
    48 MHz/115200 → **IBRD=26, FBRD=3** (`pl011.rs:128-129`); `virt:` IBRD=13,
    FBRD=1.
  - **Flow control = none**: `CR = UARTEN|TXE|RXE` (`pl011.rs:135`); the RTS/CTS
    hardware-flow bits `CR.RTSEN`/`CR.CTSEN` (DS §11.5 p.151-152) are **not**
    set, and the CTS0/RTS0 pins (GPIO16/17, ALT3, DS §11.3 p.145) are **not**
    muxed — only TX/RX (GPIO14/15, ALT0, `pl011.rs:98-103`). Hardware
    (RTS/CTS) flow in `Rsconf` is therefore not honourable today (Step-2 R5).
- **`Rsconf` on the PL011:** `speed` (baud idx) → IBRD/FBRD; `ctrl` NONE/SOFT
  map (HARD/BOTH blocked until RTS/CTS pins are muxed); the MFP `ucr`/`rsr`/
  `tsr`/`scr` register args have **no 1:1 PL011 model** — framing bits *could*
  map to LCRH but are currently **ignored on input**, and the packed status
  return is **synthesized** (`docs/api/xbios/0f-rsconf.md` Pi binding). **Not
  wired in the dispatcher today (Step-2 R1).**
- **`Bconmap`:** single-UART build → contractually returns `0x2c`
  (unavailable), per the XBIOS spec — no `MAPTAB`, nothing to remap. **Not
  wired today (Step-2 R3): returns `EINVFN` instead of `0x2c`.** Handles:
  `CON` = screen+keyboard; `AUX (1)` = PL011 (debug, but a BIOS *dummy*
  today — K9/R6); `PRN`/`MIDI` are absent.
- **`Midiws`:** no MIDI ACIA on the Pi (§3.9) → contractual **no-op** (`void`).
  **Not wired today (Step-2 R4): returns `EINVFN` instead of the silent
  no-op.**
- **RX IRQ caveat:** on real Pi-4 the PL011 RX IRQ is not delivered through
  GIC-400 as in QEMU, so UART input polls directly (`docs/MEMORY.md`); this
  only matters for the debug/shell path, not the framebuffer console.
  `virt:` RX IRQ works.

### 3.3 Keyboard / mouse

- **Current:** USB-HID keyboard + mouse over the xHCI controller (VL805) on
  the PCIe bus (`drivers/usb/xhci`, `drivers/usb/hid.rs`, `drivers/usb/mod.rs`
  `poll`). HID boot reports are translated to Atari IKBD scancodes / packets
  (`drivers/usb/hid.rs`) and pushed as raw IKBD bytes through `ikbd.rs::feed`,
  which drives the console key ring (keyboard) and `vdi::feed_relative` (mouse
  cursor). Polled from `host_env::pump_input`, not IRQ-driven.
- **Planned:** a native Atari ST keyboard on a second UART at 7812.5 baud
  (`docs/MEMORY.md`); the IKBD parser exists, UART2 pin wiring pending. **(plan)**
- **HID→IKBD keyboard scancode map** (`drivers/usb/hid.rs:14-43`
  `translate_key`): USB HID usage → Atari **make** code (break = `| 0x80`).
  Extracted from FreeMiNT `usb_keyboard.c`. Key examples: letters
  `A=0x1e … Z=0x2c`; digits `1=0x02 … 0=0x0b`; `Return=0x1c Esc=0x01
  Backspace=0x0e Tab=0x0f Space=0x39`; **arrows Left=0x4b Right=0x4d
  Up=0x48 Down=0x50**; `F1..F10=0x3b..0x44`, `F11→Help=0x62 F12→Undo=0x61`;
  `Home=0x47 Insert=0x52 Delete=0x53`. Modifiers
  (`hid.rs:52-65`): Ctrl(either)=0x1d, LShift=0x2a, RShift=0x36, Alt=0x38,
  CapsLock toggle=0x3a. Report processing is a make/break diff vs. the
  previous 8-byte boot report (`hid.rs:69-100`).
- **Key-ring word encoding** (`ikbd.rs:139`, `console.rs:109-133`): each make
  key is queued as a 16-bit word **`scancode << 8 | ascii`** (ascii `0` for
  keys with no character, e.g. arrows / function keys). ASCII readers
  (`Cconin`/`Bconin` via `rx_pop`) take the low byte and skip ascii-0 keys;
  the AES event manager reads the whole word via `rx_pop_key`. **(Note:
  `Bconin(2)` should pack the scancode into bits 16-23 but currently returns
  ASCII only — Step-2 K2.)**
- **Mouse packet synthesis** (`drivers/usb/hid.rs:106-112`): USB boot mouse
  report `[buttons, dx, dy]` → IKBD relative packet `[0xF8|wire, dx, dy]`.
  Button double-swap: USB(bit0 left,bit1 right) → wire(bit0 right,bit1 left)
  at `hid.rs:108` → VDI(bit0 left,bit1 right) at `ikbd.rs:98-104`; net =
  identity. `+dy = down` passed through. Middle button + wheel dropped.
  Decoded in `ikbd.rs:68-122` → `vdi::feed_relative` (accumulate + clamp to
  the live framebuffer bounds).
- **Keyboard layout:** built-in **US (QWERTY)** + **DE (QWERTZ)**, extracted
  verbatim from EmuTOS `keyb_us.h`/`keyb_de.h` (`keymap.rs:50-103,118-171`),
  each three 128-byte `norm`/`shft`/`caps` tables + AltGr `(scancode, ascii)`
  override pairs (DE: `@ [ ] \ { }`, `keymap.rs:174-183`). Selected at boot by
  `init_from_disk` (`keymap.rs:266-281`): `C:\KEYBOARD.KBD` (`KBD1`-magic
  native blob, `load_kbd`) > `#K <layout>` line in `C:\NEWDESK.INF` >
  compiled default **US**. `_AKP` cookie = `0x00000000` (US, §3.7).
- **XBIOS `Kbdvbase`/`Iorec`/`Initmous`/`Keytbl`/`Bioskeys`/`Ikbdws`/`Kbrate`:**
  the IKBD vector table + input record model is the *contract surface* over
  the HID path. **Step-2 status:** only `Kbdvbase` (0x22) is wired in the
  XBIOS dispatcher (and it returns an **all-zero** vector table that no code
  populates or dispatches through); `Initmous 0x00`, `Iorec 0x0E`,
  `Keytbl 0x10`, `Bioskeys 0x18`, `Ikbdws 0x19`, `Kbrate 0x23` are **not in
  the dispatcher** (`tos-api/src/xbios/mod.rs`) → return `EINVFN`. The
  keyboard/mouse *work* via the direct `ikbd::feed` path, but the XBIOS
  keyboard-group **API** is unimplemented — see `docs/STEP2-VALIDATION.md`
  (K-series).

### 3.4 Block device / drives (EMMC SD)

- **Device:** **EMMC2 SD-card controller** (Arasan/iproc SDHCI, phys base
  `0xFE34_0000`; `bcm2711/emmc.rs:16-18`). Address map: DS §1.2 (p.4-6); EMMC2
  IRQ 62 (DS p.90 interrupt table); register/bit detail per BCM2835 DS §5 + SD
  Host Controller Simplified Spec (`docs/hardware/emmc-sd.md`). `virt:` QEMU
  `raspi4b` wires its emulated card to the **legacy EMMC** `0xFE30_0000`; the
  driver probes EMMC2 then EMMC and binds whichever responds
  (`emmc.rs:375-386`) — one binary for both. `Rwabs`/`Getbpb`/`Mediach`/
  `Drvmap` operate on it via `hal/block.rs::BlockDevice` →
  `tos-api::bios::block::BlockDev` (`host_env.rs:246-267`).
- **Sector size = 512 bytes, fixed** (`hal/block.rs:9`, `bios/mod.rs:26`; SD
  `BLKSIZECNT` = 512 per transfer, `emmc.rs:298,323`). PIO single-block read
  (CMD17) + write (CMD24); **write path implemented**; no DMA, no IRQ (polled,
  `emmc.rs:210,284-331`).
- **Block count:** read from the card **CSD** at init (CMD9) — v2 SDHC/SDXC
  `(C_SIZE+1)×1024` 512-B blocks, v1 SDSC from `C_SIZE`/`C_SIZE_MULT`/
  `READ_BL_LEN` (`emmc.rs:107-130,262-270`); exposed as `block_count()`.
- **Card layout (real HW):** MBR + **4 × FAT16 primary partitions → C: D: E: F:**
  (verified on real Pi-4) ⇒ `Drvmap()` = bits 2-5 = `0x3C`. `virt:` a raw FAT16
  image built by `tools/make-sd-image.sh` (MBR sig `0x55AA`, one primary of type
  `0x06`) → **C:** ⇒ `Drvmap()` = `0x04`. Drives are assigned from **C:**
  (`FIRST_HD=2`); A:/B: (floppy) never set. MBR parsed once at boot in
  `bios/drive.rs::mount` (primaries at `0x1BE`, then the extended/EBR chain;
  no-MBR device mounts whole as C:).
- **BPB:** synthesized per call from the mounted partition's FAT16 boot sector
  (partition-relative sector 0), little-endian, no byte-swap on ARM
  (`bios/bpb.rs:60-103`). Fields filled: `recsiz clsiz clsizb rdlen fsiz fatrec
  datrec numcl b_flags`; `B_16` when `numcl >= 4085`, `B_1FAT` when one FAT.
- **Filesystem scope:** **FAT16 hard-disk only**; FAT12/floppy out of scope;
  exFAT is a Phase-09 goal (`docs/MEMORY.md`). FAT32/exFAT boot sectors
  (`fsiz16==0`) ⇒ `Getbpb` returns NULL.
- **`Mediach` = non-removable → always `0`** (`bios/mod.rs:120-122`): ignores
  its argument, never probes hardware, no `forcechange`/latch. **Step-2 gap:**
  it does not return `EUNDEV` for an invalid drive (returns `0` for any input).
- **Step-2 candidates:** `RW_NOTRANSLATE`/`RW_NOMEDIACH`/`RW_NORETRIES` flags
  not honoured; sector-0-write `forcechange` and NULL-buffer `Rwabs` features
  absent; `Mediach` invalid-drive `EUNDEV` missing; FAT16-only (FAT32/exFAT
  deferred). See `docs/STEP2-VALIDATION.md` (B-series).

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
