# `Bconout` (BIOS 0x03)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/bios.c:991` (`bconout`).

## EmuTOS source

**Primary location:** `bios/bios.c:991` — `LONG bconout(WORD handle, WORD what)`

**Helpers called:**
- `util/miscasm.S:240` — `protect_ww(func, handle, what)` — d2/a2-preserving
  indirect call for a two-WORD-argument handler.
- Per-handle output routines: `bios/parport.c:144` `bconout0` (Centronics,
  long-timeout, ctrl-C escapable), `bios/serport.c:289` `bconout1` (RS-232),
  `bios/chardev.c:91` `bconout2` (screen via `cputc`), `bios/midi.c`
  `bconout3` (MIDI out), `bios/ikbd.c` `bconout4` (IKBD command out),
  `bios/chardev.c:98` `bconout5` (raw screen via `ascii_out`),
  `bios/chardev.c:55` `charout_dummy` (handles 6,7 → 0).

**Headers / type definitions used:**
- `include/tosvars.h` — `bconout_vec[8]`
- `bios/bios.h` — `boot_status`, `CHARDEV_AVAILABLE`

**System variables / globals read:**
- `boot_status` (`bios/bios.c:997`)
- `bconout_vec[handle]` (TOS var `0x57e`)
- `bconmap_root.maptab[]` — when `BCONMAP_AVAILABLE`

**System variables / globals written:** none in `bconout`; handler 2/5
mutate the VT52/console state (cursor, escape parser) and screen RAM.

**Build configuration affecting this function:**
- `BCONMAP_AVAILABLE` — handle-remap stage for handles ≥ 6.
- `CONF_WITH_PRINTER_PORT` — without it, `bconout0`/`bcostat0` are inert.
- `CONF_SERIAL_CONSOLE` — makes `bconout5` also echo to handle 1.

---

## Source extraction summary

`Bconout` writes one character to an output device. Same three-stage
dispatcher as the other char functions, with one difference: the handlers
receive **both** the original `handle` and the character (`protect_ww`
forwards two WORDs), so a single C function can service several handles
(e.g. `bconout5` can delegate to `bconout(1, …)`).

Default routines (`bconout_init[]`, `bios/chardev.c:43`):
`{bconout0, bconout1, bconout2, bconout3, bconout4, bconout5, charout_dummy,
charout_dummy}`. Unlike the status/input tables, all of 0..5 have real
routines here.

Return-value convention is **not** the `-1/0` of the status calls. Screen and
serial/MIDI handlers return `1L` on success (`bconout2` → `return 1L`,
`bios/chardev.c:94`). The parallel handler `bconout0` is the exception: it
returns `0L` when the printer is busy/timed-out and the `prnout()` result
otherwise (`bios/parport.c:144-172`). This asymmetry is real and observable.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x03
- **Stack frame on entry (m68k):**
  ```
  +4: what   (word)
  +2: handle (word)
   0: function_id (word) = 0x0003
  ```
- **Total parameter bytes:** 6
- **Native (AArch64) ABI:** ID `0x03` in `x8`; `handle` in `x0`, `what` in
  `x1`; result in `x0`.

  | m68k stack | AArch64 reg | Name     | Type / size |
  |------------|-------------|----------|-------------|
  | +2         | `x0`        | `handle` | `WORD` (16-bit) |
  | +4         | `x1`        | `what`   | `WORD` (16-bit) |

---

## Inputs

| Position | Name     | Type | Size | Source location   | Notes |
|----------|----------|------|------|-------------------|-------|
| arg0     | `handle` | `WORD` | 2  | `bios/bios.c:991` | `0..7` table-indexed |
| arg1     | `what`   | `WORD` | 2  | `bios/bios.c:991` | byte to output; only low 8 bits used (`bconout5` does `LOBYTE(ch)`, `bios/chardev.c:104`) |

Handle meanings (default `bconout_vec`):

| Handle | Device (output)                | Default routine |
|-------:|--------------------------------|-----------------|
| 0      | PRT / Centronics               | `bconout0` |
| 1      | AUX / RS-232                   | `bconout1` |
| 2      | CON / screen (VT52)            | `bconout2` |
| 3      | MIDI out                       | `bconout3` |
| 4      | IKBD command out               | `bconout4` |
| 5      | raw screen (no VT52)           | `bconout5` |
| 6,7    | unused / Bconmap               | `charout_dummy` → 0 |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** device-dependent (see table). `1L` = byte accepted for most
  handlers; `0L` = printer busy (handle 0 only), or boot-guard / invalid
  handle.
- **Source location:** the handler; e.g. `bios/chardev.c:94` (`return 1L`).

| Handler | Return | Source |
|---------|--------|--------|
| `bconout0` (PRT) | `0L` on busy/timeout; `prnout(c)` result on send | `bios/parport.c:144-172` |
| `bconout1` (AUX) | `1L` after byte transmitted | `bios/serport.c:289` |
| `bconout2` (CON) | `1L` | `bios/chardev.c:91-95` |
| `bconout3` (MIDI) | `1L` | `bios/midi.c` |
| `bconout4` (IKBD) | `1L` | `bios/ikbd.c` |
| `bconout5` (raw)  | `1L` | `bios/chardev.c:98-106` |
| `charout_dummy` (6,7) | `0L` | `bios/chardev.c:55` |
| boot too early / invalid handle | `0L` | `bios/bios.c:997,1008-1010` |

### Side effects on global state

- Handle 2/5: VT52 console state (cursor position, escape-parser state) and
  screen RAM change via `cputc`/`ascii_out` (`bios/conout.c`).

### Side effects on hardware

- Handle 0/1/3/4: writes the byte to the printer/UART/ACIA data register.
- Handle 2/5: writes glyph cells to the framebuffer.
- Handle 0: may spin up to a long timeout waiting on the Centronics BUSY line.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `CHARDEV_AVAILABLE` clear | `bios/bios.c:997` | `0L` |
| `handle` outside `0..7` and not in Bconmap range | `bios/bios.c:1008-1010` | `0L` |
| Handle 0 printer busy / timed out | `bios/parport.c:171` | `0L` |
| Handles 6,7 (`charout_dummy`) | `bios/chardev.c:55` | `0L` |

`bconout0`'s `0L` is a genuine "not sent, retry" signal, distinct from the
other handlers' guaranteed success — the only handler where the return value
carries information a caller must act on.

**Error paths not in Compendium:** the printer-busy `0L` return and the
boot-guard `0L`.

---

## Verification criteria

- **Nominal (screen):** `Bconout(2, 'A')` returns `1` and places 'A' at the
  cursor (advancing the VT52 cursor).
- **Nominal (raw):** `Bconout(5, 'A')` returns `1` and writes 'A' literally
  (no VT52 escape interpretation).
- **Printer busy:** with the Centronics BUSY line asserted (or
  `!CONF_WITH_PRINTER_PORT`), `Bconout(0, 'A')` returns `0`.
- **Invalid handle:** `Bconout(6, 'A')` (no Bconmap) returns `0`.

---

## Edge cases discovered in source

1. **Handlers get the handle too.** `protect_ww` forwards `handle` *and*
   `what` (`bios/bios.c:1005,1009`). This is why `bconout5` can call
   `bconout(1, ch)` to mirror to serial (`bios/chardev.c:100-103`) — it is not
   limited to its own channel.

2. **Handle 5 is "raw screen", not a separate device.** `bconout5` writes the
   same screen as handle 2 but via `ascii_out(LOBYTE(ch))` with **no** VT52
   escape parsing (`bios/chardev.c:104`). There is no `bcostat5` — the
   symmetric status must use `Bcostat(2)`.

3. **Only the low byte is emitted.** `bconout5` masks with `LOBYTE`; the other
   handlers likewise ignore the high byte of `what`.

4. **Handle 4 is a keyboard-controller command channel**, not user text —
   `bconout4` sends bytes to the IKBD. The higher-level API is `Ikbdws`
   (XBIOS 0x19).

5. **Printer output can block for a long time.** `bconout0` waits up to
   `LONG_TIMEOUT` for BUSY to clear, polling `bconin2()` for ctrl-C to abort
   (`bios/parport.c:159-166`) — an EmuTOS convenience over Atari TOS.

---

## Dependencies

### Requires (must be initialized / functional)

- `chardev_init()` — `CHARDEV_AVAILABLE` + `bconout_vec`.
- Handle 2/5: screen initialised (`v_bas_ad`, `cputc`/`ascii_out` state).
- Handle 1: `init_serport()`. Handle 0: `parport_init()` (+
  `CONF_WITH_PRINTER_PORT`).

### Used by

- Every GEMDOS output call (`Cconout`, `Cauxout`, `Cprnout`, `Cconws`, …);
  kernel `kprintf()` (via handle 2); AES/VDI text output.

---

## Related data structures

- `docs/api/bios/_state.md` — `bconout_vec[8]`, VT52 console state
- `docs/api/_line-a.md` — VT52 cursor / colour variables handler 2 touches

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: agrees on
  the `(handle, char)` signature.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_VT52`: handle-2 escape parsing.
- `CONF_SERIAL_CONSOLE`: `bconout5` also writes handle 1.
- `CONF_WITH_PRINTER_PORT`: handle 0 active (else `0L`).
- `BCONMAP_AVAILABLE`: handles ≥ 6 via remap table.

---

## NewTOS implementation notes

(Empty during Phase 00.)
