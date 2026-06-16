# `Setexc` — BIOS 0x05

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/bios.c:1071` (`setexc`).

## EmuTOS source

```c
LONG setexc(WORD num, LONG vector)
{
    LONG *addr = (LONG *)(4L * num);
    LONG oldvector = *addr;
    if (vector != -1) *addr = vector;
    return oldvector;
}
```

The function treats the first 1024 bytes of m68k memory as a flat
array of 256 LONG vectors. Vector `num` lives at byte offset
`4 * num`. m68k vector layout:

| `num`       | Vector                                       |
|-------------|----------------------------------------------|
| 0           | reset SP (read-only — overwriting it is harmless on m68k post-reset) |
| 1           | reset PC                                     |
| 2..63       | CPU traps + interrupts (bus error, address error, illegal instruction, divide-by-zero, autovectors 1..7, traps 0..15) |
| 64..255     | user/system vectors                          |
| 0x100..0x102 | "etv_term" (process terminate hook), used by `Pterm` |
| 0x100..0x118 | various GEM/AES hooks                       |

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x05` |
| WORD args | 3 (WORD + LONG) |
| Stack frame | `+4: vector (LONG)`, `+2: num (W)`, `+0: 0x0005` |
| Vector entry | `bios_vecs[5]` |

## Inputs

| Pos | Name     | Type | Range / meaning                              |
|-----|----------|------|----------------------------------------------|
| 0   | `num`    | WORD | exception vector number (0..1023 practically) |
| 1   | `vector` | LONG | new handler address, or `-1L` for query-only  |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG |

The **previous** vector value (always returned, regardless of
whether `vector == -1`).

## Errors

None. The function cannot fail in EmuTOS. Out-of-range `num`
silently writes to addresses outside the vector table — caller
ruins their own memory.

## Edge cases

### 1. `vector == -1` ⇒ query

Standard idiom for "what is the current handler?":

```c
LONG cur = Setexc(num, -1L);
```

Atari documentation explicitly defines `-1` as the query value.
Returning `-1L` from a previous Setexc that genuinely tried to
install address `-1` is **not possible** — the function refuses to
write `-1`. In practice no real handler lives at address `-1`.

### 2. No range check

`num` is a `WORD`. Negative `num` writes to addresses below 0
(unsigned wrap). `num > 1023` writes outside the m68k vector page
into low memory (system globals at `0x400+`). EmuTOS does not
protect this.

### 3. The MMU/MPU question

On m68k there is no MMU protection of the vector page by default.
On NewTOS / AArch64, the "vector page" is a software fiction —
exception handlers live in the AArch64 VBAR_EL1 table, not at
`0..0x400`. NewTOS must maintain a **shadow vector table** at the
emulated `0..0x400` and have its actual exception handlers consult
the shadow when dispatching m68k-style hooks.

### 4. Atomicity

Read-modify-write of a 32-bit word. On m68k a single LONG store is
atomic w.r.t. interrupts at the same level. On AArch64 we need
explicit atomic semantics if the vector page is shared across CPUs.

### 5. `etv_term` (vector 0x102)

`Pterm`'s user-term-handler protocol uses `Setexc(0x102, -1L)` to
fetch the current term hook, install a custom one, then chain via
the saved value. See `docs/api/gemdos/process/01-pterm.md` for the
protect_v wrapping.

### 6. AES "ev_timer" et al.

AES hooks several vectors (0x102 etv_term, 0x108 etv_critic,
0x10C etv_term, etc.) via Setexc. NewTOS must honour these as the
canonical TOS hooks.

## Dependencies

- The 1024-byte vector page at address 0.

Used by: AES/VDI installation, `Pterm` cleanup, custom interrupt
handlers, debuggers, terminate-stay-resident programs.

## Cross-references

- **Compendium:** "Setexc" — agrees on `-1` semantics and return
  value.
- **Profibuch:** "Setexc (0x05)" — agrees, lists known vector
  numbers.
- `docs/api/gemdos/process/01-pterm.md`
- `docs/api/bios/_state.md` — vector page layout.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-06 design driver**: NewTOS keeps a 1024-byte software
"m68k vector shadow" mapped at virtual `0..0x400` (read-write to
all processes — this is *the* ABI). Native AArch64 exception
handlers in `kernel/arch/aarch64/exceptions.rs` consult the shadow
when delivering m68k-emulated traps. Setexc becomes a plain
trapping store into that page.
