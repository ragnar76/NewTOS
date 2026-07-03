# `Bioskeys` (XBIOS 0x18)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/ikbd.c:136` (`bioskeys`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, `void` return, the "reset to power-up country
> defaults" behaviour. Reference: `current_keytbl` and the country tables.

## EmuTOS source

**Primary location:** `bios/ikbd.c:136` — `void bioskeys(void)`, slot 0x18
of `xbios_vecs[]` (`bios/xbios.c:1314`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:**
- `bios/country.c` — `get_keytbl()` — return the compiled default keyboard
  tables for the configured country.

**Headers / type definitions used:**
- `bios/ikbd.h:57-86` — `struct keytbl`.

**System variables / globals read:** the compiled country keyboard tables.

**System variables / globals written:**
- `current_keytbl` — reset to `*get_keytbl()` (`bios/ikbd.c:138-139`).

**Build configuration affecting this function:**
- The compiled country / keyboard-layout selection (which tables
  `get_keytbl` returns).

---

## Source extraction summary

`Bioskeys` restores the keyboard translation tables to their power-up state.
It fetches the country-default `struct keytbl` from `get_keytbl()` and copies
it wholesale into `current_keytbl`, undoing any `Keytbl` (0x10) overrides —
including the alt/dead/features fields that `Keytbl` cannot set. There are no
arguments and no return value.

This is the inverse of `Keytbl`: `Keytbl` swaps individual tables in;
`Bioskeys` puts the whole default set back.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x18
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0018
  ```
  No parameters (`Bioskeys()` = `xbios_v_v(24)`, `include/xbiosbind.h:42`).
- **Total parameter bytes:** 2.
- **Native (AArch64) ABI:** ID `0x18` in `x8`; no argument registers;
  **`void`** — no return value.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None.

---

## Outputs

### Return value

- **Register:** none (`void`).
- **Type:** `void`.
- **Source location:** `bios/ikbd.c:136` (declared `void`).

### Side effects on global state

- `current_keytbl = *get_keytbl()` — all eight fields reset
  (`bios/ikbd.c:138-139`).

### Side effects on hardware

None (software table swap; the ISR picks up the new tables on the next key).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

---

## Verification criteria

- **Undoes `Keytbl`:** after `Keytbl(mytbl, -1, -1)` then `Bioskeys()`,
  `Keytbl(-1,-1,-1)->norm` equals the country default again (not `mytbl`).
- **Resets all fields:** `alt*`/`dead`/`features` are restored to the country
  defaults, not just `norm`/`shft`/`caps`.

---

## Edge cases discovered in source

1. **Whole-struct reset.** `Bioskeys` overwrites all eight `struct keytbl`
   fields, so it also restores `altnorm/altshft/altcaps/dead/features` that
   `Keytbl` never touches (`bios/ikbd.c:138-139`).
2. **Country-dependent result.** The default tables come from the compiled
   country (`get_keytbl`), so the "power-up state" differs between a US and a
   German build.

---

## Dependencies

### Requires (must be initialized / functional)

- The compiled country keyboard tables; `current_keytbl` storage.

### Used by

- Programs that installed a custom layout and want to restore the default on
  exit; the desktop.

---

## Related data structures

- `docs/api/xbios/_state.md §2` — `current_keytbl`.
- `docs/api/xbios/10-keytbl.md` — the per-table setter.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents "restore
  power-up keyboard tables".)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- The default tables depend on the compiled country / keyboard layout.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: `Bioskeys` reloads the NewTOS country-default tables into
`current_keytbl`; the USB-HID translator uses them thereafter.
