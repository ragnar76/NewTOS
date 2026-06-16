# `Bconin` — BIOS 0x02

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bios/bios.c:958` (`bconin`).

## EmuTOS source

- **Primary:** `bios/bios.c:958` — `bconin(WORD handle)`
- **Helpers:** `protect_v()` (`util/miscasm.S:213`)
- **Tables:** `bconin_vec[8]`, `bconmap_root.maptab[]`
- **Globals read:** `boot_status & CHARDEV_AVAILABLE`
- **Build config:** `BCONMAP_AVAILABLE`.

## Source extraction

```c
LONG bconin(WORD handle)
{
    if (!(boot_status & CHARDEV_AVAILABLE))
        return 0;

#if BCONMAP_AVAILABLE
    WORD map_index = handle - BCONMAP_START_HANDLE;
    if (map_index >= bconmap_root.maptabsize) return 0L;
    if (map_index >= 0)
        return protect_v(bconmap_root.maptab[map_index].Bconin);
#endif

    if ((handle >= 0) && (handle <= 7))
        return protect_v(bconin_vec[handle]);
    return 0L;
}
```

Identical structure to `Bconstat`; only the table differs.

`bconin` is **blocking** for handles whose underlying driver blocks
(default: 1=AUX, 2=CON, 3=MIDI). Drivers spin on hardware-ready bits.

## Trap interface

| | |
|--|--|
| Trap | 13 |
| Function ID | `0x0002` |
| WORD args | 1 |

## Inputs

| Pos | Name     | Type | Range                                       |
|-----|----------|------|---------------------------------------------|
| 0   | `handle` | WORD | 0=PRT, 1=AUX, 2=CON, 3=MIDI-in, 4=IKBD-in, 5..7 |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Encoding (CON, h=2) | high word: GSX-2.0 IKBD scancode (low byte of high word). low word: high byte = 0, low byte = ASCII (or 0 if non-ASCII). |
| Encoding (AUX, h=1) | character in low byte; upper bytes 0. |
| Encoding (MIDI, h=3) | byte in low byte; upper bytes 0. |

Comment in `bios/bios.c:945-951` documents the encoding.

| Failure mode                              | Value |
|-------------------------------------------|-------|
| `boot_status & CHARDEV_AVAILABLE == 0`    | `0L`  |
| invalid handle                            | `0L`  |
| `bconin_vec[handle] == char_dummy`        | `0L`  |

## Errors

No separate error code — `0L` doubles as "no device" *and* a legitimate
NUL byte. Callers must use `Bconstat` first when ambiguity matters.

## Edge cases

- **Default `bconin_vec[]`** (`bios/chardev.c:35`):
  ```
  [0]=bconin0, [1]=bconin1, [2]=bconin2, [3]=bconin3, [4..7]=char_dummy
  ```
  - `bconin0` (parport input) — reads the parport status / data lines;
    most printer cables don't drive these.
  - `bconin1` — RS-232 input, blocks on UART receive flag.
  - `bconin2` — IKBD queue dequeue; folds in mouse-button events as
    pseudo-keycodes per Atari convention.
  - `bconin3` — MIDI input.
- **Mouse / function key encoding** (handle 2): the IKBD driver maps
  some events into the upper-word scancode field while leaving the
  ASCII byte zero — applications use the scancode to distinguish
  cursor keys, F1..F10, etc.
- **`protect_v` register save** preserves d2/a2 — same caveat as
  `Bconstat`.
- **No timeout**: a hardware fault that prevents data from arriving
  blocks the caller indefinitely. The IKBD driver guards against this
  via the IKBD reset path; AUX has no such guard.
- **System-call invariant**: even though `bconin` is blocking, BIOS
  exception priorities mean keyboard, MFP timer, and VBL interrupts
  continue to run; the system does not freeze.

## Dependencies

- `chardev_init()` must have run.
- Per-handle driver active and able to deliver bytes (e.g., IKBD
  initialised; UART configured).

Used by: `Cconin`, `Cnecin`, `Crawcin`, `Cauxin`, `Crawio` (read mode)
— all GEMDOS console-input paths.

## Cross-references

- **Compendium:** "Bconin" — agrees on encoding and blocking.
- **Profibuch:** "Bconin (2)" — agrees.
- **`docs/api/bios/_dispatcher.md`**.

## Configuration / version

- `BCONMAP_AVAILABLE`.
- `CONF_SERIAL_CONSOLE`: keyboard input still reads via handle 2; the
  serial-console mode primarily affects output, but ANSI input on
  serial is supported when `CONF_SERIAL_CONSOLE_ANSI`.

## NewTOS implementation notes

(Empty during Phase 00.)
