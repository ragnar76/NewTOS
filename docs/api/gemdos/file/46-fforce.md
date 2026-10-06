# `Fforce` (GEMDOS 0x46)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fshand.c:32` (`xforce`) and the 3-arg internal
> `bdos/fshand.c:46` (`ixforce`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the standard-target / non-standard-source rule, the
> refcount bump for file mappings, error codes, per-process scope. Reference:
> `sft[]`/`p_uft[]` internals.

## EmuTOS source

**Primary location:** `bdos/fshand.c:32` — `long xforce(int std, int h)`
(wraps `ixforce(std, h, run)`).

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x46` =
`xforce`, `wparms=2` (`bdos/bdosmain.c:266`).

**Helpers called:**
- `bdos/fshand.c:46` — `ixforce()` — the shared implementation (also called by
  the process launcher to wire a child's std handles).
- `bdos/fsmain.c:474` — `getofd()` — validate the source handle.

**Headers / type definitions used:**
- `include/bdosdefs.h:61` — `NUMSTD = 6`; `bdos/fs.h:307-312` — `FTAB`.

**System variables / globals read:** `sft[h-NUMSTD].f_ofd` (the source
mapping).
**System variables / globals written:** `p->p_uft[std]` (the target slot);
`sft[h-NUMSTD].f_use` (bumped for file mappings).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fforce` redirects a **standard** handle (`std`, 0..5) so that it refers to the
same object as another handle `h`. It is the mechanism behind stdin/stdout
redirection: after `Fforce(1, fileh)`, GEMDOS console output routed through
handle 1 goes to the file (via the dispatcher's standard-handle interception —
see `docs/api/gemdos/_dispatcher.md` §5).

`ixforce` validates `std` (must be `0..NUMSTD-1`, else `EIHNDL`). The source `h`
is handled by class:

- **BIOS device** (`h < 0`): the negative handle is stored directly into
  `p_uft[std]`; the standard handle now does BIOS char-device I/O.
- **Standard handle** (`0 <= h < NUMSTD`): rejected with `EIHNDL` — you cannot
  force one standard handle onto another. (Use `Fdup` to lift it to a
  non-standard handle first.)
- **Non-standard handle** (`h >= NUMSTD`): validated with `getofd` (no OFD →
  `EIHNDL`). If that SFT entry is itself mapped to a BIOS device
  (`f_ofd < 0`), the negative handle propagates and `f_use` is **not** bumped.
  Otherwise `p_uft[std] = h` and `sft[h-NUMSTD].f_use++`, so the OFD survives
  even if the caller later closes the source handle.

Returns `E_OK` on success. The change is per-process (`p_uft` lives in the PD);
children inherit the parent's table at `Pexec` time and diverge thereafter.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x46
- **Stack frame on entry (m68k):** `Fforce(std, h)` — `std` pushed first,
  matching `xforce(int std, int h)` and `wparms=2`:
  ```
  +4: h   (word)
  +2: std (word)
   0: function_id (word) = 0x0046
  ```
- **Total parameter bytes:** 4 (WORD + WORD) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x46` in `x8`; args in `x0..x1`; LONG result
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `std` | `WORD` (16) — target standard handle 0..5 |
  | +4         | `x1`        | `h`   | `WORD` (16) — source: `<0` BIOS dev, or `>= 6` non-std |

---

## Inputs

| Position | Name  | Type  | Size | Source location    | Notes |
|----------|-------|-------|------|--------------------|-------|
| arg0     | `std` | `int` | 2    | `bdos/fshand.c:50` | must be a standard handle 0..5 |
| arg1     | `h`   | `int` | 2    | `bdos/fshand.c:53` | source handle; a standard handle (0..5) is rejected |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `E_OK` (0) on success; negative error otherwise.
- **Source location:** `bdos/fshand.c:77`.

### Side effects on global state

- Sets `p_uft[std]` to `h` or to the propagated BIOS handle
  (`bdos/fshand.c:54,68-74`).
- For a file mapping, `sft[h-NUMSTD].f_use++` (`bdos/fshand.c:73`).

### Side effects on hardware

- None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `std < 0` or `std >= NUMSTD` | `bdos/fshand.c:50-51` | `EIHNDL` (-37) |
| Source `h` is itself a standard handle (`0 <= h < NUMSTD`) | `bdos/fshand.c:57-58` | `EIHNDL` (-37) |
| Source `h >= NUMSTD` but has no OFD (`getofd` NULL) | `bdos/fshand.c:60-61` | `EIHNDL` (-37) |

---

## Verification criteria

- **Nominal file redirect:** with `fh` from `Fopen`, `Fforce(1, fh)` returns
  `E_OK`; subsequent `Cconout`/`Fwrite(1, …)` write to the file, and
  `sft[fh-6].f_use` is now 2.
- **BIOS pass-through:** `Fforce(1, -1)` returns `E_OK`; handle 1 now does
  console I/O.
- **Reject std source:** `Fforce(1, 0)` returns `EIHNDL` (-37).
- **Reject bad target:** `Fforce(6, fh)` returns `EIHNDL` (-37).
- **Reject unopened source:** `Fforce(1, 40)` with no open handle 40 returns
  `EIHNDL` (-37).

---

## Edge cases discovered in source

1. **Cannot force to another standard handle.** `h in [0, NUMSTD)` → `EIHNDL`;
   lift it via `Fdup` first (`bdos/fshand.c:57-58`).

2. **File mapping bumps `f_use`.** `sft[h-NUMSTD].f_use++` keeps the OFD alive
   past a close of the source handle (`bdos/fshand.c:73`).

3. **BIOS-mapped non-std source does NOT bump `f_use`.** If the source SFT
   entry's `f_ofd` is negative (it was itself forced from a BIOS device), the
   negative handle is propagated with no refcount change (`bdos/fshand.c:68-69`).

4. **Idempotent force re-bumps.** Forcing the same file mapping again increments
   `f_use` again; each bump needs a matching `Fclose` to balance.

5. **Per-process.** `p_uft` is in the PD; `Fforce` affects only `run`. A
   `Pexec` child inherits the parent's mappings at creation, then diverges
   (`bdos/fshand.c:34,46`).

6. **`ixforce` is the launcher hook.** The 3-arg form is called internally to
   set up a child process's standard handles before it runs — not reachable as a
   user syscall. `init_pd_files` (`bdos/proc.c:387-399`) calls it for every
   standard handle of `run` that is `> 0`, so the child shares the file and
   `f_use` counts it; values `<= 0` are copied without a call.

7. **The previous mapping is not released.** `ixforce` overwrites
   `p_uft[std]` without closing or decrementing whatever it mapped before
   (`bdos/fshand.c:46-77`). The use count a `Fforce(1, fh)` took stays on `fh`
   after `Fforce(1, saved)` restores the old mapping; it is given back by a
   `Fclose(1)` while the mapping is in place, or by the owner's `Pterm`
   (`ixterm`, `bdos/proc.c:121-123`).

8. **Restore idiom.** `saved = Fdup(1); Fforce(1, fh); …; Fforce(1, saved);
   Fclose(saved);` leaves `p_uft[1]` holding the **BIOS handle** `saved` was
   mapped to (`f_ofd < 0` propagates, `bdos/fshand.c:68-69`), so closing
   `saved` afterwards does not disturb handle 1 (mintlib `system()`,
   `sdk/libc/mintlib/posix/system.c:326-340`).

---

## Dependencies

### Requires (must be initialized / functional)

- `run->p_uft[]`, `sft[]`; a valid source OFD for file mappings.

### Used by

- Shells and AES for redirection; the process launcher (`ixforce`) to wire a
  child's stdin/stdout/stderr.

---

## Related data structures

- `docs/api/gemdos/_state.md` §2 — SFT / `p_uft[]` model.
- `docs/api/structures/ofd.md` — `f_use` reference counting.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the
  redirection semantics and standard-target restriction.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `NUMSTD = 6`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06: the process-launch path uses `ixforce` to wire the child's std
handles. The refcount semantics — bump for file mappings, no bump for
BIOS-mapped non-std sources — are contract for correct close balancing.

TASK/PROC02 (2026-10-06): `tos-api/src/bdos/fs/file.rs` `ixforce(std, h, p)`
follows `bdos/fshand.c:46-77` line for line (validation order, BIOS
propagation, `f_use++`), `fforce` is `ixforce(std, h, run)`. With no running
process (kernel caller) nothing is written and no use is taken — the stand-in
PDs are read-only. The loader's `init_pd_files` calls `ixforce` for the
child (`docs/api/gemdos/process/4b-pexec.md`, edge case 7). Before PROC02
`Fforce` kept no use count and the loader gave every child `-1` in all six
handles.
