# `Cauxin` — GEMDOS 0x03

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xauxin()`.

## EmuTOS source

- **Primary:** `bdos/console.c:425` — `xauxin()`
- **Helpers:** `Bconin` (BIOS 0x02)
- **Globals read:** `run->p_uft[2]` (stdaux standard handle)
- **Build config:** transitive (`BCONMAP_AVAILABLE` if AUX has been
  remapped via `Bconmap`).

## Source extraction

```c
long xauxin(void)
{
    return Bconin(HXFORM(run->p_uft[2]));
}
```

Pure pass-through to `Bconin` on the AUX device (default BIOS device 1
= RS-232). **Does not** consult the AUX typeahead buffer
(`aux_typeahead`) maintained in `bdos/console.c` — the buffer exists
but no GEMDOS path ever populates or drains it for AUX. See
discrepancy `D-003`.

`Cauxin` is **blocking** — `Bconin` blocks until a byte is available.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0003` |
| WORD args | 0 |

## Inputs

None.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Encoding | typically the data byte in low 8 bits; upper bytes are device-driver-defined (RS-232 may pack flags). EmuTOS forwards `Bconin` verbatim. |

## Errors

None at the GEMDOS level. Hardware framing/parity errors surface in the
return value's upper bytes if the BIOS RS-232 driver is configured to
report them; default builds do not.

## Edge cases

- **No typeahead integration** — see `D-003`. Even if a Ctrl-S arrives
  on AUX, GEMDOS does not honour it.
- **Standard-handle redirection** (`run->p_uft[2] > 0`): dispatcher reads
  one byte from the redirected file via `xread()`; this function not
  entered.
- **`Bconmap` (XBIOS 0x2C)** can remap which BIOS device handle 6/7
  refers to; AUX (handle 1) is unaffected by `Bconmap` itself.

## Dependencies

- `Bconin(h)`
- `run->p_uft[2]`

Used by: programs reading from RS-232 / current AUX device.

## Cross-references

- **Compendium:** "Cauxin" — agrees.
- **Profibuch:** "Cauxin (3)" — agrees, including blocking semantics.
- **`docs/api/_discrepancies.md` D-003** — typeahead buffer unused.

## Configuration / version

- `CONF_WITH_MFP`, `CONF_WITH_SCC`: affect which BIOS driver backs
  device 1 (or whatever `p_uft[2]` resolves to).

## NewTOS implementation notes

(Empty during Phase 00. Filled in during Phase 07.)
