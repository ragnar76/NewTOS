# `Cconos` (GEMDOS 0x10)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:168` (`xconostat`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the truth-valued status return (`-1` ready, `0` not).
> Reference: the BIOS output-status driver.

## EmuTOS source

**Primary location:** `bdos/console.c:168` — `long xconostat(void)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x10` =
`xconostat`, `stdio_typ = 0x81` (stdout), `wparms = 0` (`bdos/bdosmain.c:189`).
When stdout is redirected to a file (`run->p_uft[1] > 0`) the dispatcher returns
`-1L` unconditionally (a file is always writable) — see Edge case 1.

**Helpers called:**
- BIOS `Bcostat(2)` (0x08) — output-ready status for the console device.

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM(h) = 3+h`.

**System variables / globals read:**
- `run->p_uft[1]` (`bdos/console.c:170`) — stdout device.

**System variables / globals written:** none.

**Build configuration affecting this function:** none directly.

---

## Source extraction summary

`Cconos` reports whether the console output device is ready to accept a byte. It
is a **pure pass-through**: `xconostat` resolves stdout from `run->p_uft[1]`
(default `H_Console` → BIOS handle 2) and returns `Bcostat(HXFORM(p_uft[1]))`.
`Bcostat` returns `-1` (non-zero) when the device can accept a byte and `0`
when it cannot. When stdout is redirected to a file, the dispatcher answers
`-1L` unconditionally.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x10
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0010
  ```
  No parameters.
- **Total parameter bytes:** 2 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x10` in `x8`; no argument registers; LONG
  result sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None. The device is `run->p_uft[1]`, not a parameter.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `-1L` if the console output device is ready, `0L` if not.
- **Source location:** `bdos/console.c:170` (forwards `Bcostat`).

### Side effects on global state

- None.

### Side effects on hardware

- Reads the console output-ready flag (via BIOS).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

**Error paths not in Compendium:** none.

---

## Verification criteria

- **Ready:** with the console able to accept output, `Cconos()` returns `-1L`.
- **Not ready:** with the console output stalled, `Cconos()` returns `0L`.
- **Redirected stdout:** with `run->p_uft[1]` forced to a file, `Cconos()`
  returns `-1L` unconditionally (`bdos/bdosmain.c:561-564`).

---

## Edge cases discovered in source

1. **Redirected stdout is always ready.** The dispatcher's `case 16` returns
   `-1L` without consulting anything (`bdos/bdosmain.c:561-564`).

2. **Truth-valued, canonically `-1`.** The non-zero value is `Bcostat`'s; test
   for non-zero, not `== -1`.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console output status `Bcostat(2)`.
- `stdhdl_init()` (sets `p_uft[1] = H_Console`).

### Used by

- Programs throttling output to a slow console.

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

- Configuration-independent.

---

## NewTOS implementation notes

(Empty during Phase 00.)
