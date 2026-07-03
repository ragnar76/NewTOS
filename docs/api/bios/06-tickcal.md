# `Tickcal` (BIOS 0x06)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/bios.c:1096` (`tickcal`).

## EmuTOS source

**Primary location:** `bios/bios.c:1096` — `LONG tickcal(void)`
(body: `return 20L;`).

**Helpers called:** none.

**Headers / type definitions used:** none.

**System variables / globals read/written:** none — the return is a literal
constant `20L`.

**Build configuration affecting this function:** none. The constant does not
vary with any `CONF_*` flag; it is hard-coded even if the underlying timer
were reconfigured.

---

## Source extraction summary

`Tickcal` returns the period, in milliseconds, of the system tick that drives
GEMDOS/AES timing. On Atari hardware the 200 Hz timer-C interrupt runs the
50 Hz "tick" processing (`init_system_timer`), and one 50 Hz tick is 20 ms —
hence the literal `20L`. The value is a constant, unrelated to any runtime
measurement; even the `_hz_200` counter (which ticks at 200 Hz) is a separate
mechanism (`Tickcal` is not derived from it).

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x06
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0006
  ```
- **Total parameter bytes:** 2 (just the ID)
- **Native (AArch64) ABI:** ID `0x06` in `x8`; no arguments; result `20`
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** the integer `20` (milliseconds per system tick).
- **Source location:** `bios/bios.c:1098`.

### Side effects on global state

None.

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (none) | `bios/bios.c:1096` | always `20L` |

---

## Verification criteria

- **Nominal:** `Tickcal()` returns exactly `20` (decimal), with no inputs and
  no side effects, on every call.

---

## Edge cases discovered in source

1. **Pure constant — never reflects real hardware.** It is `20L` regardless of
   the actual timer rate. A build with a different tick rate would still report
   20; the value is an ABI constant, not a measurement.

2. **Not related to `_hz_200`.** Programs that want elapsed time read the
   200 Hz counter `_hz_200` (`0x4ba`) directly and convert with `×5` ms/tick;
   `Tickcal` is only the *GEMDOS/AES* 50 Hz tick period, used e.g. for
   `evnt_timer` scaling. Confusing the two is a common documentation error.

---

## Dependencies

### Requires (must be initialized / functional)

- None (constant). The associated 50 Hz processing is enabled later
  (`timer_c_sieve`), but `Tickcal`'s return does not depend on it.

### Used by

- AES `evnt_timer` period computation; user programs converting ticks → ms.

---

## Related data structures

- `docs/api/bios/_state.md` — `_hz_200` system-tick counter (distinct
  mechanism)

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: "period
  of the system timer in ms" = 20.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

None. `20L` across all EmuTOS builds and TOS versions.

---

## NewTOS implementation notes

(Empty during Phase 00.)

NewTOS keeps returning `20` for ABI purposes irrespective of its internal
tick rate, so timing-conversion code in ported programs stays correct.
