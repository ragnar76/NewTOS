# `Cauxos` (GEMDOS 0x13)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:195` (`xauxostat`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the truth-valued status return (`-1` ready, `0` not).
> Reference: the serial output-status driver.

## EmuTOS source

**Primary location:** `bdos/console.c:195` — `long xauxostat(void)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x13` =
`xauxostat`, `stdio_typ = 0x82` (stdaux), `wparms = 0` (`bdos/bdosmain.c:192`).
When stdaux is redirected to a file (`run->p_uft[2] > 0`) the dispatcher returns
`-1L` unconditionally — see Edge case 1.

**Helpers called:**
- BIOS `Bcostat(1)` (0x08) — serial output-ready status.

**Headers / type definitions used:**
- `bdos/console.h:24,36` — `H_Aux = -2`, `HXFORM(h) = 3+h` (→ BIOS handle 1).

**System variables / globals read:**
- `run->p_uft[2]` (`bdos/console.c:197`) — stdaux device.

**System variables / globals written:** none.

**Build configuration affecting this function:** none directly (transitive
serial back-end selection: `CONF_WITH_MFP` / `CONF_WITH_SCC`).

---

## Source extraction summary

`Cauxos` reports whether the auxiliary (serial) output device is ready to accept
a byte. It is a **pure pass-through**: `xauxostat` resolves stdaux from
`run->p_uft[2]` (default `H_Aux = -2` → BIOS handle 1) and returns
`Bcostat(HXFORM(p_uft[2]))`. `Bcostat(1)` returns `-1` when the UART transmit
register can accept a byte, `0` when it cannot. A redirected stdaux returns
`-1L` unconditionally.

Note that `xauxostat` reads `p_uft[2]` — the **stdaux** handle — the same slot
`Cauxin`/`Cauxout`/`Cauxis` use.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x13
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0013
  ```
  No parameters.
- **Total parameter bytes:** 2 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x13` in `x8`; no argument registers; LONG
  result sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None. The device is `run->p_uft[2]`, not a parameter.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `-1L` if the AUX output device is ready, `0L` if not.
- **Source location:** `bdos/console.c:197` (forwards `Bcostat`).

### Side effects on global state

- None.

### Side effects on hardware

- Reads the serial transmit-ready flag (via BIOS).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

**Error paths not in Compendium:** none.

---

## Verification criteria

- **Ready:** with the UART able to accept output, `Cauxos()` returns `-1L`.
- **Not ready:** with the transmit buffer full, `Cauxos()` returns `0L`.
- **Redirected stdaux:** with `run->p_uft[2]` forced to a file, `Cauxos()`
  returns `-1L` unconditionally (`bdos/bdosmain.c:561-564`).

---

## Edge cases discovered in source

1. **Redirected stdaux is always ready.** The dispatcher's `case 19` returns
   `-1L` (`bdos/bdosmain.c:561-564`).

2. **Uses the stdaux handle for both directions.** `Cauxos` and `Cauxis`/
   `Cauxin`/`Cauxout` all key off `p_uft[2]`.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS serial output status `Bcostat(1)`.
- `stdhdl_init()` (sets `p_uft[2] = H_Aux`).

### Used by

- Programs throttling RS-232 output before each `Cauxout`.

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

- AUX BIOS device selection: `CONF_WITH_MFP`, `CONF_WITH_SCC`.

---

## NewTOS implementation notes

(Empty during Phase 00.)
