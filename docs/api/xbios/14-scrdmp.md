# `Scrdmp` (XBIOS 0x14)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/xbios.c:490` (`scrdmp`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, `void` return, the "invoke the installed dump
> routine then reset `dumpflg`" behaviour, the replaceable `dump_vec`.
> Reference: `protect_v`, the default `just_rts`, the Alt-Help trigger.

## EmuTOS source

**Primary location:** `bios/xbios.c:490` — `static void scrdmp(void)`, slot
0x14 of `xbios_vecs[]` (`bios/xbios.c:1310`).

**Installed handler / vector:** `dump_vec` (`include/tosvars.h:92`, sysvar
`0x502`) — the screen-dump routine; default `just_rts` (a no-op), set by
`bios/chardev.c:84`.

**Helpers called:**
- `util/miscasm.S` — `protect_v()` — call `dump_vec` in a `d2`/`a2`-preserving
  supervisor wrapper.

**Headers / type definitions used:**
- `include/tosvars.h:61` — `dumpflg`; `:92` — `dump_vec`.

**System variables / globals read:**
- `dump_vec` (`0x502`) — the routine to invoke.

**System variables / globals written:**
- `dumpflg` (`0x4EE`) — reset to `-1` after the dump (`bios/xbios.c:493`).

**Build configuration affecting this function:** none (always present).

---

## Source extraction summary

`Scrdmp` performs a screen dump by calling whatever routine is installed at
`dump_vec` (via `protect_v`, which preserves `d2`/`a2`), then resets
`dumpflg = -1` to re-arm future dumps. EmuTOS ships **no** built-in dump
routine: `dump_vec` defaults to `just_rts`, so a stock `Scrdmp` does nothing
until a program installs a real dump handler. The Alt-Help key increments
`dumpflg` (`bios/ikbd.c:596`), which the VBL notices to trigger a dump; the
XBIOS call is the programmatic equivalent.

Programs replace `dump_vec` to redirect the dump (to a printer, a file, a
PostScript stream). `Scrdmp` returns nothing.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x14
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0014
  ```
  No parameters (`Scrdmp()` = `xbios_v_v(20)`, `include/xbiosbind.h:38`).
- **Total parameter bytes:** 2.
- **Native (AArch64) ABI:** ID `0x14` in `x8`; no argument registers;
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
- **Source location:** `bios/xbios.c:490` (declared `void`).

### Side effects on global state

- `dumpflg = -1` (`bios/xbios.c:493`).

### Side effects on hardware

- Whatever `dump_vec` does (nothing by default; a printer/file write if a
  handler is installed).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

---

## Verification criteria

- **Invokes `dump_vec`:** installing a routine at `dump_vec` and calling
  `Scrdmp()` runs that routine exactly once.
- **Resets `dumpflg`:** after `Scrdmp()`, `dumpflg == -1`.
- **Default no-op:** with the default `dump_vec == just_rts`, `Scrdmp()` does
  nothing observable (and does not error).

---

## Edge cases discovered in source

1. **Default is a no-op.** `dump_vec` starts as `just_rts`
   (`bios/chardev.c:84`); EmuTOS has no built-in printer dump. The old spec's
   "default `dmpfunc` dumps to printer" is wrong — corrected (D-046).
2. **`dumpflg` is the re-arm flag.** `Scrdmp` sets it to `-1` (idle) after the
   dump; Alt-Help does `dumpflg++` (`bios/ikbd.c:596`) to request a dump the
   VBL then performs. A program can watch/poke `dumpflg` to coordinate.
3. **Runs via `protect_v`.** The dump routine executes in a supervisor
   wrapper that preserves `d2`/`a2`, so a C dump handler is register-safe.

---

## Dependencies

### Requires (must be initialized / functional)

- `dump_vec` (installed routine or the default `just_rts`), `dumpflg`.

### Used by

- Screenshot / print-screen tools, the Alt-Help key path.

---

## Related data structures

- `docs/api/xbios/_state.md §5` — `dump_vec`, `dumpflg`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents "dump screen
  to printer"; EmuTOS ships no default dumper.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- No `CONF_WITH_*` gate; the default `dump_vec` is a no-op on every build.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07/08: NewTOS may install a default `dump_vec` that writes a PNG of the
framebuffer to a configured path; `Scrdmp` invokes it and resets `dumpflg`,
and programs can still replace `dump_vec`.
