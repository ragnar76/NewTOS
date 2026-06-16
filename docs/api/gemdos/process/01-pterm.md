# `Pterm` — GEMDOS 0x4C

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/proc.c:601` (`xterm`).

## EmuTOS source

- **Primary:** `bdos/proc.c:601` — `void xterm(UWORD rc)`
- **Helper:** `ixterm()` (`bdos/proc.c:110`) — actual cleanup
- **Helper:** `protect_v()` (`util/miscasm.S:213`) — invoke user term
  handler with d2/a2 preserved
- **Helper:** `termuser()` (`bdos/rwa.S`) — re-enters the trap return
  path of the parent
- **Globals read/written:** `run` (current PD), `sft[]` (system file
  table), `dirtbl[]` (current-directory table), `pmd`/`pmdalt` (memory
  pools)
- **Build config:** `CONF_WITH_ALT_RAM` selects whether `pmdalt` is
  also walked.

## Source extraction

```c
void xterm(UWORD rc)
{
    PFVOID userterm;
    PD *p = run;

    userterm = (PFVOID)Setexc(0x102, (long)-1L);  /* get user term handler */
    protect_v((PFLONG)userterm);                  /* call it (d2/a2 saved) */

    run = run->p_parent;
    ixterm(p);

    run->p_dreg[0] = rc;
    termuser();
}
```

### `ixterm(PD *r)` — cleanup logic (`bdos/proc.c:110`)

```c
static void ixterm(PD *r)
{
    /* close all open standard handles */
    for (i = 0; i < NUMSTD; i++)
        if ((h = r->p_uft[i]) > 0) xclose(h);

    /* close all open non-standard files owned by r */
    for (i = 0; i < OPNFILES; i++)
        if (r == sft[i].f_own) xclose(i + NUMSTD);

    /* drop current-directory references */
    for (i = 0; i < NUMCURDIR; i++)
        if ((h = r->p_curdir[i]) != 0)
            decr_curdir_usage(h);

    /* free everything in pmd allocated to r */
    free_all_owned(r, &pmd);
#if CONF_WITH_ALT_RAM
    if (has_alt_ram)
        free_all_owned(r, &pmdalt);
#endif
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x004C` |
| WORD args | 1 |
| Stack frame | `+2: rc (WORD)`, `+0: 0x004C (fn)` |

## Inputs

| Pos | Name | Type  | Range / meaning                      |
|-----|------|-------|--------------------------------------|
| 0   | `rc` | UWORD | exit code returned to parent in `D0` |

EmuTOS declares the parameter as `UWORD` (16 bits, unsigned). The
parent receives it sign-extended into a LONG in `D0` — see Compendium
discrepancy below.

## Outputs

**Does not return.** Stored as `run->p_dreg[0] = rc;` before
`termuser()` re-enters the parent's trap return path. From the
parent's perspective, its `Pexec(0, ...)` returns with `rc` in `D0`.

## Errors

None — termination always succeeds. If `run->p_parent == NULL`
(boot process), `run = run->p_parent` makes `run` NULL and
subsequent `run->p_dreg[0] = rc` would dereference NULL — see edge
cases.

## Edge cases

### 1. User-term handler at exception vector 0x102 (= 0x408)

Before the process is dismantled, `xterm` reads the handler stored
at exception number `0x102` (vector address `0x408`) and calls it
with `d2`/`a2` preserved. The handler may:
- Run any cleanup (flush buffers, save state).
- **Not return**: jump elsewhere (jumping back into the now-dying
  process is undefined).
- **Return**: `xterm` continues with `ixterm` and `termuser`.

This is the documented mechanism for C-runtime-library `atexit`
handlers and similar.

### 2. File handle cleanup

- All six standard handles `p_uft[0..5]` are closed if positive
  (positive = redirected to a file; non-positive = unredirected
  BIOS-handle indicator, no close needed).
- The system file table is then walked: any entry whose `f_own ==
  this PD` is closed by `xclose(i + NUMSTD)`. This catches every
  file the process opened with `Fopen`/`Fcreate` regardless of
  redirection.

### 3. Current-directory table

`p_curdir[d]` is an index into `dirtbl[]`. Each non-zero entry is
released via `decr_curdir_usage`. When the use count drops to zero,
the DND becomes scavengeable.

### 4. Memory free

`free_all_owned(r, &pmd)` walks `pmd.mp_mal` and frees every MD with
`m_own == r`. This includes:
- The basepage / TPA itself (allocated by the parent's `Pexec`).
- The environment block.
- Every block obtained via `Malloc`/`Mxalloc`.

Done **after** parent restoration so that if the user-term handler
or the `decr_curdir_usage` path needs the dying process's memory it
is still mapped — though by that time the user-term handler has
already returned.

### 5. Parent restoration vs. PD pointer

The order matters:
```
run = run->p_parent;  /* swap to parent first  */
ixterm(p);            /* then dismantle 'p'    */
```
Because `ixterm` calls `xclose`, and `xclose` may longjmp into
`errbuf` on hard-error (see `docs/api/gemdos/_dispatcher.md` setjmp
discussion). If `run` still pointed at the dying process when
longjmp fired, the system would be inconsistent. With the swap done
first, a longjmp lands the parent in a sane state with the dying
PD's memory still allocated (a leak — but recoverable; the parent
can `Pterm` itself to release everything).

### 6. `D0` width

`p->p_dreg[0]` is a `LONG`. Storing a `UWORD` `rc` zero-extends to
32 bits. **EmuTOS does not sign-extend** — Compendium states the
return code is "returned in the low word of D0, high word
undefined". Practical impact: parents reading exit code via C
`(int)Pexec(...)` see the right value if `int` is 16-bit; on
32-bit-`int` toolchains they see `rc` zero-extended. (See discrepancy
log entry below.)

### 7. `termuser()` mechanics

`termuser` is implemented in `bdos/rwa.S` as the entry into the
`gouser` save-area unwind sequence — it pretends the current trap
is returning to the parent, walking the saved register frame stored
in the parent's PD. The parent therefore resumes exactly as if its
`Pexec` (or other `Pterm`-yielding call) returned normally.

### 8. Boot process (no parent)

If the very first process (the one launched by the BIOS during
init) calls `Pterm`, `run->p_parent` is `NULL`. The next
`run = NULL` followed by `run->p_dreg[0] = rc` dereferences NULL.
On classic Atari this is "system halt" territory; EmuTOS does not
guard against it explicitly.

NewTOS will install a sentinel "init" PD that catches `Pterm` from
the AESPRG / shell and reboots, rather than crashing.

## Dependencies

- `Setexc(0x102, -1)` — reads exception vector
- `protect_v` — register-preserving thunk
- `xclose` — close one open file
- `decr_curdir_usage` — release directory cache reference
- `free_all_owned` (`bdos/proc.c:88`) — walk allocated list and free
- `termuser` (`bdos/rwa.S`) — re-enter parent's trap unwind

Used by: every program at exit time (libc-equivalent `exit()`),
`Pterm0` (delegates with `rc=0`), `Ptermres` (chains to it).

## Discrepancies

- **D-005 (NEW): `Pterm` rc width** — EmuTOS treats `rc` as `UWORD`
  and stores it directly into the LONG `p_dreg[0]`, zero-extending.
  Atari documentation specifies "16-bit return code, high word
  undefined". Behavioural difference for code that reads `D0` as a
  full `LONG` and expects sign-extension or undefined high bits.
  → log entry to add to `docs/api/_discrepancies.md`.

## Cross-references

- **Compendium:** "Pterm" — agrees on signature and 0x102 handler.
- **Profibuch:** "Pterm (0x4C)" — agrees, includes example user-term
  handler.
- **`docs/api/gemdos/process/00-pterm0.md`** — wrapper.
- **`docs/api/gemdos/process/03-ptermres.md`** — chains via `xterm`.
- **`docs/api/structures/basepage.md`** — `p_dreg[0]`, `p_parent`.
- **`docs/api/structures/md.md`** — `m_own`-based free.
- **`docs/api/gemdos/_dispatcher.md`** — `osif` longjmp setup.

## Configuration

- `CONF_WITH_ALT_RAM` — adds the second-pool `free_all_owned` call.
- `OPNFILES = 75`, `NUMCURDIR = 16`, `NUMSTD = 6` — pool sizes.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06 implementation note: the `Setexc(0x102, -1)` read mechanism
must be wired through the NewTOS exception-table abstraction — we
cannot literally use vector address 0x408 on AArch64. Map exception
*number* 0x102 to a slot in our per-process handler table.
