# `Supexec` — XBIOS 0x26

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/vectors.S:1376` (`_supexec`).

## NewTOS status: **decision deferred** (needs ADR-Supexec)

Returns `EINVFN` today (the XBIOS dispatcher's catch-all — defined behaviour, not
a stub). `Supexec` runs a caller-provided routine "in supervisor mode"; on
NewTOS that means executing user code at EL1, which needs the **ADR-Supexec**
security / ABI decision first. **Deferred until a program actually needs it.**

## EmuTOS source

```asm
_supexec:
        movea.l 4(a7),a0
        jmp     (a0)
```

The function is the **shortest XBIOS handler**: pull the function
pointer off the stack, jump to it. The XBIOS dispatcher already
ran in supervisor mode (entered via trap #14), so the callee
inherits supervisor-mode privileges. On RTS, control returns
through the dispatcher's RTE which restores user mode if entry
was from user mode.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x26` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: codeptr (LONG)`, `+0: 0x0026` |
| Vector | `xbios_vecs[38]` |

## Inputs

| Pos | Name      | Type        | Range / meaning                       |
|-----|-----------|-------------|---------------------------------------|
| 0   | `codeptr` | LONG (LONG (*)(void)) | function to call in supervisor mode |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG |

Whatever the called function returns in `D0`. The calling
convention is "all registers preserved by caller; D0 = return
value".

## Errors

None at the XBIOS level. The called function may itself fault —
in which case the bus-error / address-error / illegal-instruction
exception handler runs as for any supervisor-mode fault.

## Edge cases

### 1. No argument passing

`Supexec` calls the function with **no arguments** (the asm is
literally `jmp (a0)`). To pass parameters, programs use globals or
embed pointers in the called function's text (PC-relative
addressing).

### 2. m68k JSR vs JMP semantics

`jmp (a0)` (not `jsr`!) means the called function returns directly
to *the dispatcher*, not to `_supexec`. The function's RTS pops
the dispatcher's return address. This works because XBIOS's
return path (`vectors.S` trampoline) expects exactly one return
address on the supervisor stack.

### 3. Supervisor-mode privilege

User-mode code calls `Supexec` to access supervisor-only
operations: hardware registers, vector table modification, MMU
state, halt instructions. NewTOS / AArch64 has no analogous
privilege flip at the user level — see notes.

### 4. Stack overflow risk

The supervisor stack is small (typically 1024 bytes in classic
TOS). Deep recursion or large local variables in the called
function exhausts it. EmuTOS doesn't check.

### 5. Re-entrant Supexec

Calling `Supexec` from within a Supexec'd function works — both
calls share the same supervisor stack. Interrupts taken during
Supexec also use this stack, increasing the overflow risk.

### 6. AES uses Supexec heavily

For palette manipulation, screen-mode probing, MMU ops. Many
"system info" tools run their probe in Supexec.

### 7. Long-running callbacks

Supervisor mode disables most of the multitasking model (such as
it is on TOS). Long-running Supexec'd code starves the VBL
handler — Atari folklore says "max 200 ms in Supexec".

## Dependencies

- The supervisor stack (`vectors.S`).

Used by: every program that needs to peek/poke hardware: monitors,
benchmarks, palette flippers, copy-protection schemes.

## Cross-references

- **Compendium:** "Supexec" — agrees on no-args convention.
- **Profibuch:** "Supexec (0x26)" — agrees.
- `docs/api/xbios/_dispatcher.md` — trap-14 entry.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-06 design driver — critical**: AArch64 has no m68k-style
"supervisor flag in SR". NewTOS runs all user code in EL0 and
kernel code in EL1; there is no in-process privilege flip. The
TOS `Supexec` semantics must be either:

1. **Trap into kernel and execute the user's callback in EL1** —
   security disaster (callback could be malicious, EL1 has full
   memory access).

2. **Pseudo-supervise**: callback runs in EL0 but with elevated
   I/O permissions for a curated set of MMIO regions
   (palette, line-A, etc.) mapped read/write into the calling
   process's address space. Most TOS Supexec uses fall into this
   category.

3. **Kernel-side translation**: the kernel maintains a list of
   known TOS Supexec patterns (palette swap, screen flip, …) and
   recognises common callbacks via signature matching. Foreign
   callbacks fail with a graceful fault.

ADR pending — to be filed in `docs/MEMORY.md` once PHASE-06
prototyping picks an approach. Working assumption: option 2
(curated MMIO mapping) since most Supexec calls just touch hardware
shadow registers we already emulate.
