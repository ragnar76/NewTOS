# AES Resource Manager (`rsrc_*` 110–114) + File Selector (`fsel_*` 90–91)

> **PHASE-00b D8, Batch 6.** Source: EmuTOS @ `1a7484ab` —
> `aes/gemrslib.c` (`rs_*`), `aes/gemfslib.c` (`fs_input`), header
> `include/rsdefs.h`, types `include/aesdefs.h:578-593`, dispatch
> `aes/gemsuper.c`.
>
> Call convention: [`_dispatcher.md`](./_dispatcher.md).

## The `.RSC` file format (`RSHDR`, `rsdefs.h:16-36`)

A resource file is a self-contained blob: a header of **offsets + counts**
followed by the object trees, TEDINFOs, ICONBLKs, BITBLKs and strings.
All internal pointers are stored as **byte offsets from the file base**
and must be **fixed up** to absolute addresses on load (`rs_load`).

```c
typedef struct {
/* +0x00 */ UWORD rsh_vrsn;      /* format version (0 = classic) */
/* +0x02 */ UWORD rsh_object;    /* offset: OBJECT array */
/* +0x04 */ UWORD rsh_tedinfo;   /* offset: TEDINFO array */
/* +0x06 */ UWORD rsh_iconblk;   /* offset: ICONBLK array */
/* +0x08 */ UWORD rsh_bitblk;    /* offset: BITBLK array */
/* +0x0A */ UWORD rsh_frstr;     /* offset: free-string ptr array */
/* +0x0C */ UWORD rsh_string;    /* offset: string data */
/* +0x0E */ UWORD rsh_imdata;    /* offset: image data */
/* +0x10 */ UWORD rsh_frimg;     /* offset: free-image ptr array */
/* +0x12 */ UWORD rsh_trindex;   /* offset: tree-index (offset per tree) */
/* +0x14 */ WORD  rsh_nobs;      /* count: objects */
/* +0x16 */ WORD  rsh_ntree;     /* count: trees */
/* +0x18 */ WORD  rsh_nted;      /* count: TEDINFOs */
/* +0x1A */ WORD  rsh_nib;       /* count: ICONBLKs */
/* +0x1C */ WORD  rsh_nbb;       /* count: BITBLKs */
/* +0x1E */ WORD  rsh_nstring;   /* count: strings */
/* +0x20 */ WORD  rsh_nimages;   /* count: images */
/* +0x22 */ UWORD rsh_rssize;    /* total file size in bytes */
} RSHDR;   /* 36 bytes */
```

The fix-up (`rs_load`) walks `rsh_trindex` to build `ap_ptree` (the tree
table in `AESGLOBAL`), then converts every offset field (object `ob_spec`,
TEDINFO text/template/valid, ICONBLK mask/data/text, BITBLK data, free
strings/images) from file-relative to absolute, and applies `rs_obfix`
(char→pixel coords) to every object.

## Resource object types (`R_*`, `aesdefs.h:578-593`)

Selector for `rsrc_gaddr`/`rsrc_saddr` — which array + indexing scheme:

| | | | |
|--|--|--|--|
| 0 `R_TREE` | 1 `R_OBJECT` | 2 `R_TEDINFO` | 3 `R_ICONBLK` |
| 4 `R_BITBLK` | 5 `R_STRING` | 6 `R_IMAGEDATA` | 7 `R_OBSPEC` |
| 8 `R_TEPTEXT` (TEDINFO sub-ptrs) | … | 15 `R_FRSTR` (free strings) | |

## Functions

| Op | Name | Dispatch | Effect |
|---:|------|----------|--------|
| 110 | `rsrc_load`  | `rs_load(global, fname)` | load `fname.RSC`, allocate, fix up all offsets→addresses, build `ap_ptree`. Returns 1 / 0 on fail. **Uses GEMDOS** (`Fopen`/`Fread`/`Malloc`) |
| 111 | `rsrc_free`  | `rs_free(global)` | free the loaded resource + tree table |
| 112 | `rsrc_gaddr` | `rs_gaddr(global, type, index, &addr)` | look up the address of resource object `index` of `R_*` `type` → addrout[0] |
| 113 | `rsrc_saddr` | `rs_saddr(global, type, index, addr)` | store an address into the resource table (for app-built objects) |
| 114 | `rsrc_obfix` | `rs_obfix(tree, obj)` | convert one object's char-grid `ob_x/y/w/h` to pixel coords (`hi byte`=chars, `lo byte`=pixel offset) |

`intin`: `RS_TYPE`/`RS_INDEX`=intin[0..1], `RS_OBJ`=intin[1];
`RS_PFNAME`/`RS_INADDR`/`RS_TREE`=addrin[0]; result in addrout[0].

## File Selector (`fsel_*`)

| Op | Name | Dispatch | Effect |
|---:|------|----------|--------|
| 90 | `fsel_input`   | `fs_input(path, sel, &button, NULL)` | modal file-selector dialog. `path` = in/out directory+wildcard, `sel` = in/out filename; `button` = 1 OK / 0 Cancel |
| 91 | `fsel_exinput` | `fs_input(path, sel, &button, title)` | same, plus a custom `title` string |

`path`/`sel` are **in-out** user buffers (addrin[0..1], addrout via the
same buffers); the AES reads the directory via GEMDOS (`Fsfirst`/`Fsnext`)
and writes the user's choice back.

---

## NewTOS implementation notes

- `rsrc_load` is the bridge from **GEMDOS files** (Phase 08/09 — `Fopen`/
  `Fread`/`Malloc`, all already implemented) to the in-memory object trees
  the rest of the AES draws. The **offset→address fix-up** is the core
  logic and is pure arithmetic over the loaded blob — easy to unit-test on
  host against a known `.RSC`.
- `rs_obfix` coordinate convention: each `ob_x/y/w/h` packs a character
  count in the high byte and a pixel offset in the low byte; fix-up
  multiplies the char count by `gl_wchar`/`gl_hchar` (from `graf_handle`,
  Batch 4) and adds the offset. Get this exactly right — every dialog's
  layout depends on it.
- `fsel_input` needs: the directory read (GEMDOS `Fsfirst`/`Fsnext`, done),
  an object-tree dialog (`form_do`, Batch 3), and a scrollable list — so it
  comes **after** the Form + Window managers.
- The `.RSC` loader + `rsrc_gaddr` are what let real GEM apps (and our own
  desktop `DESK.RSC`) run unmodified → high value for compatibility.
