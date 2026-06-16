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

`d_scan` is a **separate** scan position used by tree walks for
internal operations like flushing modified DNDs at unmount.

### Lifetime

- Root DND created at mount time and never freed.
- Non-root DNDs lazily allocated when path resolution descends into a
  new directory.
- Reclaimed when the DND pool is exhausted, in LRU order,
  skipping `DND_LOCKED` ones.

DND pool size: `NDIRS = 16` (`bdos/fs.h` constant area). Tunable at
build time but ABI-visible only as "system supports up to N cached
directories".

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

Two intersecting open-file linked lists per drive:

- `m_ofl`: every OFD whose drive == this DMD, regardless of which
  directory.
- `DND.d_files`: every OFD whose containing directory == this DND.

Used at `Mediach`-detection time to fail outstanding I/O cleanly,
and at unmount to flush dirty DFDs.

## Relationship summary

```
   PD.p_curdir[d]  →  index into dirtbl[]  →  DND  ⇄  DND tree
                                                ↓
                                              DMD  ─→  m_ofl  →  OFD list
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

- `NDIRS = 16` — DND pool size.
- `BLKDEVNUM = 16` — DMD array size (one per drive).
- `CONF_WITH_TT_SHIFTER` does not affect filesystem; matters only for
  total drive count vs. floppy-only configs.

## NewTOS implementation notes

(Empty during Phase 00. PHASE-05 implements directory tree caching;
PHASE-04 implements the DMD per-drive geometry.)
