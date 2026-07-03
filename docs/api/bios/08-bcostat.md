# `Bcostat` (BIOS 0x08)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/bios.c:1143` (`bcostat`).

## EmuTOS source

**Primary location:** `bios/bios.c:1143` — `LONG bcostat(WORD handle)`

**Helpers called:**
- `util/miscasm.S:213` — `protect_v(func)` — d2/a2-preserving indirect call.
- Per-handle output-status routines: `bios/parport.c:129` `bcostat0`
  (Centronics BUSY via MFP GPIP), `bios/serport.c:271` `bcostat1` (UART
  TX-empty), `bios/chardev.c:112` `bcostat2` (screen — always ready),
  `bios/midi.c` `bcostat3` (MIDI TX), `bios/ikbd.c` `bcostat4` (IKBD TX),
  `bios/chardev.c:50` `char_dummy` (returns 0).

**Headers / type definitions used:**
- `include/tosvars.h` — `bcostat_vec[8]`
- `bios/bios.h` — `boot_status`, `CHARDEV_AVAILABLE`

**System variables / globals read:**
- `boot_status` (`bios/bios.c:1149`)
- `bcostat_vec[handle]` (TOS var `0x55e`)
- `bconmap_root.maptab[]` — when `BCONMAP_AVAILABLE`

**System variables / globals written:** none in `bcostat`.

**Build configuration affecting this function:**
- `BCONMAP_AVAILABLE` — handle-remap stage for handles ≥ 6.
- `CONF_WITH_PRINTER_PORT` — without it, `bcostat0` returns 0 (busy).
- `CONF_SERIAL_CONSOLE` — makes `bcostat2` delegate to `bcostat(1)`.

---

## Source extraction summary

`Bcostat` reports whether an output device can accept the next byte. Identical
dispatcher shape as `Bconstat`: boot guard → optional Bconmap remap →
`bcostat_vec[handle]` via `protect_v`; else `0L`.

The default `bcostat_vec` (`bcostat_init[]`, `bios/chardev.c:38`) is
`{bcostat0, bcostat1, bcostat2, bcostat4, bcostat3, char_dummy, char_dummy,
char_dummy}` — note that **the MIDI (`bcostat3`) and IKBD (`bcostat4`) slots
are swapped** relative to their handle numbers, per the source comment
`/* note that IKBD and MIDI bcostat() are swapped! */` (`bios/chardev.c:40`).
So handle 3 → `bcostat4` (IKBD TX) and handle 4 → `bcostat3` (MIDI TX). This
swap is historical Atari behaviour applications depend on.

`bcostat2` (screen) returns `-1` unconditionally (always ready), unless
`CONF_SERIAL_CONSOLE` is set, in which case it returns `bcostat(1)` (serial
readiness). There is deliberately **no** `bcostat5` for the raw-screen output
channel — programs writing via handle 5 must query readiness with
`Bcostat(2)`.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x08
- **Stack frame on entry (m68k):**
  ```
  +2: handle (word)
   0: function_id (word) = 0x0008
  ```
- **Total parameter bytes:** 4
- **Native (AArch64) ABI:** ID `0x08` in `x8`; `handle` in `x0`; result
  (`0` or `-1`) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name     | Type / size |
  |------------|-------------|----------|-------------|
  | +2         | `x0`        | `handle` | `WORD` (16) |

---

## Inputs

| Position | Name     | Type | Size | Source location    | Notes |
|----------|----------|------|------|--------------------|-------|
| arg0     | `handle` | `WORD` | 2  | `bios/bios.c:1143` | `0..7` table-indexed; else `0L` |

Handle meanings — comment `bios/bios.c:1141` documents `0:PRT 1:AUX 2:CON
3:MID 4:KEYB`, but note the swap in the actual table:

| Handle | Device (output-status) | Default routine | Notes |
|-------:|------------------------|-----------------|-------|
| 0      | PRT / Centronics       | `bcostat0` | BUSY via MFP GPIP |
| 1      | AUX / RS-232           | `bcostat1` | UART TX-empty |
| 2      | CON / screen           | `bcostat2` | always `-1` (or `bcostat(1)`) |
| 3      | **IKBD TX**            | `bcostat4` | **swapped** |
| 4      | **MIDI TX**            | `bcostat3` | **swapped** |
| 5..7   | unused / Bconmap       | `char_dummy` → 0 | |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `-1L` = ready to accept a byte; `0L` = busy / unavailable /
  invalid handle. Only these two values.
- **Source location:** `bios/bios.c:1157-1161`; e.g. `bcostat2` returns `-1`
  (`bios/chardev.c:117`).

### Side effects on global state

None.

### Side effects on hardware

Reads a status register (MFP GPIP for printer BUSY, UART TX-empty for
serial/MIDI/IKBD). No writes.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `CHARDEV_AVAILABLE` clear | `bios/bios.c:1149` | `0L` |
| `handle` outside `0..7` and not in Bconmap range | `bios/bios.c:1160-1162` | `0L` |
| Bconmap index ≥ `maptabsize` | `bios/bios.c:1154` | `0L` |
| Handle wired to `char_dummy` (5..7) | `bios/chardev.c:50` | `0L` |
| Printer busy or `!CONF_WITH_PRINTER_PORT` (handle 0) | `bios/parport.c:129-141` | `0L` |

No distinct error code — `0L` means both "busy" and "unavailable/invalid".

**Error paths not in Compendium:** the boot-guard `0L`.

---

## Verification criteria

- **Nominal (screen ready):** `Bcostat(2) == -1` whenever
  `CHARDEV_AVAILABLE` is set (screen is always ready).
- **Nominal (printer busy):** with BUSY asserted (or
  `!CONF_WITH_PRINTER_PORT`), `Bcostat(0) == 0`.
- **Swap observable:** with MIDI TX ready but IKBD TX busy, `Bcostat(4)`
  (MIDI) and `Bcostat(3)` (IKBD) report the *swapped* device readiness.
- **Invalid handle:** `Bcostat(9)` → `0`.

---

## Edge cases discovered in source

1. **MIDI/IKBD status slots are swapped.** Handle 3 → `bcostat4` (IKBD) and
   handle 4 → `bcostat3` (MIDI) — the opposite of `bconout`/`bconin`
   (`bios/chardev.c:38-41`). This is intentional historical behaviour, kept for
   application compatibility.

2. **Screen status is a constant `-1`.** `bcostat2` (`bios/chardev.c:112-118`)
   returns `-1` (always ready), or `bcostat(1)` under `CONF_SERIAL_CONSOLE`.

3. **No `bcostat5`.** The raw-screen output handle (5) has no status routine;
   `bcostat_vec[5] == char_dummy` → `0L`. Programs must use `Bcostat(2)`.

4. **`bcostat0` returns `0` when the printer port is compiled out.** Under
   `!CONF_WITH_PRINTER_PORT` it always reports busy (`bios/parport.c:140`).

5. **Any non-zero means ready, but EmuTOS returns exactly `-1`.** It is not a
   byte count or enable mask.

---

## Dependencies

### Requires (must be initialized / functional)

- `chardev_init()` — `CHARDEV_AVAILABLE` + `bcostat_vec`.

### Used by

- GEMDOS `Cconos` (0x10), `Cprnos` (0x11), `Cauxos` (0x13); output-throttling
  loops in user programs.

---

## Related data structures

- `docs/api/bios/_state.md` — `bcostat_vec[8]` (with the swap documented)

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically documents
  the swap and the `-1`/`0` encoding.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `BCONMAP_AVAILABLE`, `CONF_SERIAL_CONSOLE`, `CONF_WITH_PRINTER_PORT`.

---

## NewTOS implementation notes

(Empty during Phase 00.)
