# `Ssystem` — GEMDOS 0x154 (MiNT extension)

> **FreeMiNT extension spec** (EmuTOS does not implement `Ssystem`).
> Source: FreeMiNT @ `f906416` — `sys/syscalls.master:460`,
> `sys/ssystem.c` (`s_system`), `sys/mint/ssystem.h` (mode codes),
> `sys/cookie.c` (`get_cookie`/`set_cookie`/`del_cookie`).
> Adopted per ADR-012 (`docs/MEMORY.md`) as NewTOS's cookie-jar access
> path; data model: [`../bios/_cookie-jar.md`](../bios/_cookie-jar.md).
>
> **Scope of this spec:** the `Ssystem` trap shell + the three **cookie**
> subfunctions only. The many other FreeMiNT modes (`S_OSNAME`,
> `S_OSVERSION`, `S_GETLVAL`, …) are out of scope for now; the dispatcher
> returns `ENOSYS` for unimplemented modes so the surface stays honest.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0154` (340) |
| Signature | `LONG Ssystem(WORD mode, LONG arg1, LONG arg2)` |
| ABI | AAPCS64: `x8 = 0x154`, `x0 = mode`, `x1 = arg1`, `x2 = arg2`, result in `x0` |

**Dispatch note (NewTOS):** `0x154` is far above the current GEMDOS
`MAX_FN = 0x57`, so the static dispatch table cannot hold it. The GEMDOS
dispatcher special-cases `func_id == 0x154` **before** the table lookup
(a small `match` arm), routing to the Ssystem handler. Future MiNT
extensions follow the same "high id, special-cased" pattern.

## Mode codes (cookie subset, `sys/mint/ssystem.h`)

| Mode | Value | Meaning |
|------|------:|---------|
| `S_GETCOOKIE` | 8 | read a cookie |
| `S_SETCOOKIE` | 9 | add / update a cookie |
| `S_DELCOOKIE` | 26 | remove a cookie |

Any other mode → `ENOSYS` (not yet implemented).

## `S_GETCOOKIE` (mode 8) — `get_cookie(arg1, arg2)`

`arg1` = cookie **tag**; `arg2` = pointer to a `LONG` where the value is
written (the *return-by-pointer* form FreeMiNT uses). Three behaviours
(`sys/cookie.c:get_cookie`):

1. **`arg1 == 0`** → return the **NULL-slot value** = jar capacity. If
   `arg2 != 0`, write it to `*arg2` and return `E_OK`; if `arg2 == 0`,
   return the value directly in `x0`.
2. **`arg1` high word == 0** (a small number) → treat `arg1` as a **1-based
   slot index**: enumerate the Nth cookie, writing its `{tag,value}`
   (FreeMiNT copies the entry). `EINVAL` (or the documented error) if the
   index exceeds the cookie count.
3. **otherwise** → search the jar for `tag`; on match write `value` to
   `*arg2` and return `E_OK`; if absent return the not-found error.

## `S_SETCOOKIE` (mode 9) — `set_cookie(arg1, arg2)`

`arg1` = tag, `arg2` = value (`sys/cookie.c:set_cookie`):

- **Validate the tag**: every byte must be non-zero
  (`tag & 0xFF000000`, `0x00FF0000`, `0x0000FF00`, `0x000000FF` all
  non-zero) — this keeps the slot-number space (high word 0) usable by
  `S_GETCOOKIE`. Invalid → `EINVAL`.
- **Update or append**: scan to the terminator; if `tag` already present,
  overwrite its value (`E_OK`). Otherwise, if there is room
  (`count+1 < capacity`, where `capacity` = the terminator's `value`),
  write `{tag,value}` into the terminator slot and push a fresh
  `{0, capacity}` terminator down (`E_OK`). If full → error (FreeMiNT
  grows the jar; NewTOS can grow via the kernel allocator, see below).

## `S_DELCOOKIE` (mode 26) — `del_cookie(arg1)`

`arg1` = tag. Remove the entry and compact the jar. (Body:
`sys/cookie.c:del_cookie`.)

## NewTOS implementation notes

- **Jar storage (ADR-012):** kernel-owned, allocated from the kernel heap,
  pointer in a kernel-internal global (the NewTOS analogue of FreeMiNT's
  `*CJAR` — **not** the Atari `$5A0` sysvar). The jar is EL1 high-half
  memory; user programs never see the pointer, only values via this trap.
  This *is* the value-query design ADR-012 chose — FreeMiNT's
  return-by-pointer `arg2` fits it exactly (the kernel writes one `LONG`
  to the caller's `arg2` buffer with an unprivileged/checked store).
- **Growth:** dynamic allocation means no fixed cap; grow the jar (realloc
  + copy + re-seed the `{0, capacity}` terminator) when `set_cookie` would
  overflow, instead of failing.
- **NewTOS cookie set** (seeded at boot, replacing `fill_cookie_jar`):
  keep the meaningful tags — `_MCH` (a NewTOS machine id), `_IDT` (date/
  time format), `_FLK` (GEMDOS file-locking present), `_AKP` (keyboard
  language) — and add an **`NTOS`** identity cookie (value = NewTOS
  version) so ported programs can detect the environment. **Drop**
  `_VDO`/`_SND`/`_FDC`/`_FPU` (Shifter/PSG/floppy/m68k-FPU — meaningless).
- **`arg2` writes** target a user buffer: use the same unprivileged-store
  path as `Cconrs` (an EL0-permission-checked write), not a raw kernel
  store, so a bad pointer faults the caller rather than the kernel.

## Cross-references

- Data model + access analysis: `docs/api/bios/_cookie-jar.md`
- Decision: ADR-012 (`docs/MEMORY.md`)
- FreeMiNT `sys/ssystem.c`, `sys/cookie.c`, `sys/mint/ssystem.h` @ `f906416`
- Compendium / MiNT docs: "Ssystem", "Cookie Jar" — cross-check pending.
