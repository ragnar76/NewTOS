# `Cauxout` (GEMDOS 0x04)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:331` (`xauxout`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, pass-through of the byte, the `Bconout` return.
> Reference: the serial driver internals.

## EmuTOS source

**Primary location:** `bdos/console.c:331` — `long xauxout(int ch)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x04` =
`xauxout`, `stdio_typ = 0x82` (stdaux), `wparms = 1` (`bdos/bdosmain.c:159`).
When stdaux is redirected (`run->p_uft[2] > 0`) the dispatcher writes the byte
to the file and returns `0` without entering this body — see Edge case 2.

**Helpers called:**
- BIOS `Bconout(1,ch)` (0x03).

**Headers / type definitions used:**
- `bdos/console.h:24,36` — `H_Aux = -2`, `HXFORM(h) = 3+h` (→ BIOS handle 1).

**System variables / globals read:**
- `run->p_uft[2]` (`bdos/console.c:333`).

**System variables / globals written:** none.

**Build configuration affecting this function:** none directly (transitive:
`CONF_WITH_MFP` / `CONF_WITH_SCC` for the serial back-end).

---

## Source extraction summary

`Cauxout` writes one character to the auxiliary (serial) device. It is a **pure
pass-through**: `xauxout` resolves stdaux from `run->p_uft[2]` (default
`H_Aux = -2` → BIOS handle 1) and returns `Bconout(HXFORM(p_uft[2]), ch)`.

There is no tab expansion, no `glbcolumn` tracking, and no `conbrk` flow-control
probe — those belong to the cooked CON path only. Only the low 8 bits of `ch`
are written. The value returned is whatever the serial `Bconout` returns; the
EmuTOS serial `bconout1` returns a non-zero value (it waits for transmit-ready
and then writes), so in practice the return is non-zero, but this is a BIOS
driver property, not a `Cauxout` contract.

Because `Bconout` waits for the transmit register to be free before writing,
`Cauxout` is effectively blocking on a full transmit buffer.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x04
- **Stack frame on entry (m68k):**
  ```
  +2: ch (word)
   0: function_id (word) = 0x0004
  ```
- **Total parameter bytes:** 4 → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x04` in `x8`; `ch` in `x0`; LONG result
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `ch` | `WORD` (16) — low 8 bits used |

---

## Inputs

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| arg0     | `ch` | `WORD` | 2  | `bdos/console.c:331` | byte to send; low 8 bits |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** the serial `Bconout` return (driver-defined; normally non-zero
  after the byte is queued/sent).
- **Source location:** `bdos/console.c:333`.

### Side effects on global state

- None in the BDOS layer.

### Side effects on hardware

- Writes the byte to the serial transmit register (after waiting for
  transmit-ready).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no GEMDOS error return) | — | — |

**Error paths not in Compendium:** none at the GEMDOS level.

---

## Verification criteria

- **Nominal:** `Cauxout('A')` sends `0x41` on the AUX device.
- **No tab expansion:** `Cauxout('\t')` sends a literal 0x09 (no space
  expansion, no column tracking).
- **Redirected stdaux:** with `run->p_uft[2]` forced to a file, the byte is
  written to the file and the call returns `0` (`bdos/bdosmain.c:521-528`).

---

## Edge cases discovered in source

1. **No cooking.** No tab expansion, `glbcolumn`, or `conbrk` — unlike
   `Cconout`. AUX output is raw.

2. **Effectively blocking.** The serial `Bconout` waits for transmit-ready, so
   `Cauxout` blocks until the UART can accept the byte.

3. **Redirected stdaux returns `0`.** The dispatcher `xwrite`s one byte and
   returns `0`, differing from the console path's driver return value.

4. **High byte of `ch` ignored.**

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS serial output `Bconout(1,…)` and its driver (`init_serport`).
- `stdhdl_init()` (sets `p_uft[2] = H_Aux`).

### Used by

- Programs writing to RS-232 / the current AUX device.

---

## Related data structures

- `docs/api/bios/03-bconout.md` — the underlying byte-output BIOS call.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- AUX BIOS device selection: `CONF_WITH_MFP`, `CONF_WITH_SCC`.

---

## NewTOS implementation notes

(Empty during Phase 00.)
