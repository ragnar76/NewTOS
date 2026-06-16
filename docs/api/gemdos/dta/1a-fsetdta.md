# `Fsetdta` — GEMDOS 0x1A

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsmain.c:403` (`xsetdta`).

## EmuTOS source

```c
void xsetdta(DTAINFO *addr) { run->p_xdta = (DTA *)addr; }
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x001A` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: addr (LONG)`, `+0: 0x001A` |

## Inputs

| Pos | Name   | Type | Range / meaning                            |
|-----|--------|------|--------------------------------------------|
| 0   | `addr` | DTA* | new Disk Transfer Address (44-byte buffer) |

## Outputs

None — function declared `void`. The dispatcher returns whatever
the previous `D0` contained; per Compendium, callers must not read
a return value. Practically, `bdosmain.c`'s dispatcher zeroes D0
before invoking 0-arg functions, so D0 = 0 on return is reliable.

## Errors

None — no validation. `addr` may be `NULL`, point at ROM, or be
unaligned. The next `Fsfirst`/`Fsnext` will dereference it and
crash, fault, or corrupt memory accordingly.

## Edge cases

### 1. Per-process

`p_xdta` lives in the PD; each process has its own DTA. `Pexec`
initialises the child's `p_xdta` to its own `p_cmdlin` field
(see `docs/api/structures/basepage.md`) — Atari's classic "DTA
defaults to command-line area" quirk. A child that calls `Fsfirst`
without `Fsetdta` first will overwrite its own command-line.

### 2. No size validation

The DTA is 44 bytes. The caller must ensure the buffer is at least
that big and writable. No bounds check.

### 3. NULL is "legal"

`Fsetdta(NULL)` succeeds. Subsequent `Fsfirst` writes to address 0.
EmuTOS does not special-case this.

### 4. Pointer aliasing

Multiple processes can have `p_xdta` pointing at the same buffer.
Concurrent `Fsfirst`/`Fsnext` would clobber each other — but EmuTOS
is single-threaded.

## Dependencies

- `run` PD pointer.

## Cross-references

- **Compendium:** "Fsetdta" — agrees.
- **Profibuch:** "Fsetdta (0x1A)" — agrees.
- `docs/api/gemdos/dta/2f-fgetdta.md`
- `docs/api/structures/dta.md`
- `docs/api/structures/basepage.md` — `p_xdta` and the "DTA defaults
  to command-line" Atari quirk.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: NewTOS will validate that `addr` is in the calling
process's writable address range as a debug-build assertion (no
hard ABI break — release builds preserve "trust the caller"
behaviour).
