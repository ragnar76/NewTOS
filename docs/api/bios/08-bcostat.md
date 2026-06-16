# `Bcostat` — BIOS 0x08

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bios/bios.c:1143` (`bcostat`).

## EmuTOS source

- **Primary:** `bios/bios.c:1143` — `bcostat(WORD handle)`
- **Helpers:** `protect_v()`
- **Tables:** `bcostat_vec[8]`, `bconmap_root.maptab[]`
- **Globals read:** `boot_status & CHARDEV_AVAILABLE`
- **Build config:** `BCONMAP_AVAILABLE`.

## Source extraction

```c
LONG bcostat(WORD handle)
{
    if (!(boot_status & CHARDEV_AVAILABLE))
        return 0;

#if BCONMAP_AVAILABLE
    WORD map_index = handle - BCONMAP_START_HANDLE;
    if (map_index >= bconmap_root.maptabsize) return 0L;
    if (map_index >= 0)
        return protect_v(bconmap_root.maptab[map_index].Bcostat);
#endif

    if ((handle >= 0) && (handle <= 7))
        return protect_v(bcostat_vec[handle]);
    return 0L;
}
```

Identical pattern to `Bconstat`; reports whether the output device is
ready to accept the next byte.

## Trap interface

| | |
|--|--|
| Trap | 13 |
| Function ID | `0x0008` |
| WORD args | 1 |

## Inputs

| Pos | Name     | Type | Range                                                |
|-----|----------|------|------------------------------------------------------|
| 0   | `handle` | WORD | 0=PRT, 1=AUX, 2=CON, 3=MIDI-out, 4=IKBD-out, 5..7    |

Note: comment at `bios/bios.c:1138` shows the documented mapping
`0:PRT 1:AUX 2:CON 3:MID 4:KEYB`.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |

| State                       | Value |
|-----------------------------|-------|
| Output device ready         | `-1L` |
| Output device busy / unable | `0L`  |
| Boot guard / invalid handle | `0L`  |

## Errors

None separate from "not ready".

## Edge cases

- **Default `bcostat_vec[]`** (`bios/chardev.c:39-42`) — note the
  swap of MIDI/IKBD slots vs. the `bconout`/`bconin` tables:
  ```
  [0]=bcostat0, [1]=bcostat1, [2]=bcostat2, [3]=bcostat4,
  [4]=bcostat3, [5..7]=char_dummy
  ```
  Source comment: `/* note that IKBD and MIDI bcostat() are swapped! */`
  - `bcostat0` (parport): polls Centronics BUSY/SELECT.
  - `bcostat1` (RS-232): UART transmitter-empty flag.
  - `bcostat2` (screen): always returns `-1L` (or, with
    `CONF_SERIAL_CONSOLE`, returns `bcostat(1)`); see
    `bios/chardev.c:113`.
  - `bcostat3` (MIDI tx): UART tx-empty.
  - `bcostat4` (IKBD tx): UART tx-empty.
- **`char_dummy`** at slots 5..7 returns `0L`. (Note: `bcostat5` is
  *not* in the default table — `bconout5` writes to the screen which
  is always ready, but the symmetric stat call never reports
  ready. Programs writing through handle 5 should use `Bcostat(2)`.)
- **`-1L` semantics**: any non-zero return is documented as "ready",
  but EmuTOS specifically returns `-1L`, not the byte count or an
  enable mask.
- **`protect_v` register save** preserves d2/a2.

## Dependencies

- `chardev_init()` must have run.

Used by: `Cconos` (GEMDOS 0x10), `Cprnos` (0x11), `Cauxos` (0x13),
output-throttling code in user programs.

## Cross-references

- **Compendium:** "Bcostat" — agrees, with the swap noted.
- **Profibuch:** "Bcostat (8)" — agrees.
- **`docs/api/bios/_dispatcher.md`**.

## Configuration / version

- `BCONMAP_AVAILABLE`, `CONF_SERIAL_CONSOLE`.

## NewTOS implementation notes

(Empty during Phase 00.)
