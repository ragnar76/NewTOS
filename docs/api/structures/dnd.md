# `DND` / `DMD` — Directory Node Descriptor and Drive Media Block

> Phase 00 spec D6. Source: EmuTOS @ `1a7484ab` —
> `bdos/fs.h:206-260`. Used by `bdos/fsdir.c`, `bdos/fsfat.c`,
> `bdos/fsbuf.c`.

## Two structures, one filesystem cache

- **`DMD`** — per-mounted-drive metadata. Pre-computed FAT geometry
  (cluster size logs/masks), pointers to the open-file list, and the
  root of the DND tree.
- **`DND`** — per-cached-directory state. EmuTOS keeps an in-memory
  tree of recently visited directories per drive; each node holds
  enough state to resume scanning without re-reading the parent.

## `DND` — Directory Node Descriptor (≤ 64 bytes — architectural limit)

```c
struct _dnd {
/* 0x00 */ char    d_name[11];     /* FCB-style 8.3 name */
/* 0x0B */ UBYTE   d_fill;         /* attributes */
/* 0x0C */ UWORD   d_flag;         /* DND_LOCKED + reserved */
/* 0x0E */ CLNO    d_strtcl;       /* starting cluster of directory */

/* 0x10 */ DOSTIME d_td;           /* time/date (little-endian) */
/* 0x14 */ OFD    *d_ofd;          /* OFD for this dir (NULL if not open) */
/* 0x18 */ DND    *d_parent;       /* parent dir (..)               */
/* 0x1C */ DND    *d_left;         /* first child                   */

/* 0x20 */ DND    *d_right;        /* sibling DND in same parent    */
/* 0x24 */ DMD    *d_drv;          /* owning drive's media block    */
/* 0x28 */ OFD    *d_dirfil;       /* OFD used to scan this dir     */
/* 0x2C */ LONG    d_dirpos;       /* byte position within parent's dir */

/* 0x30 */ LONG    d_scan;         /* current scan position for tree walk */
/* 0x34 */ OFD    *d_files;        /* list of open files in this dir */
/* 0x38 — 56 bytes; fits in 64-byte budget */
};
```

### `d_flag`

```c
#define DND_LOCKED  0x8000   /* may not be scavenged */
```

`DND_LOCKED` is set on the root DND of every drive and on any DND
referenced by an open OFD. Other DNDs are eligible for eviction by
`free_available_dnds()` (`bdos/fsdir.c`) when the DND pool runs low.

### Tree shape

For each drive, DNDs form a tree rooted at `DMD.m_dtl`:

```
        root (drive A)
        /     |       \
     parent ─ left ── right ── ...   (children list of root)
        |
       grandchild ...
```

The `d_left`/`d_right` pair is **first-child / next-sibling** rather
than left/right binary tree. `d_parent` points up to the parent
directory's DND.

### `d_dirfil` and `d_dirpos`

When iterating the directory (e.g., during `Fsfirst`/`Fsnext` or
path resolution), `d_dirfil` holds the OFD used to read the
directory data, and `d_dirpos` is the byte offset within the
directory of the next entry to read.

**Correction:** `d_scan` is **not** a separate scan position used by tree
walks or unmount flushing — that description was fabricated. A full-tree
grep of `bdos/*.c` finds exactly one reference: `makdnd()` zeroes it
(`p1->d_scan = 0L;`, `bdos/fsdir.c:1644`) when recycling a scavenged DND; it
is **never read anywhere**. The 1986 changelog at the top of `fsdir.c` even
says so directly: "removed all references to d_scan" (`bdos/fsdir.c:55,60`).
The field is dead/vestigial, kept only because the struct's on-disk-adjacent
layout is pinned for `FOLDRnnn.PRG` compatibility (see "Lifetime" below) —
there is no "flush modified DNDs at unmount" mechanism in this codebase at
all (EmuTOS has no directory-tree unmount concept; see the corrected
`d_files`/`m_ofl` note below).

**Correction:** `d_fill` (offset 0x0B) is likewise **never referenced**
anywhere in `bdos/*.c` — its own source comment already hedges with a
question mark (`/* attributes? */`, `bdos/fs.h:209`). Treat it as reserved
padding, not a real attributes field.

### Lifetime

- Root DND created at mount time (`getdmd()`/`log_media()`,
  `bdos/fsdrive.c:140-158,184-249`) and never freed by ordinary operation.
- Non-root DNDs lazily allocated (`makdnd()`, `bdos/fsdir.c:1610`) when path
  resolution descends into a new directory.
- **Correction:** there is no `NDIRS` constant and no per-type DND pool.
  DND/DMD/OFD structs are all carved from the **same shared static
  `osmem[]` block pool** used for `MD`s (`bdos/osmem.c`, see
  `docs/api/structures/md.md` "MD meta-allocator" — `MEMTYPE_DND = 2`,
  `MEMTYPE_DMD = 1`, `MEMTYPE_OFD = 3`, `bdos/mem.h:44-48`). Each allocation
  is a fixed 64-byte block; the pool holds `NUM_OSM_BLOCKS = 118` such
  blocks total, shared system-wide across MDBLOCKs, DMDs, DNDs, *and* OFDs —
  there is no independent "N cached directories" limit.
- Reclamation is **not LRU**. Two distinct mechanisms exist, neither of which
  tracks recency:
  1. `makdnd()` (`bdos/fsdir.c:1610-1682`), when adding a child DND, first
     scans the **existing siblings of that one parent** for a candidate with
     no children (`!d_left`), not `DND_LOCKED`, not referenced by any
     `dirtbl[]` entry, and with no open files (`d_files == NULL`) — the
     first such sibling found is recycled in place. This is a local,
     single-parent scavenge, not a tree-wide LRU.
  2. `free_available_dnds()` (`bdos/fsdir.c:2053-2074`, via
     `process_dnd_tree()`, `:1957-2044`) runs only when `xmgetblk()` cannot
     get memory for a DND/DMD/OFD request. It walks **every** DND tree on
     **every** drive and frees **every** eligible DND it finds in one pass
     (no children, no open files, not a current directory in `dirtbl[]`, not
     a drive root, not `DND_LOCKED`) — an eligibility sweep, not a
     bounded LRU eviction of "just enough" entries.
  3. **If neither scavenge yields a free block**, `xmgetblk()`
     (`bdos/osmem.c:291-354`) prints `"Out of internal memory... System
     halted!"` and calls `halt()` — **a full system freeze**, not a
     GEMDOS error return. This is why every DND/OFD/DMD allocation site in
     `bdos/fsdir.c`/`bdos/fsopnclo.c`/`bdos/fsdrive.c` is commented "only
     returns if it succeeds" — `MGET(DND)`/`MGET(OFD)`/`MGET(DMD)` for these
     three types genuinely cannot return failure to the caller; they halt
     instead. **Only `MEMTYPE_MDBLOCK` requests get a `NULL` return on
     exhaustion** (`bdos/osmem.c:295-299,327-329`). See
     `docs/api/_discrepancies.md` D-064 — this is a real, previously
     undocumented risk on the `Fopen`/`Fcreate`/`Dcreate`/`Frename` mutation
     paths and needs a NewTOS design decision (do **not** replicate the
     halt).

## `DMD` — Drive Media Block (per mounted drive)

```c
struct _dmd {
/* 0x00 */ RECNO  m_recoff[3];   /* offsets: [0]=fat [1]=dir [2]=data */
/* 0x0C */ WORD   m_drvnum;      /* BIOS drive number */
/* 0x0E */ WORD   m_fsiz;        /* FAT size in records */
/* 0x10 */ WORD   m_clsiz;       /* cluster size in records */
/* 0x12 */ UWORD  m_clsizb;      /* cluster size in bytes */
/* 0x14 */ UWORD  m_recsiz;      /* record size in bytes */

/* 0x16 */ CLNO   m_numcl;       /* total data clusters */
/* 0x18 */ WORD   m_clrlog;      /* log2(clsiz in records) */
/* 0x1A */ WORD   m_clrm;        /* clsiz-in-rec mask: clsiz - 1 */
/* 0x1C */ WORD   m_rblog;       /* log2(recsiz in bytes) */
/* 0x1E */ WORD   m_rbm;         /* recsiz mask: recsiz - 1 */
/* 0x20 */ WORD   m_clblog;      /* log2(clsizb) */
/* 0x22 */ WORD   m_clbm;        /* clsizb mask */
/* 0x24 */ OFD   *m_fatofd;      /* synthetic OFD for the FAT */

/* 0x28 */ OFD   *m_ofl;         /* head of open-files list */
/* 0x2C */ DND   *m_dtl;         /* root DND for this drive */
/* 0x30 */ UBYTE  m_16;          /* 1 = FAT16, 0 = FAT12 */
/* 0x31 */ UBYTE  m_1fat;        /* 1 = single FAT only */
/* 0x32 — total 50 bytes (struct alignment may pad) */
};
```

### Pre-computed log/mask fields

To avoid `MUL`/`DIV` in hot paths:

- `m_clrlog`, `m_clrm`: convert "records → cluster" via shift / and.
- `m_rblog`, `m_rbm`: convert "bytes → record" via shift / and.
- `m_clblog`, `m_clbm`: convert "bytes → cluster" via shift / and.

Required invariant: `recsiz` and `clsiz` are powers of two. Non-power
sizes cause incorrect arithmetic and are not supported by EmuTOS or
classic TOS.

### `m_fatofd` — the FAT as a pseudo-file

EmuTOS treats the FAT itself as a synthetic file whose data area
is the FAT region. This lets the same `bdos/fsbuf.c` cache code
service FAT reads and ordinary file reads. `m_fatofd->o_strtcl`
points at the first FAT sector via the standard cluster→record
machinery.

### `m_ofl` and `DND.d_files`

**Correction:** `m_ofl` is declared (`bdos/fs.h:258`) but **never assigned or
read anywhere** in `bdos/*.c` — it is completely dead/vestigial in this
EmuTOS build, not "every OFD whose drive == this DMD". The claim that it is
"used at `Mediach`-detection time... and at unmount to flush dirty DFDs" was
fabricated; there is no unmount concept in EmuTOS, and the actual
media-change recovery path (`osif()`'s `E_CHNG` handler,
`bdos/bdosmain.c:455-487`) does its cleanup differently — see below.

`DND.d_files` **is** real and used: it is the per-directory list of open
OFDs (every OFD whose containing directory == this DND), walked by `ixdel()`
to detect/close same-process open files before erasing a directory entry
(`bdos/fsopnclo.c:660-669`) and checked by `xrmdir()`/`process_dnd_tree()` to
refuse recycling a DND that still has open files.

### What actually happens on media change

On a detected media change (`E_CHNG`, propagated via `longjmp`), `osif()`
(`bdos/bdosmain.c:455-487`) does **not** flush anything for the old media —
it discards the old DMD/DND tree wholesale: `offree(dmd)` frees every open
OFD still pointing at that DMD (`bdos/bdosmain.c:387-405`, **without**
closing/flushing them — any unwritten data is lost), `freetree(dn)` releases
the whole DND tree, `mark_bcbs_invalid(errdrv)` **invalidates** (does not
flush) that drive's buffer-cache entries (`bdos/bdosmain.c:409-424`), then
the BPB is re-read and the drive re-logged via `log_media()`. This is
"discard, don't persist" — the opposite of a flush. See
`docs/api/_discrepancies.md` D-065 for the corresponding `_state.md` §3
correction.

## Relationship summary

```
   PD.p_curdir[d]  →  index into dirtbl[]  →  DND  ⇄  DND tree
                                                ↓
                                              DMD  ─→  m_ofl  (declared, dead — never used)
                                                       m_dtl  →  root DND
   OFD.o_dnode  →  DND
   OFD.o_dmd    →  DMD
```

## Cross-references

- **Compendium:** internals chapter touches DMD; DND is largely
  EmuTOS / TOS-internal.
- **Profibuch:** "Drive-Media-Block" — agrees on visible layout.
- **`docs/api/structures/ofd.md`** — `o_dnode`/`o_dmd` link.
- **`docs/api/structures/bpb.md`** — DMD subsumes BPB info.
- **`docs/api/gemdos/_state.md`** (TBD D7) — `dirtbl[]`, DND pool,
  DMD array.
- **EmuTOS source:** `bdos/fsdir.c:findit`, `bdos/fsmain.c:log_media`,
  `bdos/fsbuf.c:bufl[]`.

## Configuration

- **Correction:** `BLKDEVNUM = 26` (`include/sysconf.h:43`, drives A:..Z:),
  **not 16** — `drvtbl[BLKDEVNUM]` (`bdos/fsglob.c:28`) is the DMD array size,
  one slot per drive. An earlier draft used 16/A:..P: throughout; see
  `docs/api/_discrepancies.md` D-025 (already corrected in the drive/directory
  function specs, but this structures file had not been updated — fixed now).
- **Correction:** there is no `NDIRS` constant; DND/DMD/OFD share one static
  118-block `osmem[]` pool with the MD allocator (see "Lifetime" above and
  `docs/api/structures/md.md`).
- `CONF_WITH_TT_SHIFTER` does not affect filesystem; matters only for
  total drive count vs. floppy-only configs.

## NewTOS implementation notes

(Empty during Phase 00. PHASE-05 implements directory tree caching;
PHASE-04 implements the DMD per-drive geometry.)
