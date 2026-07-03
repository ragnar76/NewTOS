# `Supexec` (XBIOS 0x26)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/vectors.S:1376` (`_supexec`), slot 0x26 of
> `xbios_vecs[]` (`bios/xbios.c:1341`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the "run the callback in supervisor mode, return its
> D0" behaviour, no argument passing. Reference: the m68k `jmp` tail-call and
> supervisor-stack mechanics — which do **not** map to AArch64 (see notes).

## NewTOS status: **decision deferred** (needs ADR-Supexec)

`Supexec` is present in the table (slot 0x26) with a real handler on EmuTOS,
so it is **not** the dispatcher's func-number fallback. On NewTOS it needs the
**ADR-Supexec** security/ABI decision before it can run a caller's code with
kernel privilege (AArch64 has no in-process privilege flip — see notes).
**Deferred until a program actually needs it.** (The old spec's opening line
"Returns `EINVFN` today (the dispatcher catch-all)" is wrong on two counts:
the dispatcher returns the *function number*, not `EINVFN`, and `Supexec` has
its own table slot so it never hits the catch-all. Corrected here; see D-039.)

## EmuTOS source

**Primary location:** `bios/vectors.S:1376` — `_supexec` (assembly). The
non-debug handler is two instructions: load the code pointer from `4(a7)` and
`jmp` to it. A debug-only C version (`bios/xbios.c:786-806`, `DBG_XBIOS`)
brackets the call with a full register save/restore.

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:** none.

**Headers / type definitions used:** none.

**System variables / globals read/written:** none (the callback may touch
anything).

**Build configuration affecting this function:**
- `DBG_XBIOS` — swaps the bare asm handler for the register-preserving C
  wrapper. Production builds use the bare asm.

---

## Source extraction summary

`Supexec` runs a caller-provided routine in supervisor mode and returns
whatever that routine leaves in D0. The XBIOS trap already entered supervisor
mode, so the production handler simply tail-`jmp`s to the callback
(`movea.l 4(a7),a0; jmp (a0)`) — the callback's terminating `rts` returns
straight through the XBIOS dispatcher's `rte`, which restores the caller's
prior mode.

**Register preservation:** the production (`jmp`) handler does **not** save or
restore any registers — it is a bare tail jump. (Only the `DBG_XBIOS` C
version saves `d2-d7/a2-a6`.) The XBIOS dispatcher's own save area preserves
`d3-d7/a3-a7` across the whole trap, but a Supexec callback that clobbers
`d0-d2/a0-a2` follows normal m68k caller-saved conventions. The callback
receives **no arguments** (`jmp (a0)`); to pass data it uses globals or
PC-relative constants. EmuTOS documents that the callback must not make BIOS
or GEMDOS calls.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x26
- **Stack frame on entry (m68k):** `Supexec(codeptr)` = `xbios_l_l(38, a)`
  (`include/xbiosbind.h:56`):
  ```
  +2: codeptr (LONG, function pointer)
   0: function_id (word) = 0x0026
  ```
- **Total parameter bytes:** 6 (L).
- **Native (AArch64) ABI:** ID `0x26` in `x8`; arg in `x0`; the LONG result
  (the callback's return) **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name      | Type / size |
  |------------|-------------|-----------|-------------|
  | +2         | `x0`        | `codeptr` | `LONG (*)(void)` (function ptr) |

---

## Inputs

| Position | Name      | Type   | Size | Source location     | Notes |
|----------|-----------|--------|------|---------------------|-------|
| arg0     | `codeptr` | `PFLONG` | 4  | `bios/vectors.S:1377` | routine to run in supervisor mode; called with no args |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** whatever the callback returns in D0.
- **Source location:** the callback's own `rts`; `_supexec` does not modify D0.

### Side effects on global state / hardware

- Whatever the callback does (arbitrary — this is the point of `Supexec`).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return at the XBIOS level) | — | — |

`Supexec` itself has no error path. A faulting callback raises a
bus/address/illegal-instruction exception in supervisor mode, handled by the
generic exception handlers.

---

## Verification criteria

- **Runs the callback:** `Supexec(fn)` executes `fn` once and returns its D0.
- **Supervisor privilege:** a callback that reads a supervisor-only location
  (or the m68k SR) succeeds where the same code would fault in user mode.
- **No arguments:** `fn` is entered with no parameters; passing data requires
  globals (a callback expecting args in registers gets whatever the dispatcher
  left).
- **Return propagation:** `fn`'s D0 is the value `Supexec` returns.

---

## Edge cases discovered in source

1. **Production handler is a bare `jmp` — no register save.** `_supexec` does
   `jmp (a0)`, not `jsr` (`bios/vectors.S:1376-1378`); the callback's `rts`
   returns through the dispatcher. Only the `DBG_XBIOS` C version saves
   `d2-d7/a2-a6`. The old spec claimed the convention is "all registers
   preserved by caller" — accurate for the *dispatcher's* `d3-d7/a3-a7`, but
   the handler itself preserves nothing.
2. **No argument passing.** `jmp (a0)` gives the callback no parameters; use
   globals / PC-relative data.
3. **Callback must not call BIOS/GEMDOS.** Documented restriction
   (`bios/xbios.c:769-773`) — re-entering the trap machinery from inside a
   Supexec callback is unsafe.
4. **Supervisor stack is small.** Deep recursion / large locals in the
   callback can overflow the supervisor stack; EmuTOS does not check.

---

## Dependencies

### Requires (must be initialized / functional)

- The XBIOS trap path (already in supervisor mode) and a valid `codeptr`.

### Used by

- Any program peeking/poking hardware, vector tables, MMU state; monitors,
  benchmarks, palette flippers, copy-protection.

---

## Related data structures

- `docs/api/xbios/_dispatcher.md` — the trap entry and the save area.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the no-args,
  run-in-supervisor-mode convention.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `DBG_XBIOS` swaps in the register-preserving C wrapper; production uses the
  bare asm.

---

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-06 design driver — needs ADR-Supexec.** AArch64 has no m68k-style
in-process supervisor flag: user code runs at EL0, kernel at EL1, with no
cheap privilege flip. The TOS `Supexec` semantics must resolve to one of:

1. **Run the callback at EL1** (trap into the kernel and jump to the user
   pointer) — full-memory-access security hole; rejected unless sandboxed.
2. **Pseudo-supervise**: run the callback at EL0 with a curated set of MMIO
   regions (palette, Line-A shadows, hardware we already emulate) mapped
   read/write into the process. Most real `Supexec` uses fall here — the
   working assumption.
3. **Kernel-side pattern translation**: recognise common callback signatures
   (palette swap, screen flip) and perform the intended effect natively;
   foreign callbacks fault gracefully.

ADR to be filed in `docs/MEMORY.md` once PHASE-06 prototyping picks an
approach. Until then `Supexec` is deferred; it is **not** silently stubbed to
`EINVFN`.
