# `Ptermres` — GEMDOS 0x31

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/proc.c:624` (`xtermres`).

## EmuTOS source

- **Primary:** `bdos/proc.c:624` — `WORD xtermres(long blkln, WORD rc)`
- **Helpers:** `xsetblk()` (`bdos/mem.c`) — shrink owning MD,
  `reserve_blocks()` (`bdos/proc.c:75`) — remove MDs from the
  allocated list, `xterm()` — chain through to standard termination
- **Globals:** `run`, `pmd`, `pmdalt`
- **Build config:** `CONF_WITH_ALT_RAM`.

## Source extraction

```c
WORD xtermres(long blkln, WORD rc)
{
    xsetblk(0, run, blkln);

    reserve_blocks(run, &pmd);
#if CONF_WITH_ALT_RAM
    if (has_alt_ram)
        reserve_blocks(run, &pmdalt);
#endif
    xterm(rc);
}

static void reserve_blocks(PD *p, MPB *mpb)
{
    MD *m, **q;
    for (m = *(q = &mpb->mp_mal); m; m = *q) {
        if (m->m_own == p) {
            *q = m->m_link;       /* unlink from allocated list */
            xmfremd(m);           /* free the MD struct itself */
        } else {
            q = &m->m_link;
        }
    }
}
```

The function never returns despite the `WORD` return type.
The signature is historical; callers ignore the return value.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0031` |
| WORD args | 3 (LONG `blkln` = 2 WORDs + WORD `rc`) |
| Stack frame | `+6: rc (WORD)`, `+2: blkln (LONG)`, `+0: 0x0031` |

LONG arguments to GEMDOS are passed as **two WORDs in big-endian
order** (high WORD at lower address). The dispatcher's wparms / lparms
encoding decodes them into the C ABI; see
`docs/api/gemdos/_dispatcher.md`.

## Inputs

| Pos | Name    | Type | Range / meaning                                      |
|-----|---------|------|------------------------------------------------------|
| 0   | `blkln` | LONG | New size in bytes for the basepage's TPA block       |
| 1   | `rc`    | WORD | exit code passed to parent (see `Pterm` spec)        |

`blkln` should include the basepage itself (256 bytes minimum) plus
any code/data the resident portion needs. The classic TSR pattern:

```c
extern char _end;        /* set by linker; end of bss */
long size = &_end - basepage;
Ptermres(size, 0);
```

## Outputs

**Does not return.** Same behaviour as `Pterm`: control returns to
the parent. The exit code semantics, user-term handler invocation,
file/curdir cleanup are inherited from the chained `xterm` call.

## Errors

None directly. `xsetblk(0, run, blkln)` could theoretically return
an error if `blkln` is invalid (e.g., larger than the original
allocation), but the return is ignored. EmuTOS silently truncates
or accepts whatever `xsetblk` does — see edge cases.

## Edge cases

### 1. The "reserve" trick

`reserve_blocks` walks the allocated list and **removes** every MD
owned by `run` *without* calling the free helper. The MD struct
itself is reclaimed (`xmfremd`) but the memory region it described
stays out of both lists — effectively a permanent allocation that no
process owns. After the subsequent `xterm`, `free_all_owned` finds
*nothing* to free (the MDs are no longer in `pmd.mp_mal`), so the
resident region survives even though the dying PD's ownership flag
has been cleared.

This is the technical heart of TSR (terminate-and-stay-resident).

### 2. Order of operations

```
1. xsetblk(0, run, blkln)  /* shrink the basepage's MD to blkln  */
2. reserve_blocks(run, pmd)/* remove all run-owned MDs from list */
3. xterm(rc)               /* normal termination from here       */
```

Step 1 must happen *before* step 2: `xsetblk` adjusts the MD's
length, then step 2 unlinks it. If reversed, step 1 would target a
no-longer-listed MD.

The basepage MD becomes "owner-less but allocated" after step 2;
its memory remains valid until the system reboots or another
process explicitly hooks/replaces it.

### 3. Standard handles and curdirs

`xterm` still runs `ixterm`, which closes all open file handles and
decrements current-directory references. **A TSR cannot keep files
open across `Ptermres`** — the OS closes them all. TSRs that need
file access run with explicit `Fopen`/`Fclose` cycles inside their
ISR / hooks.

### 4. `xsetblk` failure modes

If `blkln` < 256 (less than `sizeof(PD)`), `xsetblk` rejects it but
the return is ignored. Result: nothing shrinks, the entire original
TPA stays "reserved" — wasteful but not crashing.

If `blkln` > original block: behaviour is implementation-defined;
EmuTOS rejects (return value ignored, original size preserved).

### 5. Environment block

The `p_env` block is **also** owned by `run` and therefore reserved
by `reserve_blocks`. This is usually undesired (the environment
becomes orphaned). Conscientious TSRs explicitly `Mfree(p_env)`
before calling `Ptermres`. EmuTOS does not auto-free the env.

### 6. Multiple TSRs

Each `Ptermres` permanently reserves its block. The TPA fragments
from the bottom up. There is no in-OS mechanism to "uninstall" a
TSR — the orphaned MDs cannot be located by ownership. Restarts /
warm boot are the only general way to reclaim.

### 7. Return type quirk

The C signature returns `WORD` even though the function never
returns. The trap dispatcher table still classifies it via
`stdio_typ` accordingly; calling code ignores it.

## Dependencies

- `xsetblk` — shrink the MD
- `reserve_blocks` (file-local helper) — strip ownership
- `xmfremd` — free the MD struct
- `xterm` — full termination chain (transitively: user-term
  handler, file close, curdir release, parent restore)

Used by: TSR programs, keyboard accessories before the AES is
loaded, low-level disk caches.

## Cross-references

- **Compendium:** "Ptermres" — agrees, including the orphan-MD
  mechanism.
- **Profibuch:** "Ptermres (0x31)" — agrees, with worked TSR
  example.
- **`docs/api/gemdos/process/01-pterm.md`** — semantics inherited.
- **`docs/api/structures/md.md`** — MD list manipulation.
- **`docs/api/gemdos/memory/Mshrink.md`** (TBD) — same `xsetblk`
  helper.

## Configuration

- `CONF_WITH_ALT_RAM` — second pool also reserved.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06 / PHASE-03 implementation note: the "orphan MD" trick
requires the memory allocator to support MDs that live outside any
list yet whose memory is not on the free list. NewTOS's allocator
must accommodate this, or replace it with an explicit "system-owned"
sentinel ownership.
