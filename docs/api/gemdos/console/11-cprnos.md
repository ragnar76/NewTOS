# `Cprnos` (GEMDOS 0x11)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:177` (`xprtostat`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the truth-valued status return (`-1` ready, `0` not).
> Reference: the parport-status driver.

## EmuTOS source

**Primary location:** `bdos/console.c:177` — `long xprtostat(void)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x11` =
`xprtostat`, `stdio_typ = 0x83` (stdprn), `wparms = 0` (`bdos/bdosmain.c:190`).
When stdprn is redirected to a file (`run->p_uft[3] > 0`) the dispatcher returns
`-1L` unconditionally — see Edge case 1.

**Helpers called:**
- BIOS `Bcostat(0)` (0x08) — printer output-ready status.

**Headers / type definitions used:**
- `bdos/console.h:25,36` — `H_Print = -3`, `HXFORM(h) = 3+h` (→ BIOS handle 0).

**System variables / globals read:**
- `run->p_uft[3]` (`bdos/console.c:179`) — stdprn device.

**System variables / globals written:** none.

**Build configuration affecting this function:** `CONF_WITH_PRINTER_PORT` — when
disabled, the BIOS printer `Bcostat(0)` reports not-ready.

---

## Source extraction summary

`Cprnos` reports whether the printer is ready to accept a byte. It is a **pure
pass-through**: `xprtostat` resolves stdprn from `run->p_uft[3]` (default
`H_Print = -3` → BIOS handle 0) and returns `Bcostat(HXFORM(p_uft[3]))`. For the
Centronics printer, `Bcostat(0)` returns `-1` when the parport BUSY line is low
and SELECT is high (ready), else `0`. With `CONF_WITH_PRINTER_PORT` disabled it
reports not-ready. A redirected stdprn returns `-1L` unconditionally.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x11
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0011
  ```
  No parameters.
- **Total parameter bytes:** 2 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x11` in `x8`; no argument registers; LONG
  result sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None. The device is `run->p_uft[3]`, not a parameter.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `-1L` if the printer is ready, `0L` if not.
- **Source location:** `bdos/console.c:179` (forwards `Bcostat`).

### Side effects on global state

- None.

### Side effects on hardware

- Reads the parport BUSY/SELECT status lines (via BIOS).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

**Error paths not in Compendium:** none.

---

## Verification criteria

- **Ready:** with a ready printer, `Cprnos()` returns `-1L`.
- **Busy:** with the printer BUSY, `Cprnos()` returns `0L`.
- **No printer / disabled (`CONF_WITH_PRINTER_PORT = 0`):** `Cprnos()` returns
  `0L`.
- **Redirected stdprn:** with `run->p_uft[3]` forced to a file, `Cprnos()`
  returns `-1L` unconditionally (`bdos/bdosmain.c:561-564`).

---

## Edge cases discovered in source

1. **Redirected stdprn is always ready.** The dispatcher's `case 17` returns
   `-1L` (`bdos/bdosmain.c:561-564`).

2. **Disabled parport reports not-ready permanently.** With
   `CONF_WITH_PRINTER_PORT = 0`, `Bcostat(0)` is `0`.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS printer output status `Bcostat(0)` (`CONF_WITH_PRINTER_PORT`).
- `stdhdl_init()` (sets `p_uft[3] = H_Print`).

### Used by

- Well-behaved printer-output loops that poll before each `Cprnout`.

---

## Related data structures

- `docs/api/bios/08-bcostat.md` — the underlying output-status BIOS call.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `-1`/`0` status
  — consistent with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_PRINTER_PORT` gates the underlying driver. NewTOS targets no
  physical printer in early phases.

---

## NewTOS implementation notes

(Empty during Phase 00.)
