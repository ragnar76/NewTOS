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

## Exit-code matrix: what the `Pexec` caller receives (PROC06, 2026-10-16)

Re-validated against EmuTOS `1a7484ab`. **Read:** `xterm(UWORD rc)`
(`bdos/proc.c:601`), `run->p_dreg[0] = rc` (`:616`) with `LONG p_dreg[1]`
(`include/bdosdefs.h:93`, offset `0x68`), then `termuser` (`bdos/rwa.S:265-267`
`move.l _run,a5; jra ggusr`), which at `bdos/rwa.S:253` reloads
`movem.l P_REGSAVE(a5),d0/a3-a6` (`P_REGSAVE = P_D0SAVE = 0x68`,
`include/asmpd.h:25,27`) — the parent's `d0` is the full 32-bit `p_dreg[0]`.
The dispatcher hands the argument over as `pw[1]` (a `short`,
`bdos/bdosmain.c:674` `(*f->fncall.w)(pw[1])`), and `xterm` narrows it to
`UWORD`, so only the **low 16 bits** of whatever the caller pushed matter.
No other code touches `p_dreg[0]` between `:616` and the parent's resume
(`gouser` is *not* used here, because it would overwrite it with the current
`d0`, comment `bdos/proc.c:611-615`; `bdos/rwa.S:261-267`).
Result: the parent's `Pexec` (modes 0/4/6) receives `rc & 0xFFFF`,
**zero-extended** to 32 bits — **never negative**, never sign-extended.

| Child ends with | EmuTOS `xterm` argument | Parent `Pexec` D0 | Source |
|-----------------|-------------------------|-------------------|--------|
| `Pterm(0)` / `Pterm0` | 0 | `0x00000000` | `bdos/proc.c:588-591` (`x0term` → `xterm(0)`) |
| `Pterm(1)` | 1 | `0x00000001` | `:601,616` |
| `Pterm(0x7FFF)` | 0x7FFF | `0x00007FFF` | `:601,616` |
| `Pterm(0x8000)` (rc > 0x7FFF) | 0x8000 | `0x00008000` = 32768 | `:601,616` |
| `Pterm(-1)` | 0xFFFF | `0x0000FFFF` = 65535 | `:601,616` |
| `Pterm(-32)` | 0xFFE0 | `0x0000FFE0` = 65504 | `:601,616` |
| `Ptermres(blkln, rc)` | `rc` (WORD) | `rc & 0xFFFF`, same as `Pterm` | `bdos/proc.c:624-634` (`xterm(rc)` at `:633`) |
| trap #2 with `d0.w == 0` (CP/M-68K relic) | 0 (`clr -(sp)`) | `0` | `bdos/rwa.S:145-155` |
| Ctrl-C at console input | `xterm(-32)` | `0x0000FFE0` = 65504 | `bdos/console.c:101` (`#define terminate() xterm(-32)`) |
| Crash (exception → `dopanic`) | `kill_program` pushes `move.w #-1` then `Pterm` (`trap #1`) | `0x0000FFFF` = 65535 | `bios/panicasm.S:96-99`, `bios/kprint.c:594` |

For a `PE_GO`/`PE_GOTHENFREE` child the same: `proc_go` (`bdos/proc.c:535-578`)
sets `p->p_parent = run`, so `xterm`'s `run = run->p_parent` selects the
`Pexec` caller whose `p_dreg[0]` is written. For `PE_LOADGO` (`:358`) likewise.
The "should not return" `return (long)p` at `:248` is not reached.

Inferred (not read as a single statement): the m68k caller of `Pterm` pushes a
WORD; a value outside 16 bits cannot be expressed by an m68k program, so the
`0x12345 → 0x2345` row of the host tests below is a *native* (AArch64) ABI
consequence of the `UWORD` narrowing, not an observed EmuTOS run.

### NewTOS state and divergences (checked 2026-10-16, `main` @ `fbc86f32`)

Conforming (all deliver `rc & 0xFFFF` zero-extended):
- Explicit `Pterm`/`Pterm0`/`Ptermres` traps: `handle_pterm` masks —
  `0x31 => (frame.gpr[1] & 0xFFFF)`, `_ => (frame.gpr[0] & 0xFFFF)`,
  `0x00 => 0` (`kernel/src/arch/aarch64/exceptions.rs:1271-1275`); the masked
  value goes to `finish_terminate`, which writes it to the `Pexec` caller's
  `x0` (`:1538`). The `etv_term` upcall path carries the already-masked `rc`
  (`:1319`, `:1757-1768`).
- Crash: `kill_program` sets `x0 = -1` and calls `handle_pterm(.., 0x4C)`
  (`:1445-1452`), so the mask applies -> 65535. `uaccess_fault_tail` ends in
  `kill_program` (`:1433`).
- Kernel-launched children (the AES shell, PROC05):
  `launch::record_kernel_child_exit` -> `pterm::parent_exit_code`
  (`kernel/src/launch.rs:88`, `tos-api/src/bdos/process/pterm.rs:99`).

**Divergences (to fix):**
1. **Ctrl-C (`-32`) to a real `Pexec` caller is NOT masked.**
   `tos-api/src/gemdos/console.rs:205` `CTRLC_RC = -32` ->
   `request_terminate` -> `complete_call_tail` (`exceptions.rs:680-681`) ->
   `terminate_and_redirect(frame, rc)` (`:1347`) -> `finish_terminate(frame, rc, ..)`
   -> `frame.gpr[0] = rc as i64 as u64` (`:1538`): the caller's `x0` becomes
   `0xFFFF_FFFF_FFFF_FFE0`, i.e. **-32 as LONG**. EmuTOS: 65504. This is the
   case in TASK/PROC06.
2. **Upcall-nesting overflow (`-998`)** takes the same unmasked path
   (`exceptions.rs:1641`) -> caller sees -998; masked it would be 64538. EmuTOS
   has no equivalent (NewTOS-specific code, `-998` is not an EmuTOS value);
   apply the same mask for consistency (the kernel-launch branch already does,
   `launch.rs:88`).
3. Documentation only: the earlier `4b-pexec.md` "WORD **sign-extended** into
   `x0`" statement (Native ABI paragraph) contradicted this spec and the
   source; corrected 2026-10-16 (see `4b-pexec.md`).

Root cause: only the `Pterm` trap entry masks, not the common tail.
Fix (Developer, not done here): mask once in `finish_terminate`
(`pterm::parent_exit_code(rc)` at `:1538`/`:1544`) or in
`terminate_and_redirect`; every path then yields the EmuTOS value. The mask
at `handle_pterm` stays harmless.

**Fixed 2026-10-16 (TASK/PROC06):** `finish_terminate` writes
`pterm::exit_code_for_caller(rc)` (= `parent_exit_code(rc)` zero-extended to
`u64`) into the `Pexec` caller's `x0`, so divergences 1 and 2 are closed; see
"NewTOS implementation notes" below.

### Proposed host tests (PROC06)

Pure-function level (`tos-api/tests/`, next to `proc05_shell_exit_code.rs`):
1. `parent_exit_code`: 0->0, 1->1, 0x7FFF->0x7FFF, 0x8000->32768,
   -1->65535, -32->65504, -998->64538, 0x12345->0x2345, i32::MIN->0,
   result always in `0..=0xFFFF`.
2. Ctrl-C constant: `parent_exit_code(CTRLC_RC) == 65504` (requires
   `CTRLC_RC` visible to the test or an equivalent literal).

Frame level (needs the tail logic factored so that it is host-testable — e.g.
a pure `exit_code_for_caller(rc: i32) -> u64` used at `exceptions.rs:1538`;
otherwise only a QEMU selftest can observe it):
3. `exit_code_for_caller(-32) == 65504u64` and `(-1) == 65535`, `(-998) == 64538`;
   upper 48 bits of the u64 are always 0.

On target (QEMU selftest / APITEST child set):
4. Child `Pterm(-1)` -> parent `Pexec(0,..)` == 65535 (positive).
5. Child `Pterm(0x8000)` -> 32768; child `Pterm0` -> 0.
6. Child `Ptermres(n, -2)` -> 65534.
7. Crashing child (e.g. `sdk/examples/crchild.c`/`crashtst.c`, which already
   document 0x0000FFFF) -> 65535.
8. Ctrl-C injected into a child blocked in `Cconin` (the `selftest_console`
   scenario, `kernel/src/selftest_console.rs:202` only checks the pending
   `-32`, not what the parent gets) -> parent `Pexec` == 65504.

APITEST (`sdk/apps/apitest/apitest.c`): no existing check changes. The only
`Pexec` check, T29 (`:1433-1460`), runs `DTACHILD.NCP` (normal exit) and uses
`prc < 0` as "child did not run" — unaffected unless a child terminates by
Ctrl-C/overflow, where today it would be misread as a load error. New case
proposal: T-new "Pexec returns zero-extended child rc", using a child that
`Pterm(-1)` and asserting `prc == 65535`.

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

ARCH03 stage 2d (2026-10-05, plan E15): `free_all_owned` drops the process from
every block it owns rather than freeing every block tagged with it — a block
another owner still holds (the basepage and environment of a `Pexec` 4 child,
whose caller owns them too) survives, and its EmuTOS tag passes to that owner.
With one owner per block, the only case EmuTOS has, the result is identical
(`docs/api/gemdos/memory/49-mfree.md`, "several owners").

TASK/PROC03 spec (2026-10-18, user decision "as EmuTOS"): `Pterm` must return
through `p_parent` as `bdos/proc.c:609,616` does, and `run` follows EmuTOS
(unchanged while an accessory or AES code executes, the accdesk/AES/EmuCON PDs
as `run` in kernel context). The EmuTOS facts — what the resumed save area is,
what happens when a program changed its own `p_parent` (unguarded; a started
process's `p_parent` can only be changed by code with write access to the PD),
the `Pterm` of an accessory terminating `run` — and the points the Developer
must decide are in
[`_run-ownership-curdir.md`](./_run-ownership-curdir.md) §2 and §6.3 (D-5).
NewTOS keys the parent's saved context by the child (`parent_slot`,
`tos-api/src/bdos/process/pterm.rs:246-260`); EmuTOS keys it by the PD that
was `run` at the last trap #1 (`bdos/rwa.S:188-224,262-267`).

**Built 2026-10-18 (TASK/PROC03 B4, decision D-5 in
[`_run-ownership-curdir.md`](./_run-ownership-curdir.md) §6.4):** `Pterm`,
`Pterm0`, `Ptermres`, the console Ctrl-C and the crash path read the dying
process's `p_parent` before the teardown (`pterm::return_target`). Unchanged
→ the recorded parent resumes; changed to an ancestor on the launch chain →
the processes between are terminated and the ancestor's `Pexec` returns the
exit code (`PtermResume::unwind`, `terminate_unwound`, the loop in
`finish_terminate`); anything else → the recorded parent resumes and the
kernel logs `[pterm] p_parent 0x… names no resume point …`.

TASK/PROC02 (2026-10-06): the file half of `ixterm` (`bdos/proc.c:116-123`) —
`xclose` of every `p_uft[i] > 0`, then of every `sft[]` entry owned by the
dying PD — runs in `terminate_inner` (`Pterm`, `Pterm0`, `Ptermres`) before
the memory sweep (`file::close_process_files`). Before PROC02 files a process
left open stayed open after its `Pterm`. The current-directory half
(`decr_curdir_usage` of every `p_curdir[i]`, `bdos/proc.c:126-132`) runs since
TASK/PROC04 point 2 (2026-10-18), right after the files and before the memory
(`curdir::release_pd`; [`_run-ownership-curdir.md`](./_run-ownership-curdir.md)
§4.5, §6.5).

TASK/PROC06 (2026-10-16): the exit code is masked once in the shared
terminate tail. `finish_terminate` (`kernel/src/arch/aarch64/exceptions.rs`)
writes `tos_api::bdos::process::pterm::exit_code_for_caller(rc)` — the low 16
bits, zero-extended into the 64-bit `x0` — on its `Pexec`-caller branch; the
kernel-launch branch already stored `parent_exit_code(rc)`
(`launch::record_kernel_child_exit`, PROC05). Every terminate path now
delivers the EmuTOS value: `Pterm`/`Pterm0`/`Ptermres` (unchanged, already
masked at `handle_pterm`), a crash (`-1` → 65535, unchanged), the console
Ctrl-C (`-32` → **65504**, was −32) and the upcall-nesting overflow (`-998` →
**64538**, was −998). `CTRLC_RC` is `pub` in `tos_api::gemdos::console` for the
host test. Tests: `tos-api/tests/proc06_pexec_exit_code.rs` (host-test list
items 1–3); on target APITEST T33 (items 4–5: `C:\PTERMCH.NCP` ending with
`Pterm(-1)` / `Pterm(0x8000)` / `Pterm0` → 65535 / 32768 / 0) and
`C:\CTRLCPAR.NCP` (item 8: a child ended by Ctrl-C in `Cconin` → 65504; needs
a key, so not in APITEST); evidence `measurements/measurement-2026-10-16-proc06/`.
Items 6 (`Ptermres(n, -2)`) and 7 (crashing child) are not run as new cases:
6 goes through the same masked `handle_pterm` arm as `Pterm`, 7 through
`kill_program` → `handle_pterm`, both unchanged by PROC06.

Visible effects of the change (all toward EmuTOS):
- **EmuCON `run`** (`kernel/src/shell.rs` `cmd_run`): a child ended by Ctrl-C
  used to print `Cannot run program (error 32).` (the `rc < 0` branch); it now
  prints `[program exited]`, as for any other exit code.
- **mintlib `spawn*`/`system`** (`sdk/libc/overlay/mintlib/spawn.c:475-493`):
  a Ctrl-C'd child used to come back as `-1` with `errno = 32` (the `rval < 0`
  "Pexec error" branch); it now returns 65504 like EmuTOS. The same holds for
  any `Pexec` caller that tests `< 0` for a load error (APITEST T29's
  "child did not run").
- The AES shell (`sh_ldapp`, kernel-launched child) is unchanged — it already
  saw 65504 (PROC05).

