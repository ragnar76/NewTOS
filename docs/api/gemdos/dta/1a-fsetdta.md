# `Fsetdta` (GEMDOS 0x1A)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsmain.c:403` (`xsetdta`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, "store the pointer, no validation", per-process
> scope, the command-line-buffer default. Reference: the `PD.p_xdta` field
> location.

## EmuTOS source

**Primary location:** `bdos/fsmain.c:403` — `void xsetdta(DTAINFO *addr)`. The
body is a single assignment: `run->p_xdta = (DTA *)addr`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x1A` =
`xsetdta`, `wparms=2` (`bdos/bdosmain.c:211`).

**Helpers called:** none.

**Headers / type definitions used:**
- `include/bdosdefs.h:79` — `PD.p_xdta` (`DTA *`, at basepage offset 0x20).
- `bdos/fs.h:319-337` — `DTAINFO`; `include/bdosdefs.h:47-55` — public `DTA`.

**System variables / globals read:** `run` (current PD).
**System variables / globals written:** `run->p_xdta`.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fsetdta` sets the calling process's Disk Transfer Area pointer — the address
where `Fsfirst`/`Fsnext` write their 44-byte directory-entry results. The
implementation stores the supplied address into the current process descriptor's
`p_xdta` field and returns nothing (it is a C `void` function).

There is **no validation whatsoever**: the pointer may be NULL, point into ROM,
or be misaligned. `Fsetdta` never dereferences it — the fault (if any) happens
later inside `Fsfirst`/`Fsnext`, which write 44 bytes there.

The DTA pointer is **per-process**: it lives in the `PD`, so each process (and
each `Pexec` child) has its own. On process creation the field defaults to the
basepage's command-line buffer (`p->p_xdta = (DTA *)p->p_cmdlin`,
`bdos/proc.c:375`), at basepage offset 0x80. This is the historic Atari quirk: a
program that calls `Fsfirst` **without** first calling `Fsetdta` overwrites its
own command line with the search result.

Because `xsetdta` is `void`, the value left in D0 (and thus `x0`) is undefined
ABI scratch — callers must not read a return value. In practice the GEMDOS
dispatcher's per-arity call path returns whatever the C function left in the
result register; there is no defined result.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x1A
- **Stack frame on entry (m68k):** `Fsetdta(buf)` = `trap1(0x1a, buf)`
  (`include/bdosbind.h:31`):
  ```
  +2: buf (LONG, pointer)
   0: function_id (word) = 0x001A
  ```
- **Total parameter bytes:** 4 (LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x1A` in `x8`; `addr` in `x0`. The function is
  `void` — **no meaningful return value** in `x0`; the effect is the state
  change (`p_xdta = addr`).

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `addr` | `DTA *` (ptr) — new DTA address |

---

## Inputs

| Position | Name   | Type    | Size | Source location    | Notes |
|----------|--------|---------|------|--------------------|-------|
| arg0     | `addr` | `DTA *` | 4    | `bdos/fsmain.c:403`| new DTA pointer; not validated, not dereferenced here |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** `void` — no defined return value.
- **Encoding:** undefined; callers must not read it.
- **Source location:** `bdos/fsmain.c:403-406`.

### Side effects on global state

- `run->p_xdta = addr` (`bdos/fsmain.c:405`).

### Side effects on hardware

- None.

---

## Error conditions

None — there is no validation and no error path (`bdos/fsmain.c:403-406`). A bad
pointer manifests only when a subsequent `Fsfirst`/`Fsnext` writes to it.

---

## Verification criteria

- **Nominal:** `Fsetdta(&mydta); Fgetdta()` returns `&mydta`.
- **Search uses the new DTA:** after `Fsetdta(&mydta)`, a successful
  `Fsfirst(...)` writes its 44-byte result into `mydta`, not the command-line
  buffer.
- **NULL accepted:** `Fsetdta(NULL)` returns (no error); the fault, if any,
  is deferred to the next `Fsfirst`.
- **Per-process:** setting the DTA in a child (`Pexec`) does not change the
  parent's `p_xdta`.

---

## Edge cases discovered in source

1. **No validation.** Any pointer is stored verbatim — NULL, ROM, unaligned all
   "succeed" (`bdos/fsmain.c:405`).

2. **Per-process pointer, command-line default.** `p_xdta` is in the PD;
   `Pexec` sets a new process's default to its own `p_cmdlin` (basepage 0x80,
   `bdos/proc.c:375`). A first `Fsfirst` without `Fsetdta` clobbers the command
   line — relied upon by some Atari programs.

3. **`void` return.** No result in D0/`x0`; reading one is a program bug.

4. **DTA is 44 bytes.** The caller must ensure the buffer is at least 44 bytes
   and writable; there is no size check here.

5. **Aliasing is possible.** Multiple processes could point `p_xdta` at the same
   buffer; concurrent searches would clobber each other, but EmuTOS is
   single-threaded so this is a program-level concern.

---

## Dependencies

### Requires (must be initialized / functional)

- `run` (a current PD) — always true once GEMDOS is up.

### Used by

- Every program before a set of `Fsfirst`/`Fsnext` calls; nested-search code
  that swaps DTAs.

---

## Related data structures

- `docs/api/structures/dta.md` — the 44-byte DTA.
- `docs/api/structures/basepage.md` — `p_xdta` at 0x20, `p_cmdlin` at 0x80.
- `docs/api/gemdos/dta/2f-fgetdta.md` — reads the pointer back.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents "sets the DTA
  address"; the command-line-buffer default is a well-known Atari quirk.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: NewTOS may add a debug-build assertion that `addr` is in the calling
process's writable range (no hard ABI change — release builds keep the "trust
the caller" behaviour). The command-line-buffer default must be preserved for
m68k binaries; native source ports should always call `Fsetdta` explicitly.

**Kernel callers keep their own DTA (ARCH03 3b).** EmuTOS stores into
`run->p_xdta` (`bdos/fsmain.c:403-405`); every EmuTOS caller is a process.
NewTOS also has in-kernel (EL1) callers — the built-in desktop and shell —
whose DTA is a high-half kernel address that the 32-bit `p_xdta` cannot hold.
Their `Fsetdta`/`Fgetdta`/`Fsfirst`/`Fsnext` use a kernel DTA cell, chosen by
the caller's space (`TrapArgs::space`), **also while a process is the running
one**; before 3b such a call cut the kernel address to 32 bits into that
process's `p_xdta`. A program's DTA is unchanged. Host test
`tos-api/tests/arch03_3b_fsetdta.rs`.
