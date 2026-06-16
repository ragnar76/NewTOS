# `Bconstat` — BIOS 0x01

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bios/bios.c:914` (`bconstat`).

## EmuTOS source

- **Primary:** `bios/bios.c:914` — `bconstat(WORD handle)`
- **Helpers:** `protect_v()` (`util/miscasm.S:213`) — d2/a2-preserving call thunk
- **Tables consulted:** `bconstat_vec[8]` (initialised by
  `chardev_init`), `bconmap_root.maptab[]` (when
  `BCONMAP_AVAILABLE` and handle ≥ `BCONMAP_START_HANDLE`)
- **Globals read:** `boot_status` (must have `CHARDEV_AVAILABLE` =
  `0x10` set, see `bios/bios.h:89`)
- **Build config:** `BCONMAP_AVAILABLE` adds the per-handle remap table.

## Source extraction

```c
LONG bconstat(WORD handle)
{
    if (!(boot_status & CHARDEV_AVAILABLE))
        return 0;

#if BCONMAP_AVAILABLE
    WORD map_index = handle - BCONMAP_START_HANDLE;
    if (map_index >= bconmap_root.maptabsize) return 0L;
    if (map_index >= 0)
        return protect_v(bconmap_root.maptab[map_index].Bconstat);
#endif

    if ((handle >= 0) && (handle <= 7))
        return protect_v(bconstat_vec[handle]);
    return 0L;
}
```

Three-stage dispatch:
1. Boot guard — return `0L` until `chardev_init()` has run
   (`bios/bios.c:430` sets the flag).
2. `Bconmap` remap (handle ≥ 6 in default config) — table lookup into
   `bconmap_root.maptab`. Out-of-range → `0L`.
3. Per-handle vector table — handles 0..7 → `bconstat_vec[handle]`.

`protect_v` is an asm wrapper that pushes/pops `d2`/`a2` around the
indirect call, because original Atari hardware drivers were written in
assembly and used `d0–d2/a0–a2` as scratch — non-compliant with the C
ABI which preserves `d2`/`a2` across calls.

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x0001` |
| WORD args | 1 |
| Stack frame | `+2: handle (WORD)`, `+0: 0x0001 (fn)` |

## Inputs

| Pos | Name     | Type | Range / meaning                                |
|-----|----------|------|------------------------------------------------|
| 0   | `handle` | WORD | 0=PRT, 1=AUX, 2=CON, 3=MIDI-in, 4=IKBD-in, 5=raw-CON, 6,7=Bconmap |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |

| State                           | Value |
|---------------------------------|-------|
| Input device has data ready     | `-1L` |
| No data available               | `0L`  |
| Boot stage before `chardev_init`| `0L`  |
| `handle` out of `[0,7]` and not in `bconmap` range | `0L` |
| `bconmap` index out of bounds   | `0L`  |

## Errors

None as a separate error code; the "0L" return doubles as both
"not ready" and "invalid handle" / "device unavailable".

## Edge cases

- **`handle` < 0** without `BCONMAP_AVAILABLE` → `0L` (final guard).
  With `BCONMAP_AVAILABLE`, `map_index = handle - 6` is *more* negative
  → still `0L` via the `map_index < 0` fall-through.
- **Default `bconstat_vec[]` initialisation** (`bios/chardev.c:31-32`):
  ```
  [0]=char_dummy, [1]=bconstat1, [2]=bconstat2, [3]=bconstat3,
  [4..7]=char_dummy
  ```
  - `char_dummy` returns `0L`.
  - `bconstat1` → RS-232 receiver-ready (`bios/serport.c`).
  - `bconstat2` → IKBD queue non-empty (`bios/bios.c:bconstat2`).
  - `bconstat3` → MIDI receiver-ready (`bios/midi.c`).
  - **No `bconstat4` (IKBD)** in this table because IKBD reads come via
    handle 2 (CON) which already returns IKBD events; handle 4 input
    is a Bconmap-era extension.
- **Vector overwrite**: applications may install custom drivers by
  writing `bconstat_vec[h]`. The TOS standard exposes the table at a
  documented address (low memory area); EmuTOS's `bconstat_vec` is
  declared in `tosvars.h`/`tosvars.S` at the standard fixed offset.
- **`protect_v` does not catch faults** in the called handler. A
  buggy driver crashes BIOS.

## Dependencies

- `chardev_init()` must have run (sets `CHARDEV_AVAILABLE` in
  `boot_status`).
- `bconstat_vec[handle]` must point at a callable.

Used by: `Cconis` (GEMDOS 0x0B), `Cauxis` (0x12), `constat()`
(`bdos/console.c`), every typeahead-aware GEMDOS reader.

## Cross-references

- **Compendium:** "Bconstat" — agrees, including `-1L`/`0L` encoding.
- **Profibuch:** "Bconstat (1)" — agrees.
- **`docs/api/bios/_dispatcher.md`** — trap-13 trampoline.
- **`docs/api/bios/_state.md`** (TBD D7) — `bconstat_vec[]` table layout.

## Configuration / version

- `BCONMAP_AVAILABLE`: enables the per-handle remap, used by RS-232
  multiplexers on TT/Falcon.
- `CONF_SERIAL_CONSOLE`: leaves the `bconstat_vec` initialiser
  unchanged but redirects underlying CON I/O at the conout level.

## NewTOS implementation notes

(Empty during Phase 00. Filled in during Phase 02 / Phase 07.)
