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

## 2. System File Table (SFT) and per-process handle table

Handles come in two classes, with **two separate tables**:

- **Per-process standard handles 0..5** live in `PD.p_uft[NUMSTD]`
  (`include/bdosdefs.h:84`), a 6-entry array of **signed bytes** (`SBYTE`).
  Each entry is either a **negative BIOS character-device handle** (`H_Console
  = -1`, `H_Aux = -2`, `H_Print = -3`; `bdos/console.h:23-25`) or, once
  redirected by `Fforce`/`Fdup`, the **non-standard handle number** (`>= 6`)
  it now maps to.
- **Global non-standard handles 6..80** are indexed into the **system file
  table** `sft[OPNFILES]` (`FTAB`, `bdos/fs.h:307-312`; declared
  `bdos/fsglob.c`). Each `FTAB` holds `f_ofd` (OFD pointer, or a negative BIOS
  handle), `f_own` (owning PD), and `f_use` (reference count). Handle `h`
  addresses `sft[h - NUMSTD]`.

Sizes (`bdos/fs.h:43-45`, `include/bdosdefs.h:61`):

```
NUMSTD     = 6      /* standard handles 0..5              */
OPNFILES   = 75     /* system-wide open non-standard files */
NUMHANDLES = 81     /* NUMSTD + OPNFILES                   */
```

(Correction vs. earlier draft: `OPNFILES` is **75**, not 32/34; `NUMHANDLES`
is **81**, not 40. The 44-byte-DTA / `p_uft` "points into syshnd" wording was
wrong — `p_uft` holds either a negative BIOS handle or a non-standard handle
number, not an index into a `syshnd[]` array.)

### Default standard-handle wiring (`default_handle[]`, `bdos/console.c:60-68`)

Set at boot and restored by `Fclose` of a standard handle
(`get_default_handle`, `bdos/console.c:132`):

| Handle | `p_uft[h]` default | Device |
|-------:|--------------------|--------|
| 0 (stdin)  | `H_Console` = -1 | `CON:` (BIOS console) |
| 1 (stdout) | `H_Console` = -1 | `CON:` |
| 2 (stdaux) | `H_Aux` = -2     | `AUX:` (serial) |
| 3 (stdprn) | `H_Print` = -3   | `PRN:` (printer) |
| 4          | `H_Console` = -1 | `CON:` (TOS redirects undefined handles) |
| 5          | `H_Console` = -1 | `CON:` |

`getofd(h)` (`bdos/fsmain.c:474`) resolves any handle to an OFD via `syshnd`
(`bdos/fsmain.c:457`): for a standard handle it first follows `p_uft[h]`; a
value `> 0` becomes `sft[value - NUMSTD]`, a value `<= 0` stays a BIOS handle
(no OFD → NULL). Out-of-range handles → NULL.

`Fopen`/`Fcreate` allocate a free `sft[]` slot via `opnfil` (`f_own = run`,
`f_use = 1`); `Fclose` releases it via `xclose`→`sftdel` at `f_use == 0`.
`Fdup` claims a new slot copying the mapping (no source `f_use` bump);
`Fforce` rewires a standard slot and bumps `f_use` for file mappings. See the
per-function specs under `docs/api/gemdos/file/`.

NewTOS: per-process file table in the kernel; standard handles 0..5
short-circuit to the console/serial/printer layer without touching the SFT.

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

Stored **per process** in `PD.p_curdir[NUMCURDIR]` (one slot per drive; each
entry is an index into a system directory table, `include/bdosdefs.h:89`).
(This is the current-directory state; the DTA pointer is `p_xdta`, §6 — the two
are unrelated. An earlier draft mislabelled the path storage as `p_dta`.)
The drive/directory sub-group specs own this section; noted here only to
correct the field name.

`Dsetpath` (0x3B) updates `PD->p_curdir[curdrv]`; `Dgetpath` (0x47)
reads it. Path strings are stored *without* drive letter and *with*
leading backslash.

---

## 6. DTA Pointer (`PD.p_xdta`)

The Disk Transfer Area is a 44-byte buffer (`DTA`, `include/bdosdefs.h:47-55`)
where `Fsfirst`/`Fsnext` write directory entries. Each PD holds the active DTA
pointer in **`p_xdta`** (`DTA *`, basepage offset **0x20**,
`include/bdosdefs.h:79`). `Fsetdta` (0x1A) sets it (`run->p_xdta = addr`,
`bdos/fsmain.c:405`, no validation); `Fgetdta` (0x2F) returns it
(`bdos/fsmain.c:396`).

**Default:** on process creation `Pexec` sets `p_xdta = (DTA *)p->p_cmdlin`
(`bdos/proc.c:375`) — the basepage command-line buffer at offset **0x80**.
A program that calls `Fsfirst` before any `Fsetdta` therefore overwrites its
own command line (the classic Atari quirk; contract for m68k binaries).

**Layout of the 44-byte DTA** (`bdos/fs.h:319-337` `DTAINFO` private view /
`include/bdosdefs.h:47-55` public view):

| Offset | Public field | Private (`DTAINFO`) | Meaning |
|-------:|--------------|---------------------|---------|
| 0x00-0x0B | `d_reserved[0..11]` | `dt_name[12]` | search pattern (11 FCB chars + attr) |
| 0x0C-0x0F | `d_reserved[12..15]` | `dt_offset_drive` (LONG) | `-1` = uninitialised; else drive id (low 5 bits) + root resume offset |
| 0x10-0x11 | `d_reserved[16..17]` | `dt_cloffset` (UWORD) | sub-dir resume offset within cluster |
| 0x12-0x13 | `d_reserved[18..19]` | `dt_clnum` (UWORD) | sub-dir current cluster |
| 0x14   | `d_reserved[20]` | `dt_attr` (char) | search attribute mask |
| 0x15   | `d_attrib`   | `dt_fattr`  | attribute byte of the found entry |
| 0x16   | `d_time`     | `dt_td.time`| packed time — **native (m68k big-endian)** byte order |
| 0x18   | `d_date`     | `dt_td.date`| packed date — native byte order |
| 0x1A   | `d_length`   | `dt_fileln` | file size, LONG, native byte order |
| 0x1E   | `d_fname[14]`| `dt_fname[14]` | dotted 8.3 name, upper-case, NUL-terminated |

Bytes 0x00-0x14 (21 bytes) are the **private** search state programs must
treat as opaque between the matching `Fsfirst` and the final `Fsnext`. The
time/date/size are byte-swapped from the little-endian on-disk FCB to native
order by `makbuf` (`bdos/fsdir.c:1851-1862`) — NewTOS must reproduce the m68k
big-endian values for the DTA's public fields (contract), while its private
encoding is a free choice as long as `Fsnext` resumes correctly and returns
`ENMFIL` (-49) when exhausted/uninitialised.

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
