# `Dgetpath` — GEMDOS 0x47

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:1229` (`xgetdir`).

## EmuTOS source

```c
long xgetdir(char *buf, int drv)
{
    drv = drv ? drv-1 : run->p_curdrv;

    if (!(Drvmap() & (1L<<drv)) || (ckdrv(drv, FALSE) < 0)) {
        *buf = 0;
        return EDRIVE;
    }

    int n = run->p_curdir[drv];
    DND *p = dirtbl[n].dnd;
    int len = LEN_ZPATH - 3;       /* leave room for "X:\" prefix */

    buf = dopath(p, buf, &len);     /* writes path components */
    *--buf = 0;                     /* null-terminate (overwrite trailing /) */
    return E_OK;
}
```

`dopath` walks the DND chain from `p` up to the root, writing each
component (with `\` separator) into `buf`. The final separator is
overwritten with NUL.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0047` |
| WORD args | 3 (LONG + WORD) |
| Stack frame | `+6: drv (W)`, `+2: buf (L)`, `+0: 0x0047` |

## Inputs

| Pos | Name | Type    | Range / meaning                                |
|-----|------|---------|------------------------------------------------|
| 0   | `buf`| char *  | output buffer (≥ `LEN_ZPATH` = 128 bytes)       |
| 1   | `drv`| WORD    | 0 = current drive, 1 = A:, 2 = B:, …             |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, EDRIVE on bad drive |

`buf` filled with absolute path **without** drive letter, e.g.
`\AUTO\TOOLS` (note: leading backslash, no `C:` prefix).

For root directory, `buf` becomes empty string `""`.

## Errors

| Code    | Value | When                                              |
|---------|------:|---------------------------------------------------|
| EDRIVE  | -46   | drive not in `Drvmap()` or `ckdrv` failed         |

`buf` is set to empty string on error.

## Edge cases

### 1. Drive `+1` convention

Same as `Dfree`: 0 = current, 1..16 = A:..P:.

### 2. No drive letter in output

The returned path starts with `\` and lists components separated
by `\`. The drive letter is implicit (the caller knows which
drive it asked about). This is a deliberate TOS choice — to get
a fully qualified path, programs prepend `"X:"` themselves.

### 3. Root → empty string

If the curdir for `drv` is root, `dopath` writes nothing and the
NUL goes at offset 0. Programs displaying the path must
distinguish "" (root) from non-empty paths.

### 4. NO mediach check

`ckdrv(drv, FALSE)` does NOT trigger media-change detection
(unlike `Dfree`'s `TRUE`). This matches Atari TOS behaviour and is
called out in the source comment: programs polling `Dgetpath` for
existence checks won't accidentally remount.

### 5. Buffer size — caller allocates `LEN_ZPATH` (128)

The 124-byte cap (`LEN_ZPATH - 3`) leaves margin for the X:\
prefix. If `dopath` would exceed this, it truncates the path
silently — output may be invalid. Well-behaved code keeps paths
short.

### 6. Per-process result

`run->p_curdir[drv]` is per-PD. Each process sees its own current
directory.

### 7. Drive offline → EDRIVE + empty buf

The empty-string write is deliberate: programs reading
`Dgetpath`-then-print see `""` instead of garbage when the drive
is missing.

### 8. Concurrent access

DNDs walked by `dopath` are global. If another process deletes a
folder mid-walk (unlikely on single-thread TOS, possible on
NewTOS), the walk may see stale data. NewTOS plans a per-DND
read-lock during traversal.

## Dependencies

- `Drvmap` (BIOS 0x0A), `ckdrv`, `dopath`.
- `dirtbl[]`, `run->p_curdir[drv]`.
- DND chain (parent links).

Used by: shells displaying prompt, AES title bar, programs that
need the cwd for relative-path resolution.

## Cross-references

- **Compendium:** "Dgetpath" — agrees.
- **Profibuch:** "Dgetpath (0x47)" — agrees.
- `docs/api/gemdos/directory/3b-dsetpath.md`,
  `docs/api/gemdos/drive/19-dgetdrv.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: identical output format. `LEN_ZPATH = 128` ABI-fixed.
NewTOS may add an XBIOS extension that returns the full
`X:\path` form; the classic syscall keeps the no-letter format.
