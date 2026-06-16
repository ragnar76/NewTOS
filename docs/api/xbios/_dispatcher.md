# XBIOS Trap-14 Dispatcher

> **Phase 00 deliverable D2 (Trap 14).**
> Source: EmuTOS @ `1a7484ab`.
>
> XBIOS shares the trap dispatch trampoline with BIOS — the only
> difference is which table is consulted. See
> [`docs/api/bios/_dispatcher.md`](../bios/_dispatcher.md) for the
> trampoline mechanics; this file documents only the XBIOS-specific
> wiring.

---

## 1. Vector installation

The trap-14 vector is set up alongside trap-13 in `vecs_init()`
(`bios/bios.c`). Handler is `_xbiostrap` at `bios/vectors.S:1275`.

---

## 2. `_xbiostrap` entry (`bios/vectors.S:1275`)

```asm
_xbiostrap:
    move.w  _xbios_ent, d1
    lea     _xbios_vecs, a0
    ; falls through into biosxbios:
```

- `_xbios_ent` (`bios/xbios.c:1517`) = `ARRAY_SIZE(xbios_vecs)`,
  whose value depends on build configuration (see Section 4).
- `_xbios_vecs` (`bios/xbios.c:1285`) is the function-pointer table.

Falls through to the shared `biosxbios:` trampoline (see BIOS
dispatcher doc for the body).

---

## 3. Function "not implemented" handler

Out-of-range IDs (those past the table) cannot occur — the trampoline's
`cmp.w d1, d0; jhs bx_ret_exc` check prevents lookups beyond
`xbios_ent`. The function number is returned to caller in D0.

For IDs **inside** the table that are not implemented in the current
build, EmuTOS plants `xbios_unimpl` (`bios/vectors.S:1366`) at that
slot. `xbios_unimpl` does:

```asm
_xbios_unimpl:
    move.w  d0, -(sp)            ; push function number
    jsr     _xbios_do_unimpl     ; C: kprintf + return number
    addq.l  #2, sp
    rts
```

`xbios_do_unimpl` (`bios/xbios.c:1251`):
```c
LONG xbios_do_unimpl(WORD number)
{
#if DBG_XBIOS
    kprintf("unimplemented XBIOS function 0x%02x\n", number);
#endif
    return number;
}
```

So calling an unimplemented XBIOS function returns the function number
in D0 — the documented TOS behaviour.

---

## 4. Build-time table size

`bios/xbios.c:1268-1276` selects `LAST_ENTRY` based on configuration:

```c
#if   CONF_WITH_DMASOUND   /* Falcon DMA sound */
# define LAST_ENTRY 0x8d
#elif CONF_WITH_DSP        /* Falcon DSP */
# define LAST_ENTRY 0x7f
#elif CONF_WITH_VIDEL      /* Falcon Videl */
# define LAST_ENTRY 0x5f
#elif CONF_WITH_TT_SHIFTER /* TT256 shifter */
# define LAST_ENTRY 0x57
#else                      /* ST baseline */
# define LAST_ENTRY 0x40
#endif
```

`xbios_vecs[]` is padded with `xbios_unimpl` slots to fill up to
`LAST_ENTRY + 1` entries.

NewTOS target configuration: **TT shifter baseline** — the table will
contain entries 0x00–0x57 (88 entries). The Falcon-only ranges
(0x58–0x8D) are deferred / not exposed; reserve those IDs but return
"unimplemented".

---

## 5. `xbios_vecs[]` table

| ID    | C function       | TOS name          | Cfg                   |
|------:|------------------|-------------------|-----------------------|
| 0x00  | `Initmous`       | Initmous          |                       |
| 0x01  | `xbios_unimpl`   | (Ssbrk)           | unused                |
| 0x02  | `physbase`       | Physbase          |                       |
| 0x03  | `logbase`        | Logbase           |                       |
| 0x04  | `getrez`         | Getrez            |                       |
| 0x05  | `setscreen`      | Setscreen         |                       |
| 0x06  | `setpalette`     | Setpalette        |                       |
| 0x07  | `setcolor`       | Setcolor          |                       |
| 0x08  | `floprd`         | Floprd            |                       |
| 0x09  | `flopwr`         | Flopwr            |                       |
| 0x0A  | `flopfmt`        | Flopfmt           |                       |
| 0x0B  | `xbios_unimpl`   | (used_by_bios)    | unused                |
| 0x0C  | `midiws`         | Midiws            |                       |
| 0x0D  | `mfpint`         | Mfpint            | `CONF_WITH_MFP`       |
| 0x0E  | `iorec`          | Iorec             |                       |
| 0x0F  | `rsconf`         | Rsconf            |                       |
| 0x10  | `keytbl`         | Keytbl            |                       |
| 0x11  | `random`         | Random            |                       |
| 0x12  | `protobt`        | Protobt           |                       |
| 0x13  | `flopver`        | Flopver           |                       |
| 0x14  | `scrdmp`         | Scrdmp            |                       |
| 0x15  | `cursconf`       | Cursconf          |                       |
| 0x16  | `settime`        | Settime           |                       |
| 0x17  | `gettime`        | Gettime           |                       |
| 0x18  | `bioskeys`       | Bioskeys          |                       |
| 0x19  | `ikbdws`         | Ikbdws            |                       |
| 0x1A  | `jdisint`        | Jdisint           | `CONF_WITH_MFP`       |
| 0x1B  | `jenabint`       | Jenabint          | `CONF_WITH_MFP`       |
| 0x1C  | `giaccess`       | Giaccess          |                       |
| 0x1D  | `offgibit`       | Offgibit          |                       |
| 0x1E  | `ongibit`        | Ongibit           |                       |
| 0x1F  | `xbtimer`        | Xbtimer           | `CONF_WITH_MFP`       |
| 0x20  | `dosound`        | Dosound           |                       |
| 0x21  | `setprt`         | Setprt            | `CONF_WITH_PRINTER_PORT` |
| 0x22  | `kbdvbase`       | Kbdvbase          |                       |
| 0x23  | `kbrate`         | Kbrate            |                       |
| 0x24  | `xbios_unimpl`   | (Prtblk)          | unimpl                |
| 0x25  | `vsync`          | Vsync             |                       |
| 0x26  | `supexec`        | Supexec           |                       |
| 0x27  | `xbios_unimpl`   | (Puntaes)         | unimpl                |
| 0x28  | `xbios_unimpl`   | —                 | unimpl                |
| 0x29  | `floprate`       | Floprate          |                       |
| 0x2A  | `DMAread`        | DMAread           |                       |
| 0x2B  | `DMAwrite`       | DMAwrite          |                       |
| 0x2C  | `bconmap`        | Bconmap           |                       |
| 0x2D  | `xbios_unimpl`   | —                 | unimpl                |
| 0x2E  | `nvmaccess`      | NVMaccess         | `CONF_WITH_NVRAM`     |
| 0x2F–0x3F | `xbios_unimpl` | —              | unimpl                |
| 0x40  | `blitmode`       | Blitmode          |                       |
| 0x41–0x4F | `xbios_unimpl` | —              | filler (TT shifter)   |
| 0x50  | `esetshift`      | EsetShift         | `CONF_WITH_TT_SHIFTER`|
| 0x51  | `egetshift`      | EgetShift         | `CONF_WITH_TT_SHIFTER`|
| 0x52  | `esetbank`       | EsetBank          | `CONF_WITH_TT_SHIFTER`|
| 0x53  | `esetcolor`      | EsetColor         | `CONF_WITH_TT_SHIFTER`|
| 0x54  | `esetpalette`    | EsetPalette       | `CONF_WITH_TT_SHIFTER`|
| 0x55  | `egetpalette`    | EgetPalette       | `CONF_WITH_TT_SHIFTER`|
| 0x56  | `esetgray`       | EsetGray          | `CONF_WITH_TT_SHIFTER`|
| 0x57  | `esetsmear`      | EsetSmear         | `CONF_WITH_TT_SHIFTER`|

Beyond 0x57 (out of NewTOS scope unless we adopt CONF_WITH_VIDEL/DSP/
DMASOUND): VIDEL (0x58–0x5F), DSP (0x60–0x7F), DMA sound (0x80–0x8D).
Consult `bios/xbios.c:1432-1517` for the exact list.

---

## 6. Source-location summary

| Concern                       | File:line                  |
|-------------------------------|----------------------------|
| Trap-14 entry                 | `bios/vectors.S:1275`      |
| Shared trampoline             | `bios/vectors.S:1282`      |
| Unimpl shim                   | `bios/vectors.S:1366`      |
| Unimpl C tail                 | `bios/xbios.c:1251`        |
| Function table                | `bios/xbios.c:1285-1517`   |
| `LAST_ENTRY` selector         | `bios/xbios.c:1268-1276`   |
| Table size                    | `bios/xbios.c:1517` (`_xbios_ent`) |

---

## NewTOS ABI mapping notes

(Empty during Phase 00. Filled in during Phase 07.)
