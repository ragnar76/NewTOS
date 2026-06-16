# `Dsetpath` — GEMDOS 0x3B

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:1113` (`xchdir`).

## EmuTOS source

```c
long xchdir(char *p)
{
    if (contains_wildcard_characters(p))
        return EPTHNF;

    int dlog = extract_drive_number(p);
    if (dlog < 0) dlog = run->p_curdrv;

    int olddir = run->p_curdir[dlog];

    DND *dnd = findit(p, &s, 1);            /* mode 1: must be folder */
    if (!dnd) return EPTHNF;

    int newdir = incr_curdir_usage(dnd);
    if (newdir < 0) return EPTHNF;          /* dirtbl full */
    run->p_curdir[dlog] = newdir;

    if (olddir) decr_curdir_usage(olddir);

    return E_OK;
}
```

`dirtbl[]` is a global table of cached DND pointers, one entry per
"current directory" reference, with reference counts.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x003B` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: p (L)`, `+0: 0x003B` |

## Inputs

| Pos | Name | Type   | Range / meaning                     |
|-----|------|--------|-------------------------------------|
| 0   | `p`  | char * | absolute or relative path to a folder |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

## Errors

| Code   | Value | When                                                  |
|--------|------:|-------------------------------------------------------|
| EPTHNF | -34   | path contains wildcards; component missing; not a folder; `dirtbl[]` full |

## Edge cases

### 1. Wildcards rejected

`*` or `?` in the path returns EPTHNF immediately, without trying
to resolve. Programs can't `Dsetpath("*.dir")`.

### 2. Drive component changes target drive

`extract_drive_number("C:\foo")` returns 2 (C:); `dlog` becomes
the per-drive current-directory slot for C:. The current drive
(`p_curdrv`) is **not** changed — only the per-drive curdir for C:.

To change drive AND curdir, use `Dsetdrv` then `Dsetpath`.

### 3. Per-process per-drive curdir

`p_curdir[16]` — one slot per possible drive A:..P:. Each slot is
an index into the global `dirtbl[]`. Each process has its own
table.

### 4. `dirtbl[]` overflow

`NCURDIR` is the size of `dirtbl`. When all slots are used (many
processes / many drives with non-root cwd), `incr_curdir_usage`
returns `EINTRN` which `xchdir` translates to EPTHNF. Real-world:
unusual; AES cleans up on `Pterm`.

### 5. Reference-counted DNDs

`incr_curdir_usage` either finds an existing slot pointing at the
same DND (bumps refcount) or allocates a new one. `decr_curdir_usage`
on the old slot drops refcount; reaching 0 frees the slot.

### 6. Relative path

Without a drive prefix, `p` is resolved relative to the current
drive's curdir. `Dsetpath("..")` walks up one level.

### 7. `findit` mode 1

Mode 1 means "result must be a directory". A path resolving to a
file returns EPTHNF (not EFILNF — historical TOS quirk).

### 8. Inherited by Pexec children

`Pexec` copies `p_curdir[]` from parent to child. Child's
`Dsetpath` calls only affect the child.

### 9. Atomicity

The sequence "incr new, decr old" — if `decr` ran first and freed
the slot before `findit` succeeded, a concurrent process could
reuse the slot. EmuTOS is single-threaded so this can't happen;
NewTOS-on-RPi multitasking will need locking.

## Dependencies

- `findit` (mode 1), `extract_drive_number`,
  `contains_wildcard_characters`.
- `dirtbl[]`, `run->p_curdir[]`.
- `incr_curdir_usage`, `decr_curdir_usage`.

Used by: shell `cd` command, AES desktop, every program that
touches the cwd.

## Cross-references

- **Compendium:** "Dsetpath" — agrees.
- **Profibuch:** "Dsetpath (0x3B)" — agrees.
- `docs/api/gemdos/directory/47-dgetpath.md`,
  `docs/api/gemdos/drive/0e-dsetdrv.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: per-process curdir model preserved. `dirtbl[]` size
extended (parameterised) since RPi memory is plentiful and AES-on-
RPi may host more processes than original TOS.
