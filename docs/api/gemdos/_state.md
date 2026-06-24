# GEMDOS Subsystem State

> Phase 00 deliverable D7 (GEMDOS). Documents the BDOS-internal state
> referenced by GEMDOS spec entries. Source: EmuTOS @ `1a7484ab`.

This file captures the **shared mutable state** of the BDOS layer.
Per-function specs reference these tables by name; this file is the
single description of their structure, lifecycle, and concurrency
expectations.

> **Observable vs. internal** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> This document is mostly EmuTOS's **internal model**, captured for
> reference — NewTOS keeps the *behaviour*, not these structures.
> **Free to reimplement** (a program cannot observe them): the SFT internals,
> the disk-buffer cache `bufl[]`, `drvtbl[]`/`DMD`, the memory pools
> `pmd`/`pmdalt` layout, the concurrency mechanics. **Contract** (a program
> *does* observe these — keep value/semantics exact): the `PD`/basepage
> fields, the `conterm` bits, the `drvbits` drive map, the per-process DTA
> pointer + the 44-byte DTA the program owns, the OS-header/cookie-jar
> values. When in doubt, apply the audit rubric in COMPATIBILITY.md.

---

## 1. Process descriptor `run` (`bdos/proc.c:54`)

```c
PD *run;        /* current PD (process descriptor) */
```

Pointer to the currently-running process. Set by `xexec` /
`proc_go`, read by virtually every BDOS call that needs the calling
process's path, DTA, FCB table, or memory descriptor.

Initialised at boot to the descriptor of the bootstrap process
(EmuTOS shell or AUTO program). Updated by `Pexec(0)` and
`Pterm`/`Pterm0`/`Ptermres`.

NewTOS: same pointer, mapped to the kernel's `Process` struct.

---

## 2. System File Table (SFT)

The SFT maps file handles (≥ 6) to OFD (Open File Descriptor)
structures. EmuTOS embeds it in each PD:

```c
struct {
    UBYTE p_uft[NUMSTD];        /* standard handles 0..5, point into syshnd */
    OFD * p_xfh[OPNFILES-NUMSTD];/* extended handles 6..OPNFILES-1 */
} pd;
```

with `NUMSTD = 6`, `OPNFILES = 32` (configurable). Standard handles
0..5 are mapped to BIOS character devices via the `syshnd[]` array
(`bdos/fsglob.c:39`), extended handles 6..31 hold OFD pointers.

| Handle | Mapping                              |
|-------:|--------------------------------------|
| 0      | `CON:` — `Bconin/Bconout(2)`          |
| 1      | `CON:` — same                         |
| 2      | `AUX:` — `Bconin/Bconout(1)`          |
| 3      | `PRN:` — `Bconout(0)`, `Bcostat(0)`   |
| 4..5   | reserved (TOS extension)              |
| 6..31  | OFD pointer (file or pipe)            |

`Fopen` allocates a free slot via `opnfil`, `Fclose` releases it via
`xclose`. `Fdup` / `Fforce` rewire entries.

NewTOS: per-process file table in the kernel; standard handles
intercepted by the syscall layer (handles 0..5 short-circuit to
console without touching SFT).

---

## 3. Disk Buffer Cache `bufl[]` (`bdos/fsbuf.c`)

Two singly-linked lists of cached disk sectors:

```c
BCB *bufl[2];   /* [0] = FAT/dir buffers, [1] = data buffers */
```

Each `BCB` (`bdos/fs.h`) holds `b_bufdrv`, `b_bufrec` (lba),
`b_bufr` (pointer to actual sector buffer), `b_dirty`, `b_link`.
The cache is small (4 buffers per list by default) and uses LRU via
list-head insertion.

Critical invariants:
- `b_dirty == 1` requires writeback before eviction.
- `flush(d)` writes all dirty buffers for drive `d`.
- `flush()` (no arg) writes all dirty buffers across all drives.

NewTOS: replace with a write-back page cache in the FAT driver;
preserve `flush` semantics for `Dskwrite/Dskread` ABI.

---

## 4. Drive Descriptors

### `drvtbl[]` and `drvbits` (`bdos/fsdrive.c:42`, `bios/blkdev.c`)

```c
extern LONG drvbits;        /* bitmap of mounted drives, A=0..P=15 */
DMD *drvtbl[BLKDEVNUM];     /* one per drive, NULL if not mounted */
```

`drvbits` is the BIOS's `Drvmap()` return value. `drvtbl[d]` points
to a Drive Media Descriptor for drive `d` once it has been accessed.

### `DMD` (Drive Media Descriptor, `bdos/fs.h`)

Holds: BPB-derived geometry, FAT cache pointers, root-directory DND,
free-list bitmap state.

Created lazily by `getdmd()` on first reference to a drive.

### `curdrv` (`bdos/fsdrive.c:45`)

```c
WORD curdrv;    /* current default drive, 0=A..15=P */
```

Read by `xgetdrv` (Dgetdrv 0x19), written by `xsetdrv` (Dsetdrv
0x0E).

---

## 5. Current Path per Drive

Stored **per process** in `PD->p_dta` and the path strings in
`PD->p_curdir[NUMCURDIR]` (one slot per drive, see `bdos/proc.c`).
EmuTOS uses fixed-length 64-byte path strings rooted at the drive
letter.

`Dsetpath` (0x3B) updates `PD->p_curdir[curdrv]`; `Dgetpath` (0x47)
reads it. Path strings are stored *without* drive letter and *with*
leading backslash.

---

## 6. DTA Pointer (`PD->p_dta`)

The Disk Transfer Area is a 44-byte buffer (`include/dta.h:DTA`)
where `Fsfirst`/`Fsnext` write directory entries. Each PD has a
`p_dta` slot, defaulting to `&PD->p_dtabuf[0]`. `Fsetdta` (0x1A)
points it elsewhere; `Fgetdta` (0x2F) returns the current pointer.

Concurrency: `Fsfirst` writes 44 bytes, then `Fsnext` reads/updates
search state stored *inside* the DTA (offsets 0..21 are private
search state, 22..43 are the user-visible `d_attrib`/`d_time`/
`d_date`/`d_length`/`d_fname[14]`).

---

## 7. `conterm` (`bdos/console.c`)

```c
extern UBYTE conterm;   /* low byte of OS var conterm @ 0x484 */
```

Bit fields:

| Bit | Meaning                                               |
|----:|-------------------------------------------------------|
| 0   | `Cconin` echoes input                                  |
| 1   | enable Ctrl-S / Ctrl-Q flow control                    |
| 2   | enable Ctrl-C abort (raises terminate via Pterm)       |
| 3   | reserved                                               |
| 4   | reserved                                               |
| 5   | reserved                                               |
| 6   | reserved                                               |
| 7   | reserved                                               |

Read by `xconin`, `xnecin`, `xconrs`. Written by AES on init and by
test programs.

---

## 8. Memory Pool `pmd` and `pmdalt` (`bdos/umem.c:38..55`)

```c
MPB pmd;        /* main ST-RAM pool */
#if CONF_WITH_ALT_RAM
MPB pmdalt;     /* alt-RAM pool (TT/Falcon/expansion) */
#endif
```

Each `MPB` has `mp_mfl` (free list) and `mp_mal` (allocated list)
of `MD` records. `Malloc`/`Mfree`/`Mxalloc`/`Mshrink` operate on
these lists. `Maddalt` adds a region to `pmdalt`.

NewTOS: implement two pools; ST-RAM is the lower 1 MiB of the 1 GiB
mapped region (Phase 03), Alt-RAM is the rest.

---

## 9. OS Header `os_header` (`bdos/fsmain.c`, `bios/tosvars.S`)

The TOS OS header at low memory address `0x4F2` (`_sysbase`)
contains `os_version`, `os_beg`, `os_end`, `os_magic`, `os_date`,
`os_palmode`, `os_dosdate`, plus pointers to drive table, run-time
flags, etc.

Read by `Sversion` (0x30) and the AES/CPX init code.

---

## 10. Concurrency Model

EmuTOS is single-threaded (cooperative): only one BDOS call active
at a time. The state above is therefore not protected by locks.

NewTOS: must serialise BDOS entry per-CPU using a per-process or
global BDOS mutex (decision: ADR-pending, before PHASE-05). Disk
buffer cache and SFT need explicit locks because the kernel is
SMP-aware.

---

## Cross-references

- BIOS state — `docs/api/bios/_state.md`.
- XBIOS state — `docs/api/xbios/_state.md`.
- Structures (PD, OFD, DND, DMD, MD/MPB, DTA) — `docs/api/structures/`.
- Dependency graph — `docs/api/_dependency-graph.md`.
