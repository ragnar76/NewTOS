# XBIOS triage — D5

> Phase 00 deliverable. Source: EmuTOS @ `1a7484ab` —
> `bios/xbios.c:1285` (`xbios_vecs[]`), build profile per
> `CLAUDE.md`: `CONF_WITH_TT_SHIFTER + CONF_WITH_VT52`,
> **disable** `CONF_WITH_VIDEL`, `CONF_WITH_DSP`,
> `CONF_WITH_DMASOUND`, `CONF_ATARI_HARDWARE`,
> `CONF_WITH_MFP`, `CONF_WITH_PRINTER_PORT`,
> `CONF_WITH_NVRAM`.

For NewTOS, `LAST_ENTRY = 0x57` — XBIOS table covers 0x00..0x57
(88 entries). Functions outside the build profile (Falcon DSP /
DMA-sound / Videl / MFP / etc.) are dispatched through
`xbios_unimpl`, which returns the function number itself (not a
GEMDOS error — see *D-008* below).

## Status legend

- **✅ spec** — full per-function spec written (`docs/api/xbios/NN-name.md`)
- **🟢 implement** — NewTOS will implement (PHASE-07/08)
- **🟡 stub** — return EINVFN/sensible default; ABI surface only
- **⛔ unimpl** — `xbios_unimpl` (number return); not built in NewTOS profile
- **➡ dispatch** — present in table but always routes elsewhere

| #     | Name           | EmuTOS func           | NewTOS plan | Spec | Notes |
|------:|----------------|------------------------|-------------|------|-------|
| 0x00  | Initmous       | `Initmous`             | 🟢 PHASE-07 (USB HID) | ☐ | mouse vector configurator |
| 0x01  | Ssbrk          | `xbios_unimpl`         | ⛔ unimpl | — | obsolete since TOS 1.0 |
| 0x02  | Physbase       | `physbase` (`screen.c:1042`) | 🟢 PHASE-08 | ✅ | display physical base |
| 0x03  | Logbase        | `logbase` (`screen.c:1071`)  | 🟢 PHASE-08 | ✅ | `v_bas_ad` |
| 0x04  | Getrez         | `getrez` (`screen.c:1076`)   | 🟢 PHASE-08 | ✅ | current resolution |
| 0x05  | Setscreen      | `setscreen` (`screen.c:1102`) | 🟢 PHASE-08 | ✅ | reconfigure display |
| 0x06  | Setpalette     | `setpalette` (`screen.c:1168`) | 🟢 PHASE-08 | ✅ | full ST/STe palette set |
| 0x07  | Setcolor       | `setcolor` (`screen.c:1192`)   | 🟢 PHASE-08 | ✅ | single-entry palette |
| 0x08  | Floprd         | `floprd`               | 🟡 stub → `EUNDEV` | ☐ | no floppy hardware |
| 0x09  | Flopwr         | `flopwr`               | 🟡 stub → `EUNDEV` | ☐ | no floppy |
| 0x0A  | Flopfmt        | `flopfmt`              | 🟡 stub → `EUNDEV` | ☐ | no floppy |
| 0x0B  | (used_by_bios) | `xbios_unimpl`         | ⛔ unimpl | — | reserved |
| 0x0C  | Midiws         | `midiws`               | 🟡 stub | ☐ | no MIDI hardware (PHASE-07?) |
| 0x0D  | Mfpint         | `mfpint`               | ⛔ unimpl | — | MFP-only |
| 0x0E  | Iorec          | `iorec`                | 🟢 PHASE-07 | ☐ | input ringbuffer config |
| 0x0F  | Rsconf         | `rsconf`               | 🟡 stub | ☐ | RS232 config — UART maps to BIOS console |
| 0x10  | Keytbl         | `keytbl` (`ikbd.c`)    | 🟢 PHASE-07 | ☐ | keyboard table swap |
| 0x11  | Random         | `random` (`xbios.c:421`) | 🟢 PHASE-07 | ✅ | LCG, seeds from `hz_200` |
| 0x12  | Protobt        | `protobt`              | 🟢 PHASE-04 | ☐ | boot-sector image builder |
| 0x13  | Flopver        | `flopver`              | 🟡 stub → `EUNDEV` | ☐ | no floppy |
| 0x14  | Scrdmp         | `scrdmp`               | 🟡 stub | ☐ | screen-dump-to-printer; no printer |
| 0x15  | Cursconf       | `cursconf` (`vt52.c:893`) | 🟢 PHASE-08 | ✅ | VT52 cursor config |
| 0x16  | Settime        | `settime` (`clock.c:1218`) | 🟢 PHASE-07 | ✅ | RTC + GEMDOS time |
| 0x17  | Gettime        | `gettime` (`clock.c:1266`) | 🟢 PHASE-07 | ✅ | RTC read |
| 0x18  | Bioskeys       | `bioskeys` (`ikbd.c:136`) | 🟢 PHASE-07 | ✅ | reload default key tables |
| 0x19  | Ikbdws         | `ikbdws`               | 🟡 stub | ☐ | send raw IKBD command |
| 0x1A  | Jdisint        | `jdisint`              | ⛔ unimpl | — | MFP-only |
| 0x1B  | Jenabint       | `jenabint`             | ⛔ unimpl | — | MFP-only |
| 0x1C  | Giaccess       | `giaccess`             | 🟡 stub | ☐ | YM2149 register access; no chip |
| 0x1D  | Offgibit       | `offgibit`             | 🟡 stub | ☐ | YM2149 |
| 0x1E  | Ongibit        | `ongibit`              | 🟡 stub | ☐ | YM2149 |
| 0x1F  | Xbtimer        | `xbtimer`              | ⛔ unimpl | — | MFP-only |
| 0x20  | Dosound        | `dosound`              | 🟡 stub | ☐ | YM2149 sound; no chip |
| 0x21  | Setprt         | `setprt`               | ⛔ unimpl | — | no printer port |
| 0x22  | Kbdvbase       | `kbdvbase` (`xbios.c:716`) | 🟢 PHASE-07 | ✅ | returns `&kbdvecs` |
| 0x23  | Kbrate         | `kbrate`               | 🟢 PHASE-07 | ☐ | repeat rate config |
| 0x24  | Prtblk         | `xbios_unimpl`         | ⛔ unimpl | — | no printer |
| 0x25  | Vsync          | `vsync` (`screen.c:1206`) | 🟢 PHASE-08 | ☐ | wait for VBL |
| 0x26  | Supexec        | `supexec` (asm)        | 🟢 PHASE-06 | ✅ | run callback in supervisor mode |
| 0x27  | Puntaes        | `xbios_unimpl`         | ⛔ unimpl | — | drop AES (boot path) |
| 0x28  | (reserved)     | `xbios_unimpl`         | ⛔ unimpl | — | |
| 0x29  | Floprate       | `floprate`             | 🟡 stub | ☐ | no floppy |
| 0x2A  | DMAread        | `DMAread`              | 🟢 PHASE-04 (or stub) | ☐ | ACSI/SCSI raw I/O |
| 0x2B  | DMAwrite       | `DMAwrite`             | 🟢 PHASE-04 (or stub) | ☐ | |
| 0x2C  | Bconmap        | `bconmap`              | 🟢 PHASE-07 | ☐ | console-device mapping |
| 0x2D  | (reserved)     | `xbios_unimpl`         | ⛔ unimpl | — | |
| 0x2E  | NVMaccess      | `nvmaccess`            | ⛔ unimpl | — | no NVRAM in NewTOS profile |
| 0x2F..0x3F | (reserved) | `xbios_unimpl`     | ⛔ unimpl | — | 17 entries |
| 0x40  | Blitmode       | `blitmode`             | 🟡 stub → 0 | ☐ | no Atari blitter |
| 0x41..0x4F | (reserved) | `xbios_unimpl`     | ⛔ unimpl | — | 15 entries |
| 0x50  | EsetShift      | `esetshift`            | 🟢 PHASE-08 | ☐ | TT shifter — exposed for source ports |
| 0x51  | EgetShift      | `egetshift`            | 🟢 PHASE-08 | ☐ | |
| 0x52  | EsetBank       | `esetbank`             | 🟢 PHASE-08 | ☐ | |
| 0x53  | EsetColor      | `esetcolor`            | 🟢 PHASE-08 | ☐ | |
| 0x54  | EsetPalette    | `esetpalette`          | 🟢 PHASE-08 | ☐ | |
| 0x55  | EgetPalette    | `egetpalette`          | 🟢 PHASE-08 | ☐ | |
| 0x56  | EsetGray       | `esetgray`             | 🟢 PHASE-08 | ☐ | |
| 0x57  | EsetSmear      | `esetsmear`            | 🟢 PHASE-08 | ☐ | |

## Build-profile boundary

Functions 0x58..0xFF are **not present** in the NewTOS XBIOS
table. A trap with function ≥ `LAST_ENTRY = 0x57` falls into the
dispatcher's range check (see `docs/api/xbios/_dispatcher.md`)
and returns `EINVFN` (-32).

| Range        | Atari era    | NewTOS profile         |
|--------------|--------------|-----------------------|
| 0x58..0x5F   | Falcon Videl | absent (`xbios_unimpl`-padded only when `LAST_ENTRY` higher) |
| 0x60..0x7F   | Falcon DSP   | absent                |
| 0x80..0x8D   | Falcon DMA-sound | absent             |

## Discrepancy candidate D-008

**`xbios_unimpl` returns the function number, not `EINVFN`.**
Compendium states "calling an unimplemented XBIOS function returns
EINVFN (-32)". EmuTOS's `xbios_do_unimpl(WORD number) { return
number; }` returns the (positive) function number — which a caller
checking `result < 0` will misinterpret as success. NewTOS choice
pending: follow EmuTOS for compatibility, or correct to `EINVFN`?

Recommend **follow EmuTOS** — programs that detect "unimplemented"
do so by side effect (no observable change) rather than return-value
checking; flipping to `EINVFN` may break programs that probe XBIOS
function numbers and expect non-negative responses.

## Implementation count

| Category       | Count | NewTOS plan        |
|----------------|------:|--------------------|
| 🟢 implement    |    24 | PHASE-04/07/08     |
| 🟡 stub         |    13 | PHASE-07 (return EUNDEV / EINVFN / 0) |
| ⛔ unimpl       |    51 | xbios_unimpl       |
| **Total**      |    88 | (LAST_ENTRY = 0x57) |

Stub specs in this group will share a small "stub group spec" file
once individual stubs are written (D5 wrap-up step).
