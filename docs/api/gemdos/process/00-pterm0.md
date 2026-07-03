# `Pterm0` (GEMDOS 0x00)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/proc.c:588` (`x0term`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** trap ID/ABI (zero-argument), "does not return", and that the
> parent's `Pexec` receives exit code **0**. All termination semantics are those
> of `Pterm` (0x4C) — this is `Pterm(0)`.
> **Reference:** the delegation and the trap-2 CP/M-68K entry path.

## EmuTOS source

**Primary location:** `bdos/proc.c:588` — `void x0term(void)`.

**Installed handler / vector:** direct `funcs[]` entry `0x00` = `x0term`,
`wparms = 0` (`bdos/bdosmain.c:147`; `docs/api/gemdos/_dispatcher.md §6`).
Additionally reachable through the **trap-2 with `d0 == 0`** CP/M-68K
compatibility path installed at `Setexc(0x22, bdos_trap2)`
(`bdos/bdosmain.c:326`, `bdos/rwa.S` `_bdos_trap2`).

**Delegates to:** `xterm(0)` — full termination semantics in
[`4c-pterm.md`](4c-pterm.md).

**Headers / type definitions used:** none beyond those `xterm` uses.

**System variables / globals read/written:** none of its own — all via `xterm`
(`run`, `sft[]`, `dirtbl[]`, `pmd`/`pmdalt`, the `0x102`/`etv_term` vector).

**Build configuration affecting this function:** none directly (inherits
`CONF_WITH_ALT_RAM` behaviour from `xterm`).

---

## Source extraction summary

`Pterm0` is a thin wrapper that calls `Pterm` with a return code of zero
(`x0term` → `xterm(0)`). Every termination effect — invoking the user/GEM term
handler at vector `0x102`, closing all open files, dropping current-directory
references, freeing all owned memory, restoring the parent, and delivering the
exit code — is identical to `Pterm`; only the exit code is fixed at 0.

It exists as the smallest possible "exit with success" stub and for CP/M-68K
compatibility (the `trap #2, d0=0` convention), which the trap-2 handler routes
here.

---

## Trap interface

- **Trap number:** 1 (GEMDOS) — also reachable via trap 2 (`d0 == 0`).
- **Function ID:** 0x00
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0000
  ```
- **Total parameter bytes:** 0 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x00` in `x8`; no arguments. **No return** — the
  parent's `Pexec` receives exit code `0` in `x0`.

  (no parameters)

---

## Inputs

None.

---

## Outputs

### Return value

- **Register:** D0 / `x0` — delivered to the **parent**, not returned to the
  caller.
- **Type:** LONG (as the parent's `Pexec` sees it).
- **Encoding:** always `0` (`xterm(0)` stores `0` into the parent's `p_dreg[0]`,
  high word zero-extended — see D-005).
- **Source location:** `bdos/proc.c:590` (`xterm(0)`).

### Side effects on global state

Identical to `Pterm` (`ixterm`): parent restored, all files closed, current-dir
refs dropped, all owned memory freed. See [`4c-pterm.md`](4c-pterm.md).

### Side effects on hardware

Same as `Pterm` (dirty-buffer writeback on file close; the `0x102` handler may
touch hardware).

---

## Error conditions

None — `Pterm0` cannot fail and does not return. The same no-parent structural
caveat as `Pterm` applies (a boot process calling it would dereference a NULL
`run`).

**Error paths not in Compendium:** none.

---

## Verification criteria

- **Exit code 0:** a child launched by `Pexec(0, ...)` that calls `Pterm0`
  causes the parent's `Pexec` to return `0`.
- **Full teardown:** all the child's open files, current-dir references and
  `Malloc`ed memory are released (same as `Pterm`).
- **Term handler:** a handler installed at vector `0x102` runs once before
  teardown.
- **Does not return:** control resumes in the parent.
- **Trap-2 path:** a `trap #2` with `d0 == 0` produces the same effect as
  `Pterm0`.

---

## Edge cases discovered in source

1. **Full `xterm` path, not a shortcut.** Because `x0term` calls `xterm(0)`, the
   user term handler, file close, current-dir release, memory free and parent
   restore all run exactly as for `Pterm` — `Pterm0` is not a "lighter" exit.

2. **Explicit zero exit code (D-005).** `xterm(0)` stores `0` into the LONG
   `p_dreg[0]`; the high word is deterministically zero, unlike the "undefined
   high word" some Atari docs describe.

3. **CP/M-68K trap-2 entry.** The undocumented `trap #2, d0=0` convention is
   routed to `Pterm0` by the trap-2 handler; other trap-2 calls fall through to
   the previous vector (`bdos/rwa.S` `_bdos_trap2`).

4. **From a TSR.** A resident program installed via `Ptermres` has no live parent
   context to return to, so calling `Pterm0` from it is not meaningful.

---

## Dependencies

### Requires (must be initialized / functional)

- `xterm` (transitively: `Setexc`/`protect_v`, `xclose`, `decr_curdir_usage`,
  `free_all_owned`, `termuser`/EL0-return, a valid `run->p_parent`).

### Used by

- Hand-written assembly "tiny exit" stubs; CP/M-68K-style code via trap 2.

---

## Related data structures

- `docs/api/structures/basepage.md` — `p_dreg[0]`, `p_parent`.
- Same set as [`4c-pterm.md`](4c-pterm.md).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Documents `Pterm0` as an
  obsolete alias for `Pterm(0)` — consistent with EmuTOS.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

### Discrepancies

- **D-005** applies transitively (exit-code high-word width) — see
  [`4c-pterm.md`](4c-pterm.md) and `docs/api/_discrepancies.md#D-005`.

---

## Configuration / version notes

- Present in all builds. Behaviour tracks `xterm` (so `CONF_WITH_ALT_RAM`
  affects the memory-free step transitively).

---

## NewTOS implementation notes

(Empty during Phase 00.)

Implemented as `Pterm(0)` — shares the entire termination path with `Pterm`
(only the exit code differs). See [`4c-pterm.md`](4c-pterm.md) NewTOS notes for
the `etv_term` vector and EL0-return mapping.
