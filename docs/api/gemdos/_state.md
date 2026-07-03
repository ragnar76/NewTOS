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

## 4. Drive Descriptors and the current-drive model

### Drive count: `BLKDEVNUM = 26` (A:..Z:)

`BLKDEVNUM = 26` (`include/sysconf.h:43`), so drives are **A:..Z:**, not
A:..P:. `drvbits`/`Drvmap()` return a 32-bit bitmap with bits 0..25 usable
(`A:` = bit 0 … `Z:` = bit 25). (Correction vs. earlier draft, which used 16 /
A:..P: throughout — see `_discrepancies.md` D-025.)

### `drvtbl[]`, `drvbits`, `drvsel`, `drvrem`

```c
DMD *drvtbl[BLKDEVNUM];     /* bdos/fsglob.c:28 — one DMD per drive, NULL if not logged in */
LONG drvsel;               /* bdos/fsdrive.c:59 — mask of drives selected since power-up */
```

- **`drvbits`** — the BIOS's `Drvmap()` return value (BIOS layer,
  `bios/blkdev.c:833`, `drvbits`). Bitmap of **present** block devices. Read by
  `xsetdrv`/`xgetdir` via `Drvmap()`.
- **`drvrem`** — BIOS bitmap of **removable** drives (`include/biosext.h:36`);
  read by `ckdrv` to decide whether to run a media-change check.
- **`drvsel`** (BDOS) — drives logged in since power-up; set the first time a
  drive is accessed. Read/written by `ckdrv`.
- **`drvtbl[d]`** — points to a Drive Media Descriptor for drive `d` once it has
  been logged in; created lazily by `getdmd()`/`log_media()` on first reference.

### `DMD` (Drive Media Block, `bdos/fs.h:240-266`)

Holds BPB-derived geometry, FAT cache pointers, and the root-directory DND.
Fields read by the directory/drive group:
`m_numcl` (`CLNO`, total data clusters), `m_recsiz` (`UWORD`, bytes/sector),
`m_clsiz` (`WORD`, sectors/cluster), `m_16` (16-bit-FAT flag). These four (three
plus a computed free count) are exactly what `Dfree` returns — see §11.

Created lazily by `getdmd()` (`bdos/fsdrive.c:140`) on first reference to a drive.

### Current drive: `PD.p_curdrv` (per-process) — there is **no** global `curdrv`

The current default drive is **per-process**, stored in `PD.p_curdrv`
(`UBYTE`, basepage offset **0x37**, `include/bdosdefs.h:86`):

- Read by `xgetdrv` (Dgetdrv 0x19, `bdos/fsmain.c:440`) and by `xgetfree`/
  `xgetdir`/`dcrack` when the drive argument is 0 ("current").
- Written by `xsetdrv` (Dsetdrv 0x0E, `bdos/fsmain.c:423`) — only when the new
  value is in `0..BLKDEVNUM-1`; otherwise unchanged (silent ignore).
- Inherited by children at `Pexec` and thereafter diverges per process.

(Correction vs. earlier draft: there is **no** global `WORD curdrv` variable in
EmuTOS. The current drive lives only in `run->p_curdrv`. See `_discrepancies.md`
D-025.)

**Drive-number conventions differ across this group** (a frequent off-by-one
source): `Dsetdrv`/`Dgetdrv` are **0-based** (`0 = A:`); `Dfree`/`Dgetpath` use
the **`+1`** convention (`0 = current`, `1 = A:`).

---

## 5. Current Path per Drive

The current directory is **per-process, per-drive**, stored in
`PD.p_curdir[NUMCURDIR]` (`UBYTE[26]`, basepage offset **0x40**,
`include/bdosdefs.h:89`; `NUMCURDIR = BLKDEVNUM = 26`, `include/sysconf.h:62`).
Each entry is an **index into the global `dirtbl[]`**, not a path string. Index
`0` means "not set yet" (treated as the drive root). (This is the
current-directory state; the DTA pointer is `p_xdta`, §6 — the two are
unrelated. An earlier draft mislabelled the path storage as `p_dta` and used 16
slots; it is 26 — see D-025.)

### `dirtbl[]` — reference-counted DND cache (`bdos/fsdrive.c:54`)

```c
DIRTBL_ENTRY dirtbl[NCURDIR];   /* bdos/fs.h:342-345, NCURDIR = 40 (bdos/fs.h:44) */
/* DIRTBL_ENTRY { DND *dnd; WORD use; } */
```

`dirtbl[]` maps a small integer (the value stored in `p_curdir[]`) to a cached
directory `DND` plus a use count. Slot 0 is reserved. `Dsetpath` (via
`incr_curdir_usage`, `bdos/fsdir.c:1164`) finds an existing slot for the target
DND or allocates a free one and bumps its use count; the old slot is released by
`decr_curdir_usage` (`bdos/fsdir.c:1194`). Multiple processes/drives referencing
the same directory share one slot. When `dirtbl[]` is full,
`incr_curdir_usage` returns `EINTRN`, which `Dsetpath` surfaces to the caller as
`EPTHNF`.

`Dsetpath` (0x3B) selects the drive from an optional `X:` prefix
(`extract_drive_number`, 0..25) or the current drive, then updates
`p_curdir[dlog]`; it does **not** change `p_curdrv`. `Dgetpath` (0x47) reads
`dirtbl[p_curdir[drv]].dnd` and walks the DND parent chain (`dopath`) to build
the path string, *without* drive letter and *with* a leading backslash (root =
empty string). `NCURDIR` (40) is distinct from the drive count (26); NewTOS may
enlarge `dirtbl[]` as a free implementation choice.

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

## 11. `Dfree` info buffer (4 LONGs / 16 bytes)

`Dfree` (0x36, `xgetfree`, `bdos/fsfat.c:337`) fills a caller-supplied 16-byte
buffer with four LONGs, copied from the drive's `DMD` (§4) plus a computed free
count (`bdos/fsfat.c:361-364`):

| Index | Field           | DMD source | Type | Meaning |
|------:|-----------------|------------|------|---------|
| `buf[0]` | free clusters   | computed (`countfree16` / `getrealcl` loop) | LONG | unallocated data clusters |
| `buf[1]` | total clusters  | `m_numcl` (`CLNO`) | LONG | total data clusters on the volume |
| `buf[2]` | bytes/sector    | `m_recsiz` (`UWORD`) | LONG | bytes per logical record (sector) |
| `buf[3]` | sectors/cluster | `m_clsiz` (`WORD`) | LONG | logical records per cluster |

Each field is written as a full 32-bit LONG (native byte order — big-endian for
m68k callers). Derived: bytes/cluster = `buf[2]*buf[3]`; free bytes =
`buf[0]*buf[2]*buf[3]`; total bytes = `buf[1]*buf[2]*buf[3]`. The buffer is
**contract** (fixed layout a program reads); the DMD internals it derives from
are **free** to reimplement. See `docs/api/gemdos/directory/36-dfree.md`.

---

## Cross-references

- BIOS state — `docs/api/bios/_state.md`.
- XBIOS state — `docs/api/xbios/_state.md`.
- Structures (PD, OFD, DND, DMD, MD/MPB, DTA) — `docs/api/structures/`.
- Dependency graph — `docs/api/_dependency-graph.md`.
