# `Bconout` — BIOS 0x03

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bios/bios.c:991` (`bconout`).

## EmuTOS source

- **Primary:** `bios/bios.c:991` — `bconout(WORD handle, WORD what)`
- **Helpers:** `protect_ww()` (`util/miscasm.S:240`) — d2/a2-preserving
  call thunk for two-WORD-argument handlers
- **Tables:** `bconout_vec[8]`, `bconmap_root.maptab[]`
- **Globals read:** `boot_status & CHARDEV_AVAILABLE`
- **Build config:** `BCONMAP_AVAILABLE`,
  `CONF_SERIAL_CONSOLE`/`CONF_SERIAL_CONSOLE_ANSI` (affect handler 5).

## Source extraction

```c
LONG bconout(WORD handle, WORD what)
{
    if (!(boot_status & CHARDEV_AVAILABLE))
        return 0;

#if BCONMAP_AVAILABLE
    WORD map_index = handle - BCONMAP_START_HANDLE;
    if (map_index >= bconmap_root.maptabsize) return 0L;
    if (map_index >= 0)
        return protect_ww((PFLONG)bconmap_root.maptab[map_index].Bconout,
                          handle, what);
#endif

    if ((handle >= 0) && (handle <= 7))
        return protect_ww((PFLONG)bconout_vec[handle], handle, what);
    return 0L;
}
```

Per-handle handlers receive **both** the original `handle` and the
character, unlike `Bconin`/`Bconstat`/`Bcostat` which receive nothing.
This lets one C function service multiple handles (e.g., `bconout5`
delegates to `bconout(1, ...)` for the serial-console case).

## Trap interface

| | |
|--|--|
| Trap | 13 |
| Function ID | `0x0003` |
| WORD args | 2 |
| Stack frame | `+4: what (WORD)`, `+2: handle (WORD)`, `+0: 0x0003` |

## Inputs

| Pos | Name     | Type | Range / meaning                                  |
|-----|----------|------|--------------------------------------------------|
| 0   | `handle` | WORD | 0=PRT, 1=AUX, 2=CON (screen), 3=MIDI-out, 4=IKBD-out, 5=raw-CON, 6,7=Bconmap |
| 1   | `what`   | WORD | byte to output (low 8 bits); upper byte ignored by all default handlers |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |

| Handler                       | Return value       | Source              |
|-------------------------------|--------------------|---------------------|
| `bconout0` (parport)          | `0L` if printer BUSY timed out; non-zero (`1L` typically) on byte sent | `bios/parport.c` |
| `bconout1` (RS-232)           | `1L` after byte transmitted (waits for THR-empty) | `bios/serport.c` |
| `bconout2` (screen via cputc) | `1L`               | `bios/chardev.c:88` |
| `bconout3` (MIDI out)         | `1L`               | `bios/midi.c`       |
| `bconout4` (IKBD out)         | `1L`               | `bios/ikbd.c`       |
| `bconout5` (raw screen, no VT52) | `1L`            | `bios/chardev.c:95` |
| `charout_dummy` (handles 6,7) | `0L`               | `bios/chardev.c:55` |
| boot too early / invalid handle | `0L`             |                     |

The "non-zero on success" pattern is consistent across the standard
handlers — except `bconout0` which uses `0L` to signal "printer busy"
so applications can poll and retry.

## Errors

`bconout0` can return `0L` for "printer not ready"; all others either
succeed or return `0L` only on boot-state guard or invalid handle.

## Edge cases

- **Handle 2 (screen)** routes through `cputc()` (`bios/conout.c`),
  which interprets VT52 escape sequences when `CONF_WITH_VT52` is set.
  Escape state persists across calls — see `Cconout` spec for the
  caveat.
- **Handle 5 (raw)**: same screen, **no** VT52 — bytes are blitted
  literally via `ascii_out()`. With `CONF_SERIAL_CONSOLE`, also echoed
  to AUX (handle 1).
- **Handle 4 (IKBD out)** sends commands to the keyboard controller —
  not a user-text channel. `Ikbdws` (XBIOS 0x19) is the higher-level
  interface.
- **`protect_ww` register save** preserves d2/a2 around the call.
- **Vector overwrite**: applications may write `bconout_vec[h]`
  directly to install a custom output driver. The pattern is used by
  printer-spooler accessories.

## Dependencies

- `chardev_init()` must have run.
- For handle 2: `screen_init()` must have set up `v_bas_ad` and the
  conout state machine (`bios/conout.c:cputc_init`).
- For handle 1: `init_serport()`.
- For handle 0: `init_parport()`.

Used by: every output GEMDOS function (`Cconout`, `Cauxout`,
`Cprnout`, `Cconws`, …); the kernel's own `kprintf()`; AES/VDI
ultimately.

## Cross-references

- **Compendium:** "Bconout" — agrees on signature and convention.
- **Profibuch:** "Bconout (3)" — agrees.
- **`docs/api/bios/_dispatcher.md`**.

## Configuration / version

- `CONF_WITH_VT52`: handle 2 escape parsing.
- `CONF_SERIAL_CONSOLE`: handle 2 also writes to handle 1.
- `CONF_SERIAL_CONSOLE_ANSI`: serial mirror translates VT52 → ANSI.
- `CONF_WITH_PRINTER_PORT`: handle 0 active.
- `BCONMAP_AVAILABLE`: handles ≥ 6 routed via remap table.

## NewTOS implementation notes

(Empty during Phase 00.)
