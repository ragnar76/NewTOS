# `Pterm` (GEMDOS 0x4C)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/proc.c:601` (`xterm`) and its cleanup helper
> `bdos/proc.c:110` (`ixterm`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** trap ID/ABI, "does not return" semantics, the exit code
> delivered to the parent's `Pexec` in D0/`x0` (UWORD zero-extended to LONG),
> the fact that **all** the process's open files, current-dir references and
> `Malloc`ed memory are released, and that the user/GEM termination handler at
> vector `0x102` (`etv_term`, byte address `0x408`) is invoked first.
> **Reference (NewTOS reimplements freely):** `ixterm`'s exact cleanup order,
> the `run = run->p_parent` / `termuser`/`gouser` m68k unwind, `protect_v`,
> `free_all_owned`'s `m_own` walk, `supstk`.

## EmuTOS source

**Primary location:** `bdos/proc.c:601` — `void xterm(UWORD rc)`.

**Installed handler / vector:** (none as a trap) — direct `funcs[]` entry `0x4C`
= `xterm`, `wparms = 1` (`bdos/bdosmain.c:272`). **But** `xterm` *reads* an
installed vector: `Setexc(0x102, -1)` fetches the user/GEM termination handler
at exception number `0x102` = byte address `0x408` = `_etv_term`
(`tosvars.ld:35`, `include/tosvars.h:112`) and calls it before dismantling the
process. (`Pterm0` reaches `xterm` both via `funcs[0]` and via the trap-2
CP/M-68K path, `bdos/rwa.S`; see [`00-pterm0.md`](00-pterm0.md).)

**Helpers called:**
- `bdos/proc.c:110` — `ixterm()` — close files, drop current-dir refs, free
  owned memory.
- `bdos/proc.c:92` — `free_all_owned()` — walk a pool's `mp_mal` and `freeit`
  every MD whose `m_own` is the dying PD.
- `util/miscasm.S:213` — `protect_v()` — call the term handler with `d2`/`a2`
  preserved.
- `bdos/rwa.S` — `termuser()` — re-enter the parent's saved trap-return frame
  (the counterpart of `gouser`, `bdos/proc.c:504-526`).
- `bdos/fsopnclo.c` — `xclose()`; `bdos/fsdir.c` — `decr_curdir_usage()`.
- `include/biosbind.h:24` — `Setexc()` = BIOS 5 (`bios/bios.c:1070`); with
  `vector == -1` it returns the current vector **without** modifying it.

**Headers / type definitions used:**
- `include/bdosdefs.h:65-97` — `PD`; `p_uft[NUMSTD]`, `p_curdir[NUMCURDIR]`,
  `p_dreg[0]`, `p_parent`.
- `bdos/fs.h:43-45` — `NUMSTD=6`, `OPNFILES=75`; `include/bdosdefs.h:61-62` —
  `NUMSTD`, `NUMCURDIR = BLKDEVNUM = 26`.

**System variables / globals read:**
- `run` (`bdos/proc.c:604`) — the dying process, and its `p_parent`, `p_uft`,
  `p_curdir`.
- `sft[]` — scanned for entries owned by the dying PD.
- The vector at `0x102`/`0x408` (`etv_term`) — the user term handler.
- `pmd`/`pmdalt` — the memory pools walked by `free_all_owned`.

**System variables / globals written:**
- `run` — reassigned to `run->p_parent` **before** cleanup (`bdos/proc.c:609`).
- `run->p_dreg[0]` (the parent's saved-D0 slot) = `rc` (`bdos/proc.c:616`).
- `sft[]` / `dirtbl[]` use counts — released by the closes/decrements.
- `pmd`/`pmdalt` free lists — the dying process's blocks are returned.

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — adds the second `free_all_owned(r, &pmdalt)` call
  (`bdos/proc.c:137-140`).

---

## Source extraction summary

`Pterm` terminates the calling process and hands a 16-bit exit code back to its
parent. EmuTOS first fetches the user/GEM termination handler installed at vector
`0x102` (via `Setexc(0x102, -1)`, which reads without replacing) and calls it
through `protect_v` (which preserves `d2`/`a2`). This is the documented hook for
C-runtime `atexit`-style cleanup; if the handler returns, termination continues.

It then swaps `run` to the parent (`run = run->p_parent`) **before** dismantling
the old process via `ixterm`. `ixterm` closes every open file — the six standard
handles `p_uft[0..5]` that were redirected to files (positive values), then every
`sft[]` entry whose owner is the dying PD — decrements the use count of each
current directory the process referenced (`p_curdir[]` into `dirtbl[]`), and
finally frees every memory block the process owns by walking `mp_mal` and
`freeit`-ing each MD with `m_own == r` (`free_all_owned`), across ST-RAM and, if
present, alt-RAM.

Finally it stores the exit code into the parent's saved-D0 slot
(`run->p_dreg[0] = rc`) and calls `termuser`, which re-enters the parent's saved
trap frame so the parent's originating `Pexec` (mode 0/4/6) returns with `rc` in
D0. `xterm` is declared `void` and never returns to its own caller.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x4C
- **Stack frame on entry (m68k):**
  ```
  +2: rc  (WORD)
   0: function_id (word) = 0x004C
  ```
- **Total parameter bytes:** 2 (WORD) → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x4C` in `x8`; `rc` in `x0`. **No return** —
  control transfers to the parent, whose `Pexec` receives `rc` (WORD,
  zero-extended to LONG) in `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `rc` | `UWORD` (16) — exit code |

---

## Inputs

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| arg0     | `rc` | `UWORD` | 2 | `bdos/proc.c:601` | exit code returned to the parent |

`rc` is declared `UWORD` (16-bit, unsigned). It is not validated.

---

## Outputs

### Return value

- **Register:** D0 / `x0` — but delivered to the **parent**, not returned to the
  caller.
- **Type:** the parent's `Pexec` sees a LONG.
- **Encoding:** `rc` is stored into `run->p_dreg[0]` (a LONG field) as a plain
  assignment, so the 16-bit `rc` is **zero-extended** to 32 bits — the high word
  is always 0 (`bdos/proc.c:616`; see D-005).
- **Source location:** `bdos/proc.c:616` (`run->p_dreg[0] = rc`) → `termuser`.

### Side effects on global state

- `run` ← `run->p_parent` (`bdos/proc.c:609`).
- All the dying process's open files closed; its current-dir references dropped;
  all its `m_own`-tagged MDs freed in `pmd` (and `pmdalt` if present)
  (`ixterm`, `bdos/proc.c:110-141`).
- The parent's `p_dreg[0]` set to `rc`.

### Side effects on hardware

- Flushes/writes back via `xclose` (dirty file buffers of the closed handles).
- The user term handler at `0x102` may touch hardware (application-defined).

---

## Error conditions

`Pterm` **cannot fail** — there is no error return; the function does not return
at all. The only pathological case is structural, not an error code:

| Condition | Source location | Result |
|-----------|-----------------|--------|
| Dying process has no parent (`p_parent == NULL`) | `bdos/proc.c:609,616` | `run` becomes NULL and `run->p_dreg[0] = rc` dereferences NULL — EmuTOS does not guard this (the boot process is not expected to `Pterm`). |

**Error paths not in Compendium:** none — Compendium agrees `Pterm` does not
return and has no error code.

---

## Verification criteria

- **Nominal exit code:** a child launched by `Pexec(0, ...)` that calls
  `Pterm(0x1234)` causes the parent's `Pexec` to return `0x00001234` (LONG, high
  word 0).
- **Zero exit:** `Pterm(0)` → parent's `Pexec` returns `0`.
- **File cleanup:** a child that `Fopen`s files and then `Pterm`s leaves no
  `sft[]` entry owned by it; all its standard-handle redirections are undone.
- **Memory cleanup:** every block the child obtained via `Malloc`/`Mxalloc`, plus
  its basepage+env (for a `PE_LOADGO` child), is freed after `Pterm` — a
  subsequent `Malloc(-1)` reflects the reclaimed space.
- **Current-dir cleanup:** each `dirtbl[]` slot the child referenced has its use
  count decremented.
- **Term handler:** if a handler is installed at vector `0x102`, it is invoked
  once, before any file/memory teardown, with `d2`/`a2` preserved.
- **Does not return:** `Pterm` never returns to its own caller; control resumes
  in the parent.

---

## Edge cases discovered in source

1. **User/GEM term handler runs first, unconditionally.** `Setexc(0x102, -1)`
   reads the handler at byte address `0x408` (`_etv_term`) without replacing it
   and `protect_v` calls it before `ixterm` (`bdos/proc.c:606-607`). The handler
   may run cleanup and return (termination continues) or not return (jump
   elsewhere; jumping back into the dying process is undefined). This is the
   `atexit`/GEM `etv_term` mechanism.

2. **Parent restore happens *before* teardown.** `run = run->p_parent` precedes
   `ixterm(p)` (`bdos/proc.c:609-610`). If `xclose` inside `ixterm` hits a hard
   error and `longjmp`s into the dispatcher's `errbuf`, `run` already points at a
   healthy parent — the dying PD's memory is momentarily leaked but the system
   stays consistent (`bdos/proc.c:609-610`; dispatcher setjmp in
   `docs/api/gemdos/_dispatcher.md §8`).

3. **Standard-handle close rule.** Only `p_uft[i] > 0` handles are closed
   (positive = redirected to a real file); non-positive values are BIOS
   character-device indicators (`H_Console`/`H_Aux`/`H_Print`) that need no close
   (`bdos/proc.c:117-119`).

4. **SFT sweep catches everything the process opened.** After the std handles,
   `ixterm` walks all `OPNFILES (75)` `sft[]` entries and closes any with
   `f_own == r`, regardless of whether a handle still references them
   (`bdos/proc.c:121-123`).

5. **Exit code is zero-extended, not sign-extended (D-005).** `p_dreg[0]` is a
   LONG; assigning a `UWORD` clears the high word. Programs that read the parent
   `Pexec` result as a full LONG see a guaranteed-zero high word under EmuTOS,
   where classic Atari docs say "high word undefined".

6. **No-parent boot process is unguarded.** The very first process must not call
   `Pterm`; EmuTOS would dereference a NULL `run`. NewTOS installs a sentinel
   init PD (see NewTOS notes).

---

## Dependencies

### Requires (must be initialized / functional)

- `run` valid with a valid `p_parent`.
- `Setexc` (BIOS 5) + `protect_v`; `xclose`; `decr_curdir_usage`;
  `free_all_owned` (needs `pmd`/`pmdalt` initialised).
- `termuser`/`gouser` (m68k) or the NewTOS EL0 exception-return path, and the
  parent's saved trap frame.

### Used by

- Every program at exit (libc `exit()`), `Pterm0` (delegates with `rc=0`),
  `Ptermres` (chains to `xterm` after reserving its blocks).

---

## Related data structures

- `docs/api/structures/basepage.md` — `p_dreg[0]`, `p_parent`, `p_uft`,
  `p_curdir`.
- `docs/api/structures/md.md` — the `m_own`-based `free_all_owned`.
- `docs/api/gemdos/_state.md §1` (`run`), §2 (handles), §5 (current dirs),
  §8 (pools + the `m_own` ownership caveat).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Documents `Pterm` as
  non-returning with a 16-bit exit code and the `etv_term`/vector-`0x102`
  handler — consistent with EmuTOS; the exit-code high-word behaviour differs
  (see below).

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

### Discrepancies

- **D-005** — `Pterm` exit-code width: EmuTOS zero-extends `rc` into the LONG
  `p_dreg[0]`; Atari docs say "high word undefined". NewTOS follows EmuTOS
  (deterministic zero high word). See `docs/api/_discrepancies.md#D-005`.

---

## Configuration / version notes

- `CONF_WITH_ALT_RAM` — second-pool `free_all_owned`.
- `OPNFILES = 75`, `NUMSTD = 6`, `NUMCURDIR = BLKDEVNUM = 26` (**correction**:
  the earlier draft said `NUMCURDIR = 16`; it is 26 — see D-025).

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06 implementation note: the `Setexc(0x102, -1)` read mechanism must be
wired through the NewTOS exception-table abstraction — vector byte address
`0x408` is meaningless on AArch64. Map exception **number** `0x102` (`etv_term`)
to a slot in the per-process handler table. The `termuser`/`gouser` unwind is
replaced by the AArch64 EL0/EL1 exception-return path; the m68k save-area layout
matters only inside the future m68k emulator (Phase 10+). The exit-code delivery
into the parent's `p_dreg[0]` slot remains the contract way the child's `rc`
reaches the parent's `Pexec`.
