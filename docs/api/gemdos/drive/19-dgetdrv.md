# `Dgetdrv` (GEMDOS 0x19)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsmain.c:438` (`xgetdrv`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI (no args), the 0-based current-drive LONG return.
> Reference: the per-process `PD.p_curdrv` field.

## EmuTOS source

**Primary location:** `bdos/fsmain.c:438` — `long xgetdrv(void)`. A one-line
accessor: `return run->p_curdrv;`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x19` =
`xgetdrv`, `wparms = 0` (`bdos/bdosmain.c:210`).

**Helpers called:** none.

**Headers / type definitions used:**
- `include/bdosdefs.h:86` — `PD.p_curdrv` (`UBYTE`, basepage offset **0x37**).

**System variables / globals read:** `run` (current PD, `bdos/proc.c:54`) →
`run->p_curdrv`.
**System variables / globals written:** none.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Dgetdrv` returns the calling process's current default drive as a 0-based
index (`0 = A:`, …, `25 = Z:`). It simply reads the per-process `p_curdrv` byte
from the PD and returns it as a LONG. Because `p_curdrv` is a `UBYTE`, the value
is **zero-extended** into the LONG return — the high bytes are always zero.

The current drive is per-process: `Dgetdrv` reports whatever the current process
last set with `Dsetdrv` (or inherited from its parent at `Pexec`). There is no
validation and no error path; the returned value is always whatever byte is in
`p_curdrv`, which is only ever written by `xsetdrv` (bounds-checked to `0..25`)
or copied from the parent at process creation.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x19
- **Stack frame on entry (m68k):** `Dgetdrv()` = `trap1(0x19)`:
  ```
   0: function_id (word) = 0x0019
  ```
- **Total parameter bytes:** 0 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x19` in `x8`; no argument registers used; LONG
  result (0..25) in `x0`, zero-extended (high bytes always 0).

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
- **Encoding:** current drive of the calling process, 0-based (`0 = A:` …
  `25 = Z:`); zero-extended from the `UBYTE` field, so the high word/bytes are
  always zero.
- **Source location:** `bdos/fsmain.c:440`.

### Side effects on global state

- None (read-only).

### Side effects on hardware

- None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error path) | — | always returns `p_curdrv` |

No error is possible.

---

## Verification criteria

- **Nominal:** after `Dsetdrv(2)`, `Dgetdrv()` returns `2` (C:).
- **High bytes zero:** the returned LONG is in `0..25` with all higher bits
  clear (never sign-extended, never garbage in the high word).
- **Per-process:** a child that changes its own current drive does not affect
  the parent's `Dgetdrv` result, and vice versa.

---

## Edge cases discovered in source

1. **`p_curdrv` is a `UBYTE`**, at basepage offset **0x37** (immediately after
   `p_uft[6]` at 0x30 and `p_lddrv` at 0x36). The return is its zero-extended
   value (`include/bdosdefs.h:84-86`).

2. **Always in range 0..25.** The only writer that accepts external input,
   `xsetdrv`, bounds-checks to `0..BLKDEVNUM-1`; the boot/`Pexec` path copies a
   valid value. So `Dgetdrv` never returns an out-of-range drive.

3. **Per-process.** Different children may report different current drives;
   `Pexec` copies the parent's `p_curdrv` at creation, after which they diverge.

---

## Dependencies

### Requires (must be initialized / functional)

- `run` (current PD pointer, `bdos/proc.c:54`).

### Used by

- Shells building the prompt, `Dgetpath(buf, 0)` callers wanting the current
  drive letter, any program that needs to prefix a `Dgetpath` result with the
  drive letter.

---

## Related data structures

- `docs/api/gemdos/_state.md` §4 — current-drive model (`PD.p_curdrv`).
- `docs/api/structures/basepage.md` — `p_curdrv` at basepage offset 0x37.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the 0-based
  current-drive return.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None. The 0-based index is build-independent.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: read the per-process current-drive field; return zero-extended 0-based
index. Trivial once `p_curdrv` exists in the kernel `Process`.
