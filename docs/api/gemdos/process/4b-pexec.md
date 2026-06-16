# `Pexec` — GEMDOS 0x4B

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/proc.c:179` (`xexec`).

## EmuTOS source

- **Primary:** `bdos/proc.c:179` — `long xexec(WORD flag, char *path, char *tail, char *env)`
- **Helpers:** `kpgmhdrld()`, `kpgmld()` (`bdos/kpgmld.c`) — header and image load
  / `alloc_env()`, `alloc_tpa()` (`bdos/proc.c:434, 462`) — memory carve
  / `init_pd_fields()`, `init_pd_files()` (`bdos/proc.c:367, 388`) — PD setup
  / `proc_go()` (`bdos/proc.c:535`) — launch
  / `set_owner()`, `xmfree()`, `xopen()`, `xclose()`, `ixsfirst()`
- **Globals:** `run`, `bakbuf` / `errbuf` (longjmp safety), `cur_p` (anti-clobber static)
- **Build config:** `DETECT_NATIVE_FEATURES` (PE_RELOCATE),
  `CONF_WITH_ALT_RAM` (TT-RAM pool selection).

## Source extraction (essence)

```c
long xexec(WORD flag, char *path, char *tail, char *env)
{
    switch (flag) {
    case PE_RELOCATE:        /* 50, internal — see notes */
    case PE_BASEPAGE:        /* 5, basepage only, no flags */
    case PE_BASEPAGEFLAGS:   /* 7, basepage with explicit flags */
    case PE_GO:              /* 4, just launch (tail = PD*)  */
    case PE_GOTHENFREE:      /* 6, launch and re-own to child */
    case PE_LOADGO:          /* 0, load and launch */
    case PE_LOAD:            /* 3, load but don't launch */
    default: return EINVFN;
    }
    /* … see modes table below */
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x004B` |
| WORD args | 7 (WORD `flag` + 3× LONG = 6 WORDs) |
| Stack frame | `+12: env (LONG)`, `+8: tail (LONG)`, `+4: path (LONG)`, `+2: flag (WORD)`, `+0: 0x004B` |

## Inputs

| Pos | Name   | Type   | Range / meaning                                |
|-----|--------|--------|------------------------------------------------|
| 0   | `flag` | WORD   | mode selector — see modes table                |
| 1   | `path` | char * | file path (mode 0/3) **or** flags ULONG (mode 5/7) **or** unused |
| 2   | `tail` | char * | command tail (mode 0/3) **or** PD pointer (mode 4/6) |
| 3   | `env`  | char * | environment block, NULL → inherit from `run`   |

### Modes (`include/bdosdefs.h:24-32`)

| `flag` | Constant            | Behaviour                                                |
|-------:|---------------------|----------------------------------------------------------|
| 0      | `PE_LOADGO`         | Load executable, allocate basepage owned by **child**, launch |
| 3      | `PE_LOAD`           | Load executable, allocate basepage owned by **parent**, return PD; do not launch |
| 4      | `PE_GO`             | Launch already-loaded PD passed in `tail`; ownership unchanged |
| 5      | `PE_BASEPAGE`       | Allocate basepage only (no file load), `flags = 0`       |
| 6      | `PE_GOTHENFREE`     | Like `PE_GO` but re-own basepage + env to child first    |
| 7      | `PE_BASEPAGEFLAGS`  | Allocate basepage, `flags = (ULONG)path`                 |
| 50     | `PE_RELOCATE`       | (NatFeats only) relocate already-loaded image — internal |
| other  | —                   | `EINVFN`                                                 |

## Outputs

| Mode | Return value (LONG)                                     |
|------|---------------------------------------------------------|
| 0    | If launched and child terminated: child's exit code (`rc` from its `Pterm`). |
| 3    | Pointer to allocated PD (cast to `LONG`).               |
| 4    | Pointer to launched PD (does not return until child exits, then exit code). |
| 5,7  | Pointer to allocated PD.                                |
| 6    | Same as `PE_GO`.                                        |
| any  | Negative GEMDOS error code on failure (see Errors).     |

The "return = exit code" path goes through `Pterm` → `termuser` →
parent's saved trap frame → `osif`'s D0 (see
`docs/api/gemdos/_dispatcher.md`).

## Errors

| Code      | Constant   | Cause                                                   |
|----------:|-----------|---------------------------------------------------------|
| `-32`     | `EINVFN`  | unknown `flag`                                          |
| `-33`     | `EFILNF`  | program file not found (`ixsfirst` fails)               |
| `-34`     | `EPTHNF`  | path-not-found (relayed by `xopen`)                     |
| `-35`     | `ENHNDL`  | too many open files (relayed by `xopen`)                |
| `-39`     | `ENSMEM`  | insufficient memory for environment, basepage, or TPA   |
| `-40..`   | various   | I/O errors during `kpgmhdrld`/`kpgmld`                  |
| `-66`     | `EPLFMT`  | invalid program format (header magic mismatch)          |
| `-67`     | `EGSBF`   | bad relocation table                                    |

`kpgmhdrld` reads the program header into a `PGMHDR01`; `kpgmld`
loads the rest, applies relocations, optionally clears BSS.

## Edge cases

### 1. Memory ownership (modes 0 and 3 differ)

```c
owner = (flag == PE_LOADGO) ? p : run;
set_owner(p, owner);
set_owner(env_ptr, owner);
```

- `PE_LOADGO`: child owns the basepage and env. When child calls
  `Pterm`, both are freed by `free_all_owned`.
- `PE_LOAD`: **parent** owns the basepage and env. The child has no
  way to reach this code path itself, so the parent must explicitly
  arrange for `PE_GOTHENFREE` (6) on the eventual launch in order to
  transfer ownership to the child.

`PE_GOTHENFREE` (mode 6):
```c
case PE_GOTHENFREE:
    p = (PD *) tail;
    set_owner(p, p);
    set_owner(p->p_env, p);
    FALLTHROUGH;
case PE_GO:
    proc_go(p);
```

### 2. Environment inheritance

`alloc_env()` (`bdos/proc.c:434`):
- If `env == NULL`: copy parent's `run->p_env` (size determined by
  `envsize()` walking up to double-NUL).
- If `env != NULL`: caller-supplied block is copied verbatim.
- Allocated from the appropriate pool (ST-RAM by default, or TT-RAM
  if program flags include `PF_TTRAMMEM`).

### 3. TPA sizing (`alloc_tpa`, `bdos/proc.c:462`)

```c
needed = hdr.h01_tlen + hdr.h01_dlen + hdr.h01_blen + sizeof(PD);
p = (PD *)alloc_tpa(hdr.h01_flags, needed, &max);
```

`alloc_tpa` finds the **largest** free block that satisfies `needed`
and returns its start + actual size in `*max`. The basepage's
`p_hitpa - p_lowtpa = max`, so the program receives all the
remaining space, not just `needed`. The Atari "give the program
everything available" allocation policy.

`TPASIZE_QUANTUM = 128 KiB` rounds the search granularity.

### 4. longjmp safety during load

```c
memcpy(bakbuf, errbuf, sizeof(errbuf));
if (setjmp(errbuf)) {
    xmfree(cur_p->p_env);
    xmfree(cur_p);
    xclose(fh);
    longjmp(bakbuf, 1);
}
```

The dispatcher's outer `errbuf` is saved into `bakbuf`; `xexec`
installs its own `errbuf` so that any `Rwabs`-failure during
`kpgmld` lands here, lets `xexec` free the partially-allocated child
state, then resumes the original longjmp into the dispatcher with
the original error.

`cur_p` is `static` rather than auto specifically because GCC's
"variable might be clobbered by longjmp" warning otherwise fires —
a comment in the source.

### 5. Loaded program entry conventions

After successful `kpgmld`, `proc_go(p)`:
- Constructs a `gouser_stack` at `p_hitpa - sizeof(gouser_stack)`.
- Stores child PD into `4(sp)` (basepage at standard offset).
- Sets `retaddr = p->p_tbase` so `gouser` "returns" into the program.
- `sr = current SR & 0x0700` (preserves IPL, switches to user mode).
- `a7` = the new stack pointer; `a6` = copy of `a7`; `a5` =
  `p_dbase`; `a4` = `p_bbase`; `a0` = 0 (so accessory-detection
  pattern works); other regs zero.

The supervisor stack for the child is the **shared** `supstk[SUPSIZ]`
(`bdos/proc.c:58`). Only one process runs at a time in classic TOS,
so a single supervisor stack suffices.

### 6. Instruction-cache invalidation

```c
invalidate_instruction_cache(((UBYTE *)cur_p) + sizeof(PD), hdr.h01_tlen);
```

Required on m68040+ where the I-cache might still hold stale data
from a previous TPA tenant. **Only the text segment** is
invalidated; programs that jump into data/BSS/heap "are kindly
invited to do their cache management themselves" (source comment).

NewTOS analogue: AArch64 needs `dc cvau` + `ic ivau` + `dsb ish`
+ `isb` over the loaded text range. The same scope rule applies.

### 7. File close on success

After successful load, `kpgmld` closes the file before returning. On
the failure paths above the explicit `xclose(fh)` covers the cases
where `kpgmld` did not get that far.

### 8. Inherited file handles (`init_pd_files`)

```c
for (i = 0; i < NUMSTD; i++) {
    h = run->p_uft[i];
    if (h > 0) ixforce(i, h, p);     /* dup parent's redirected handle */
    else       p->p_uft[i] = h;       /* copy non-redirected indicator */
}
```

`ixforce` increments the SFT entry's reference count and stores `h`
into the child's `p_uft[i]`. Both parent and child share the file —
either's `Fclose` decrements; the SFT entry vanishes when both close.

### 9. Inherited current directories

```c
for (i = 0; i < NUMCURDIR; i++) {
    int dn = run->p_curdir[i];
    p->p_curdir[i] = dn;
    if (dn) dirtbl[dn].use++;
}
p->p_curdrv = run->p_curdrv;
```

Child inherits parent's CWD per drive and the active drive. Each
non-zero entry bumps the use count in `dirtbl` so DND eviction
respects the new owner.

### 10. `PE_RELOCATE` (mode 50)

Internal use only — invoked from the BIOS bootstrap via NatFeats.
The host bootstrap loads a TOS program at a host-chosen address
and asks EmuTOS to relocate it for that base. Off the critical path
for NewTOS but documented for completeness.

## Dependencies

- Memory: `alloc_env`, `alloc_tpa`, `xmfree`, `set_owner`,
  `xmfremd` (transitively via `Pterm`)
- Filesystem: `ixsfirst`, `xopen`, `xclose`, `kpgmhdrld`, `kpgmld`
- Process: `proc_go`, `gouser` (asm), `termuser` (asm)
- Cache: `invalidate_instruction_cache`
- Used by: shells (Bash-style command launch), AES (program install),
  `system()`-equivalent libc calls.

## Cross-references

- **Compendium:** "Pexec" — agrees on all 8 modes including
  `PE_BASEPAGEFLAGS` (7).
- **Profibuch:** "Pexec (0x4B)" — agrees.
- **`docs/api/gemdos/process/01-pterm.md`** — termination path.
- **`docs/api/structures/basepage.md`** — PD layout.
- **`docs/api/structures/md.md`** — `set_owner` / `m_own`.
- **`docs/api/gemdos/_dispatcher.md`** — outer `errbuf` setjmp.
- **EmuTOS source:** `bdos/kpgmld.c` (program loading + relocation),
  `bdos/rwa.S` (`gouser`/`termuser`).

## Configuration

- `DETECT_NATIVE_FEATURES` enables `PE_RELOCATE`.
- `CONF_WITH_ALT_RAM` enables TT-RAM-aware allocation.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06 implementation notes:
1. **Per-process supervisor stack.** The shared `supstk[SUPSIZ]`
   trick assumes single-task. NewTOS will eventually run multiple
   processes; each PD needs its own kernel-mode stack.
2. **`gouser`/`termuser`.** These are m68k-specific user/super-mode
   transitions. NewTOS native source ports use the AArch64 EL0/EL1
   exception return mechanism. The classic m68k save-area layout is
   only relevant inside the m68k user emulator (PHASE-10+).
3. **`set_owner`.** Currently iterates the MD list to find the
   matching MD by start address. NewTOS will likely keep the same
   approach for ABI fidelity.
