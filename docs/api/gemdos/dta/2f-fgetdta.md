# `Fgetdta` (GEMDOS 0x2F)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsmain.c:394` (`xgetdta`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the returned pointer being the full-width DTA address,
> per-process scope, the command-line-buffer default. Reference: the
> `PD.p_xdta` field location.

## EmuTOS source

**Primary location:** `bdos/fsmain.c:394` — `DTAINFO *xgetdta(void)`. The body
returns `(DTAINFO *)run->p_xdta`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x2F` =
`xgetdta`, `wparms=0` (`bdos/bdosmain.c:239`).

**Helpers called:** none.

**Headers / type definitions used:**
- `include/bdosdefs.h:79` — `PD.p_xdta` (basepage offset 0x20).

**System variables / globals read:** `run->p_xdta`.
**System variables / globals written:** none.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fgetdta` returns the calling process's current DTA pointer — whatever was last
set by `Fsetdta`, or the boot/`Pexec` default if never set. It simply reads
`run->p_xdta` and returns it; there is no error path and no side effect.

The returned value is a **pointer**, so on the native AArch64 ABI it occupies
the full 64-bit `x0` (not a sign-extended 32-bit LONG). On the m68k ABI it is a
32-bit address in D0.

If the process never called `Fsetdta`, the pointer is its basepage command-line
buffer (`p_cmdlin`, basepage offset 0x80), because `Pexec` initialises `p_xdta`
to that address (`bdos/proc.c:375`). This is the same Atari quirk noted under
`Fsetdta`: programs that call `Fsfirst` without setting the DTA overwrite their
own command line — and `Fgetdta` is how a well-behaved program discovers/saves
the current DTA before pointing it elsewhere.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x2F
- **Stack frame on entry (m68k):** `Fgetdta()` = `trap1(0x2f)`:
  ```
   0: function_id (word) = 0x002F
  ```
- **Total parameter bytes:** 0 → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x2F` in `x8`; no argument registers; the
  **full 64-bit DTA pointer in `x0`** (a pointer, **not** sign-extended from
  32 bits).

  (no stack parameters)

---

## Inputs

None.

---

## Outputs

### Return value

- **Register:** D0 / `x0` (full pointer width)
- **Type:** `DTA *` (LONG on m68k)
- **Encoding:** the current `run->p_xdta`. Never NULL unless the program set it
  so with `Fsetdta`.
- **Source location:** `bdos/fsmain.c:396`.

### Side effects on global state

- None.

### Side effects on hardware

- None.

---

## Error conditions

None — `Fgetdta` always succeeds (`bdos/fsmain.c:394-397`).

---

## Verification criteria

- **Round-trip:** `Fsetdta(&mydta); Fgetdta()` returns `&mydta`.
- **Default:** a fresh process that never called `Fsetdta` gets back its
  basepage command-line buffer address (`&PD.p_cmdlin`, basepage offset 0x80).
- **Save/restore idiom:** `old = Fgetdta(); Fsetdta(&tmp); …; Fsetdta(old);`
  restores the original DTA around a nested search.
- **Per-process:** the pointer returned reflects only the calling process's
  `p_xdta`.

---

## Edge cases discovered in source

1. **Command-line-buffer default.** If never set, `Fgetdta` returns
   `&PD.p_cmdlin` (basepage 0x80), the `Pexec` default (`bdos/proc.c:375`).
   Programs that then `Fsfirst` overwrite the command line — this is Atari ABI
   that programs depend on.

2. **Pointer result, full width.** On native AArch64 the return is a full 64-bit
   pointer in `x0`, not a sign-extended WORD/LONG.

3. **No error, no side effect.** Pure read of `run->p_xdta`
   (`bdos/fsmain.c:396`).

---

## Dependencies

### Requires (must be initialized / functional)

- `run` (a current PD).

### Used by

- Any program that saves the current DTA before redirecting it; nested-search
  code; libraries wrapping `Fsfirst`/`Fsnext`.

---

## Related data structures

- `docs/api/structures/dta.md` — the 44-byte DTA.
- `docs/api/structures/basepage.md` — `p_xdta` at 0x20, `p_cmdlin` at 0x80.
- `docs/api/gemdos/dta/1a-fsetdta.md` — sets the pointer.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents "returns the
  current DTA address".)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: returns the process DTA pointer verbatim; on AArch64 it is a full
64-bit pointer in `x0` for native callers, mapped to the m68k 32-bit form by the
Phase-10 emulator shim.
