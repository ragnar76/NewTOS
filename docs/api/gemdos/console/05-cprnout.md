# `Cprnout` (GEMDOS 0x05)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:340` (`xprtout`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, pass-through of the byte, the ready/busy return (0 on
> timeout, non-zero on accept). Reference: the parport timeout algorithm.

## EmuTOS source

**Primary location:** `bdos/console.c:340` — `long xprtout(int ch)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x05` =
`xprtout`, `stdio_typ = 0x83` (stdprn), `wparms = 1` (`bdos/bdosmain.c:160`).
When stdprn is redirected (`run->p_uft[3] > 0`) the dispatcher writes the byte
to the file and returns `0` without entering this body — see Edge case 2.

**Helpers called:**
- BIOS `Bconout(0,ch)` (0x03) → `bios/parport.c:144` `bconout0` — printer output
  with a `hz_200` timeout; returns `0` on timeout, non-zero on accept.

**Headers / type definitions used:**
- `bdos/console.h:25,36` — `H_Print = -3`, `HXFORM(h) = 3+h` (→ BIOS handle 0).

**System variables / globals read:**
- `run->p_uft[3]` (`bdos/console.c:342`).

**System variables / globals written:** none.

**Build configuration affecting this function:** `CONF_WITH_PRINTER_PORT` —
when disabled, `bconout0` immediately returns `0L` (no printer).

---

## Source extraction summary

`Cprnout` writes one character to the printer (Centronics) device. It is a
**pure pass-through**: `xprtout` resolves stdprn from `run->p_uft[3]` (default
`H_Print = -3` → BIOS handle 0) and returns `Bconout(HXFORM(p_uft[3]), ch)`.

There is no tab expansion or column tracking. The distinguishing feature versus
`Cconout`/`Cauxout` is the return value: the printer `bconout0`
(`bios/parport.c:144`) polls the parport BUSY line for up to a long `hz_200`
timeout; it returns the (non-zero) result of `prnout(c)` if the byte is
accepted, or `0L` if the printer stays busy until the timeout (or if
`CONF_WITH_PRINTER_PORT` is off). So `Cprnout` surfaces printer ready/busy to
the caller through its return, unlike the console and AUX output calls.

Only the low 8 bits of `ch` are printed.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x05
- **Stack frame on entry (m68k):**
  ```
  +2: ch (word)
   0: function_id (word) = 0x0005
  ```
- **Total parameter bytes:** 4 → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x05` in `x8`; `ch` in `x0`; LONG result
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `ch` | `WORD` (16) — low 8 bits used |

---

## Inputs

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| arg0     | `ch` | `WORD` | 2  | `bdos/console.c:340` | byte to print; low 8 bits |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** non-zero if the printer accepted the byte; `0` if the printer
  was busy for the whole timeout (or printer port disabled).
- **Source location:** `bdos/console.c:342` (forwards `bconout0`).

### Side effects on global state

- None in the BDOS layer.

### Side effects on hardware

- Drives the Centronics data/strobe lines (via BIOS `prnout`).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Printer busy for the whole timeout | `bios/parport.c:144` | `0` |
| `CONF_WITH_PRINTER_PORT` disabled | `bios/parport.c` | `0` |
| Byte accepted | `bios/parport.c` | non-zero |

**Error paths not in Compendium:** the `0` result is a busy/timeout signal, not
a negative TOS error code.

---

## Verification criteria

- **Nominal:** with a ready printer, `Cprnout('A')` prints `0x41` and returns
  non-zero.
- **Busy:** with the printer BUSY for the whole timeout, `Cprnout` returns `0`.
- **No printer / disabled:** `Cprnout` returns `0`.
- **Redirected stdprn:** with `run->p_uft[3]` forced to a file, the byte is
  written to the file and the call returns `0` (`bdos/bdosmain.c:521-528`) —
  note this `0` collides with the busy signal.

---

## Edge cases discovered in source

1. **Return surfaces printer ready/busy.** Unlike `Cconout`/`Cauxout`,
   `Cprnout` returns `0` on a busy/timeout printer and non-zero on accept.

2. **Ctrl-C can abort a stuck printer wait.** `bconout0` polls the keyboard
   during its long timeout and breaks out if the user presses Ctrl-C
   (`bios/parport.c` — quick-out), returning `0` — an EmuTOS convenience over
   Atari TOS's blocking wait.

3. **Redirected stdprn also returns `0`.** The dispatcher's file-write path
   returns `0`, indistinguishable by return value from a busy printer.

4. **No buffering / no tab expansion.** One byte per call, raw.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS printer output `Bconout(0,…)` = `bconout0` (`CONF_WITH_PRINTER_PORT`).
- `stdhdl_init()` (sets `p_uft[3] = H_Print`).

### Used by

- Programs producing printer output; higher-level print spooling.

---

## Related data structures

- `docs/api/bios/03-bconout.md` — the underlying byte-output BIOS call.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the
  busy-returns-0 semantics — consistent with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_PRINTER_PORT = 0` removes the parport driver; `Cprnout` returns `0`
  permanently. NewTOS targets no physical printer in early phases.

---

## NewTOS implementation notes

(Empty during Phase 00.)

FS11 / IO01 prep (2026-10-13, user decision 1): see `docs/api/gemdos/file/3f-fread.md` "NewTOS implementation notes" for the device table.
`Cprnout(ch)` is a pass-through to `Bconout(0, ch)` of the PRN entry. With the
absent-device entry it returns **0 immediately** ("printer not ready/timed
out"): no 30 s wait. This equals EmuTOS without `CONF_WITH_PRINTER_PORT`, where
`bconout0` returns 0 at once (`bios/parport.c:144-172`, body gated, `:170-171`).
Programs that retry on 0 will loop; that is the same behaviour as EmuTOS on a
machine without a printer port.
