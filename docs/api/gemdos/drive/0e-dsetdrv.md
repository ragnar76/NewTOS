# `Dsetdrv` — GEMDOS 0x0E

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsmain.c:420` (`xsetdrv`).

## EmuTOS source

```c
long xsetdrv(int drv)
{
    if ((drv >= 0) && (drv < BLKDEVNUM))
        run->p_curdrv = drv;
    return Drvmap();
}
```

`BLKDEVNUM = 16` — drives A:..P:. `Drvmap()` is BIOS function
0x0A and returns the bitmap of all online drives.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x000E` |
| WORD args | 1 (WORD) |
| Stack frame | `+2: drv (WORD)`, `+0: 0x000E` |

## Inputs

| Pos | Name  | Type | Range / meaning                              |
|-----|-------|------|----------------------------------------------|
| 0   | `drv` | WORD | drive index (0=A:, 1=B:, …, 15=P:)            |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (drive bitmap) |

Bit `n` set ⇒ drive `n` exists. Returned **regardless of whether
the change succeeded** — `xsetdrv` always calls `Drvmap()` at the
end.

## Errors

No error code. Out-of-range `drv` is silently ignored (current
drive unchanged); the caller cannot tell the call failed except by
calling `Dgetdrv` afterwards.

## Edge cases

### 1. Range silently clamped

`drv < 0` or `drv >= 16`: `p_curdrv` is **not** updated. The
returned drive bitmap may make the caller think the call worked.
This is the historical Atari behaviour and is deliberately preserved.

### 2. No existence check

`xsetdrv` does **not** verify that bit `drv` is set in the drive
map. A program can `Dsetdrv(5)` with drive F: missing, succeed,
then any subsequent file operation on the current drive returns
`EDRIVE` (-46). This is by design — see comment in source about
BetaDOS expecting `Dsetdrv(15)` (P:) to work even when P: is offline.

### 3. Per-process current drive

`p_curdrv` lives in the calling process's PD. Each process has its
own current drive; `Pexec` inherits the parent's at child creation,
then they diverge.

### 4. Return value source

The Drvmap result reflects **all** present drives, not just the
ones reachable from this process. EmuTOS has no per-process drive
mapping or chrooting.

### 5. Boot-time default

PD's `p_curdrv` is initialised by `Pexec` to the parent's value.
The first user process inherits from the BIOS bootstrap (typically
the boot drive — drive C: for hard-disk boots, A: for floppy).

## Dependencies

- `run` (current PD pointer).
- BIOS `Drvmap` (fn 0x0A).

Used by: shell `CD A:`, AES file selector, every chdir-equivalent.

## Cross-references

- **Compendium:** "Dsetdrv" — agrees on bitmap return.
- **Profibuch:** "Dsetdrv (0x0E)" — agrees.
- `docs/api/gemdos/drive/19-dgetdrv.md`
- `docs/api/bios/0a-drvmap.md` (pending)

## Configuration

- `BLKDEVNUM = 16` (`bdos/fs.h`); not user-tunable.

## NewTOS implementation notes

(Empty during Phase 00.)
