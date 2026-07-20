# AES Resource Manager (`rsrc_*` 110–114) + File Selector (`fsel_*` 90–91)

> **PHASE-00b D8, Batch 6 (full-contract refinement).**
> Source: EmuTOS @ `1a7484ab`.
> Primary: `aes/gemrslib.c` (`rs_load`/`rs_free`/`rs_gaddr`/`rs_saddr`/
> `rs_obfix`, the `.RSC` load + relocation `fix_trindex`/`fix_objects`/
> `fix_tedinfo_std`/`fix_nptrs`/`fix_long`, the coordinate fix
> `rs_obfix`/`fix_chpos`), `aes/gemfslib.c` (`fs_input` = both `fsel_input`
> and `fsel_exinput`, the FSELECTR dialog). Dispatch arms
> `aes/gemsuper.c:308-326,362-377`. RSC header `include/rsdefs.h:16-36`;
> `R_*` type codes + slot macros `include/aesdefs.h:456-465,568-594`;
> `AESGLOBAL` resource fields `aes/gemrslib.h:15-27`.
>
> Call convention + AESPB layout: [`_dispatcher.md`](./_dispatcher.md).
> Shared AES globals (the loaded-resource `ap_rscmem`/`ap_ptree`, the shell
> path used by `rs_load`→`sh_find`): [`_state.md`](./_state.md) §6c.
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** the opcode numbers (110–114, 90–91); each call's
> `int_in`/`addr_in`/`int_out`/`addr_out` slot layout (note `rsrc_gaddr` is the
> **only** AES call whose `addr_out[0]` the dispatcher copies back, and
> `fsel_*`'s button return is `int_out[1]`, **not** an `addr_out`); the
> **`.RSC` on-disk file format** (the `RSHDR` byte layout, the offset-table +
> count fields, the `NEW_FORMAT_RSC` bit); the offset→pointer **relocation
> rule** (every internal reference is a byte offset from the file base fixed to
> an absolute address, `-1L` left unrelocated); the **`rs_obfix` char→pixel
> coordinate rule** (hi byte = char count × cell size, lo byte = signed pixel
> offset, `w == 80` → full screen width); the **`R_*` type→array map** used by
> `rsrc_gaddr`/`rsrc_saddr`; `rsrc_gaddr`'s `1`/`0` found/not-found return; the
> `fsel_*` in-out path/name buffers and the `path`/`mask` split rule; the
> `fsel_*` button return (1 = OK, 0 = Cancel). **Reference (NewTOS reimplements
> the *how* freely):** the `rs_hdr`/`rs_global` file-local statics, the CICON
> fix-up/expansion (`fix_cicons`/`transform_all_cicons`), the FSELECTR resource
> tree drawing, `fs_active`'s shell-sort + `ad_fsnames`/`g_fslist` buffers,
> `fs_nscroll`'s `bb_screen` blit-scroll, and the `dos_alloc_anyram` work-area
> layout — these are the EmuTOS internals NewTOS rebuilds on its own file I/O,
> object manager, and rasteriser.
>
> **NewTOS deviation (ADR-013, D-100).** Everything in §0 below is EmuTOS's
> **classic Atari on-disk `.RSC` format** — accurately extracted, and it is
> what a byte-compatible loader (a future m68k-emulator compat phase) would
> need. **NewTOS's actual `rsrc_load` does not read this format at all.**
> Per ADR-013 (`docs/MEMORY.md`), NewTOS defines its own **native** `.RSC`
> (`NrscHeader`, magic `"NRSC"`, little-endian, 64-bit `ob_spec`, offsets
> from blob base, coordinates already in pixels — `tos-api/src/aes/resource.rs`);
> `rsrc_load` searches a **hardcoded** `C:\GEMSYS\RSC\<name>` then `C:\<name>`
> (not `sh_find`'s shell-path search), and `rsrc_obfix` is an **identity** (no
> char→pixel fix — native resources are already in pixels). The `rsrc_gaddr`/
> `rsrc_saddr` ABI slots, the `R_*` type semantics, and the 1/0 return
> convention remain contract; the on-disk byte layout, `sh_find`, and the
> char→pixel rule are EmuTOS-reference only for NewTOS's current native-format
> loader. Full accounting: "NewTOS implementation notes" below and
> `docs/api/_discrepancies.md` D-100.

---

## 0. The `.RSC` file format (`RSHDR`, `rsdefs.h:16-36`)

A resource file is a self-contained blob: a fixed 36-byte header of **byte
offsets + counts** followed by the object arrays, TEDINFOs, ICONBLKs, BITBLKs,
the free-string / free-image pointer tables, the string/image data, and the
tree index. Every internal reference stored on disk is a **byte offset from the
file base**; on load these are converted to absolute pointers. m68k = big-endian
on disk (the header WORDs are big-endian).

| Off | Field | Type | Meaning |
|----:|-------|------|---------|
| 0x00 | `rsh_vrsn`    | UWORD | format version; **bit 2 (`NEW_FORMAT_RSC = 0x0004`)** set → new-format file with CICONBLKs + a trailing extension array |
| 0x02 | `rsh_object`  | UWORD | offset: OBJECT array |
| 0x04 | `rsh_tedinfo` | UWORD | offset: TEDINFO array |
| 0x06 | `rsh_iconblk` | UWORD | offset: ICONBLK array |
| 0x08 | `rsh_bitblk`  | UWORD | offset: BITBLK array |
| 0x0A | `rsh_frstr`   | UWORD | offset: **free-string** pointer array (LONG offsets) |
| 0x0C | `rsh_string`  | UWORD | offset: string data |
| 0x0E | `rsh_imdata`  | UWORD | offset: image data |
| 0x10 | `rsh_frimg`   | UWORD | offset: **free-image** pointer array (LONG offsets) |
| 0x12 | `rsh_trindex` | UWORD | offset: tree-index (one LONG offset per tree) |
| 0x14 | `rsh_nobs`    | WORD  | count: objects |
| 0x16 | `rsh_ntree`   | WORD  | count: trees |
| 0x18 | `rsh_nted`    | WORD  | count: TEDINFOs |
| 0x1A | `rsh_nib`     | WORD  | count: ICONBLKs |
| 0x1C | `rsh_nbb`     | WORD  | count: BITBLKs |
| 0x1E | `rsh_nstring` | WORD  | count: strings (free strings) |
| 0x20 | `rsh_nimages` | WORD  | count: images (free images) |
| 0x22 | `rsh_rssize`  | UWORD | total classic-format file size in bytes |

> **`NEW_FORMAT_RSC` bit.** `rsh_vrsn & 0x0004` marks a *new-format* file with
> colour icons (`rsdefs.h:39`). For such files `rsh_rssize` is only the classic
> size; the **true file length** is read as a LONG at file offset `rsh_rssize`
> (`gemrslib.c:766-772`), and a trailing extension array (true length, CICON
> table offset, …) follows. The previous spec called `rsh_vrsn == 0` "classic";
> it is a **bit test**, not an equality.

### The relocation (what `rs_load` fixes up)

`rs_readit`/`rs_fixit` (`gemrslib.c:751-820`) turn the loaded blob into a live
tree by walking each count and converting offsets to absolute pointers via
`fix_long` (`add rs_hdr base`, leaving `-1L` untouched, `gemrslib.c:576-588`):

1. **`fix_trindex`** (`:591-603`) — point `rs_global->ap_ptree` at the tree-index
   array and relocate each of `rsh_ntree` tree offsets. This is what builds the
   `ap_ptree[]` table `rsrc_gaddr(R_TREE)` and the rest of the AES read.
2. **CICONs** (`fix_cicons`, `#if CONF_WITH_COLOUR_ICONS`) — fix up the CICONBLK
   pointer table and expand/transform each colour icon to the device format
   (reference; see §CICON note).
3. **`fix_tedinfo_std`** (`:655-669`) — for each of `rsh_nted` TEDINFOs relocate
   `te_ptext`/`te_ptmplt`/`te_pvalid` and recompute `te_txtlen`/`te_tmplen`
   from the relocated strings.
4. **`fix_nptrs`** for ICONBLK `ib_pmask`/`ib_pdata`/`ib_ptext` (`rsh_nib` each),
   BITBLK `bi_pdata` (`rsh_nbb`), free strings (`rsh_nstring`) and free images
   (`rsh_nimages`) (`:799-805`).
5. **`fix_objects`** (`rs_fixit`, `:606-637`) — for each of `rsh_nobs` objects:
   apply `rs_obfix` (coordinate fix, below) and relocate `ob_spec` **unless the
   type is `G_BOX`/`G_IBOX`/`G_BOXCHAR`** (whose `ob_spec` is a colour word, not
   a pointer); `G_CICON` `ob_spec` is turned into a CICONBLK-table index→pointer.

> **Object fix-up is deferred past the header read.** `rs_readit` reads and
> relocates everything *except* object coordinates/`ob_spec`; `rs_fixit` does
> those afterwards. This split exists because the char→pixel conversion needs an
> open VDI workstation (`gl_wchar`/`gl_hchar`), which is not up when the GEM
> resource itself is loaded at boot (`gemrslib.c:745-750,811-820`). For an
> app-level `rsrc_load` both run back-to-back (`rs_load`, `:826-850`).

### `R_*` resource type codes (`aesdefs.h:578-594`)

`rsrc_gaddr`/`rsrc_saddr` select **which array and indexing scheme** via the
type code. `get_addr` (`gemrslib.c:112-176`) resolves each:

| Code | Name | `rsrc_gaddr` returns |
|-----:|------|----------------------|
| 0 | `R_TREE`      | `ap_ptree[index]` — the index-th tree's OBJECT base |
| 1 | `R_OBJECT`    | `&object[index]` (base `rsh_object`) |
| 2 | `R_TEDINFO`   | `&tedinfo[index]` (base `rsh_tedinfo`) |
| 3 | `R_ICONBLK`   | `&iconblk[index]` (base `rsh_iconblk`) |
| 4 | `R_BITBLK`    | `&bitblk[index]` (base `rsh_bitblk`) |
| 5 | `R_STRING`    | the free-string **data** pointer `frstr[index]` (dereferenced) |
| 6 | `R_IMAGEDATA` | the free-image **data** pointer `frimg[index]` (dereferenced) |
| 7 | `R_OBSPEC`    | `&object[index].ob_spec` |
| 8 | `R_TEPTEXT`   | `&tedinfo[index]` (== `R_TEDINFO`; `te_ptext` is field 0) |
| 9 | `R_TEPTMPLT`  | `&tedinfo[index].te_ptmplt` |
| 10 | `R_TEPVALID` | `&tedinfo[index].te_pvalid` |
| 11 | `R_IBPMASK`  | `&iconblk[index]` (== `R_ICONBLK`; `ib_pmask` is field 0) |
| 12 | `R_IBPDATA`  | `&iconblk[index].ib_pdata` |
| 13 | `R_IBPTEXT`  | `&iconblk[index].ib_ptext` |
| 14 | `R_BIPDATA`  | `&bitblk[index]` (== `R_BITBLK`; `bi_pdata` is field 0) |
| 15 | `R_FRSTR`    | `&frstr[index]` — address **of** the free-string pointer slot |
| 16 | `R_FRIMG`    | `&frimg[index]` — address **of** the free-image pointer slot |
| other | — | `(void *)-1L` (bad type; `rsrc_gaddr` then returns 0) |

> **Correction vs. the previous spec.** (1) The type table runs **0–16**, not
> 0–15; `R_FRIMG = 16` (free images) exists (`aesdefs.h:594`). (2) `R_STRING`
> (5)/`R_IMAGEDATA` (6) return the **dereferenced data** pointer, whereas
> `R_FRSTR` (15)/`R_FRIMG` (16) return the **address of the table slot** — the
> pair `get`/`set` a free string. (3) Codes 8/11/14 alias 2/3/4 because the
> sub-pointer is the struct's first field (`gemrslib.c:129-142`). See D-054.

---

## 1. `.RSC` load / free

### `rsrc_load` (110) — `rs_load`

**Source:** `gemsuper.c:363-365` → `aes/gemrslib.c:826-850` (`rs_load`).
Helpers: `sh_find` (locate the file via the shell search path, §Dependencies),
`dos_open`/`dos_read`/`dos_lseek`/`dos_close` (GEMDOS file I/O),
`dos_alloc_anyram` (allocate the in-memory blob), `rs_readit`+`rs_fixit`
(read + relocate, §0). **Globals written:** `ap_rscmem` (the loaded blob base),
`ap_rsclen` (its length), `ap_ptree` (the relocated tree table) — all in the
caller's `AESGLOBAL` (`gemrslib.c:786-788,597`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`RS_PFNAME`) | pointer to the resource filename (NUL-terminated) |
| `int_out[0]` | `1` if loaded + relocated, `0` on any failure |

**Behaviour.** Copy the filename to a scratch buffer and **run `sh_find()`** on
it (`gemrslib.c:835-837`) so the resource is located via the same directory
search as an executable (app dir → current dir → root → AES `PATH=`, see
`shel_find` in [`scrap-shell.md`](./scrap-shell.md)); return `0` if not found.
Open read-only, read the 36-byte header, allocate `rsh_rssize` (or the true
length for a new-format file) bytes of any RAM, read the whole file in, store
`ap_rscmem`/`ap_rsclen`, then relocate every offset to a pointer and fix the
coordinates (§0). Returns `0` (and frees nothing) on a short read or alloc
failure; `1` on success. Only **one** resource is loaded per application at a
time — a second `rsrc_load` overwrites `ap_rscmem` without freeing the first
(the app must `rsrc_free` between loads).

> **NewTOS status (deviation, ADR-013/D-100).** NewTOS's `rsrc_load`
> (`tos-api/src/aes/resource.rs::rsrc_load`) does none of the above at the
> byte level: it copies the bare filename from `addr_in[0]`, tries
> `C:\GEMSYS\RSC\<name>` then `C:\<name>` (a **fixed two-entry search**, not
> `sh_find`), reads the file into a static 64 KiB buffer
> (`crate::bdos::fs::read_file`), and — if the header magic is `"NRSC"` —
> relocates the **native** offset table in place (`relocate_and_register`).
> There is no classic `RSHDR`, no big-endian header, no `NEW_FORMAT_RSC`/CICON
> path, and no separate coordinate-fix pass (coordinates are already pixels;
> see `rsrc_obfix` below). The `int_out[0] = 1`/`0` success contract is
> preserved.

### `rsrc_free` (111) — `rs_free`

**Source:** `gemsuper.c:366-367` → `aes/gemrslib.c:685-708` (`rs_free`).

| Slot | Meaning |
|------|---------|
| (no `int_in`/`addr_in`) | operates on the caller's `AESGLOBAL` (`global`) |
| `int_out[0]` | `1` on success, `0` if nothing was loaded or a free failed |

Frees the CICON buffers (if any) and the resource blob `ap_rscmem`
(`dos_free`). Returns `0` if `ap_rscmem` was NULL (nothing loaded) or if any
`dos_free` failed (`gemrslib.c:691-705`). It does **not** clear `ap_ptree` — a
program must not `rsrc_gaddr` after `rsrc_free`.

---

## 2. `rsrc_gaddr` / `rsrc_saddr` — get/set a resource address

### `rsrc_gaddr` (112) — `rs_gaddr`

**Source:** `gemsuper.c:369-371` → `aes/gemrslib.c:715-721` (`rs_gaddr`),
`get_addr` (`:112-176`). **This is the only AES opcode whose `addr_out[0]` the
dispatcher copies back to the caller** ([`_dispatcher.md`](./_dispatcher.md) §2,
`gemsuper.c:446-447`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` (`RS_TYPE`) | `R_*` type code (0–16, §0) |
| `int_in[1]` (`RS_INDEX`) | index within that type's array |
| `addr_out[0]` (`RS_OUTADDR`) | the resolved address (`get_addr` result) |
| `int_out[0]` | `1` if found, **`0` if the type was invalid** (result was `-1L`) |

Resolves `(type, index)` to an absolute address per the `R_*` table (§0) and
writes it to `addr_out[0]`. The return is `(*rsaddr != -1L)` — `1` for any valid
type, `0` only for an out-of-range type code (`gemrslib.c:720`). **Note it does
not range-check `index`** — a too-large index returns `1` with a bogus pointer
(TOS behaviour; the resource's counts bound valid indices).

### `rsrc_saddr` (113) — `rs_saddr`

**Source:** `gemsuper.c:372-373` → `aes/gemrslib.c:728-742` (`rs_saddr`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` (`RS_TYPE`) | `R_*` type code |
| `int_in[1]` (`RS_INDEX`) | index |
| `addr_in[0]` (`RS_INADDR`) | the address to store |
| `int_out[0]` | `TRUE(1)` on success, `FALSE(0)` if the type was invalid |

Stores `addr_in[0]` **into the resource's pointer slot** at `(type, index)`.
Only meaningful for the *pointer* types (`R_STRING`/`R_IMAGEDATA` via
`R_FRSTR`/`R_FRIMG`, the TEDINFO/ICONBLK sub-pointers, `R_OBSPEC`): `get_addr`
returns the address **of** the slot and `rs_saddr` writes through it
(`gemrslib.c:734-737`). Lets an application point a resource object at its own
runtime string/image/spec. Returns `FALSE` for a bad type
(`get_addr` == `-1L`).

---

## 3. `rsrc_obfix` (114) — char→pixel coordinate fix

**Source:** `gemsuper.c:375-377` → `aes/gemrslib.c:89-99` (`rs_obfix`),
`fix_chpos` (`:53-83`). Reads `gl_wchar`/`gl_hchar` (the system-font cell size
from `graf_handle`, see [`window-graphics.md`](./window-graphics.md)) and
`gl_width` (screen width).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`RS_TREE`) | the OBJECT tree base |
| `int_in[0]` (`RS_OBJ`) | the object index to fix |
| `int_out[0]` | `1` (`rs_obfix` is `void`; `crysbind` `ret` stays `TRUE`) |

**The coordinate rule (contract).** Each of the object's four packed WORDs
`ob_x`, `ob_y`, `ob_w`, `ob_h` encodes **a character count in the high byte and
a signed pixel offset in the low byte** (`fix_chpos`, `gemrslib.c:53-83`):

| Field | offset | conversion |
|-------|-------:|------------|
| `ob_x` | 0 | `char × gl_wchar` |
| `ob_y` | 1 | `char × gl_hchar` |
| `ob_w` | 2 | `char × gl_wchar`, **except `char == 80` → `gl_width`** (full-screen width) |
| `ob_h` | 3 | `char × gl_hchar` |

then **add the low-byte offset as signed** (`coffset > 128 ? coffset-256 :
coffset`). So `ob_x = 0x0A05` = column 10 → `10·gl_wchar + 5` px. Get this
exactly right — every dialog's layout, and `rsrc_load`'s `fix_objects`, depend
on it. `rs_obfix` fixes **one** object; `fix_objects` applies it to all
`rsh_nobs` objects during load.

> **Verified vs. the previous spec.** The old note ("hi byte = chars, lo byte =
> pixel offset, multiply by `gl_wchar`/`gl_hchar`") is correct in spirit but
> missed two source facts: the low-byte offset is **signed** (>128 → −256), and
> **width == 80 maps to full screen width** (`gemrslib.c:71-74,81`). See D-054.

> **NewTOS status (deviation, ADR-013/D-100).** `rsrc_obfix`
> (`tos-api/src/aes/resource.rs::rsrc_obfix`) is an **identity operation** —
> it touches no coordinates and just reports success (`int_out[0] = 1`).
> Native `.RSC` objects already store pixel `ob_x`/`ob_y`/`ob_width`/
> `ob_height` (no char-grid encoding survives the host-side `mkrsc.py`
> conversion from a classic `.rsc`), so there is nothing left for
> `rsrc_obfix` to do. The char→pixel rule above is EmuTOS-reference only.

---

## 4. File Selector (`fsel_*` 90–91)

Both opcodes call the same worker `fs_input(pipath, pisel, pbutton, pilabel)`;
they differ only in whether a custom title label is passed
(`gemsuper.c:322-326`).

### `fsel_input` (90) / `fsel_exinput` (91) — `fs_input`

**Source:** `gemsuper.c:322-326` → `aes/gemfslib.c:609-926` (`fs_input`).
Helpers: `fs_start` (build/centre the FSELECTR dialog, `:59-173`), `fs_active`
(read + sort the directory, `:268-337`), `fs_newdir`/`fs_format`/`fs_nscroll`
(fill + scroll the list), `fm_dial`(`FMD_START`/`FMD_FINISH`) + `fm_do` (the
modal loop), `set_mask`/`fs_pspec`/`fs_back` (path/mask split), `wildcmp` (TOS
wildcard match), `select_drive`, `inf_what` (decode the exit button),
`dos_sfirst`/`dos_snext` (GEMDOS directory read). **Globals read:** `drvbits`
(the BIOS drive bitmap → which drive buttons are enabled), `rs_trees[FSELECTR]`
(the resource dialog tree). Allocates a work area with `dos_alloc_anyram`.

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`FS_IPATH`) | **in/out** path buffer: directory + wildcard mask (e.g. `C:\FOO\*.TXT`) |
| `addr_in[1]` (`FS_ISEL`) | **in/out** filename buffer: the selected name (no path) |
| `addr_in[2]` (`FS_TITLE`) | **`fsel_exinput` only** — custom dialog label; **`fsel_input` passes NULL** |
| `int_out[1]` (`FS_BUTTON`) | **1 = OK, 0 = Cancel** (`inf_what` result; see below) |
| `int_out[0]` | `1` if the dialog ran, **`0`** if `path` was NULL or the work-area alloc failed |

**The path/name split (contract).** On entry `pipath` holds a full spec
`drive:\dir\mask`. `fs_input` splits it: `set_mask`/`fs_pspec`/`fs_back` walk
back to the last `\` (or the `:` drive separator) to separate the **directory
prefix** from the **wildcard mask** (`gemfslib.c:199-236,524-532`). An empty
mask defaults to `*.*` (`set_all_files`, `:530`). An empty `pipath` defaults to
`"A:\*.*"` with the current drive letter substituted (`:634-638`). The mask
drives the directory filter (`wildcmp`, `:300`); subdirectories always show
(and sort first, `\007` prefix, `:241-254`). On exit `pipath` is rewritten with
the final directory+mask (`locstr`) and `pisel` with the chosen filename
(unformatted, dot restored, `:914-916`).

**The exit button (contract).** `*pbutton = inf_what(tree, FSOK)`
(`gemfslib.c:922`, `util/optimize.c:157-172`): **1** if the OK button was
selected, **0** if Cancel (the object after OK), **−1** otherwise; the routine
also deselects that button. This is written to `int_out[1]`. So a caller reads
`int_out[0]` (did the dialog run) then `int_out[1]` (OK vs Cancel).

> **Corrections vs. the previous spec.** (1) The button return is **`int_out[1]`
> (`FS_BUTTON`)**, *not* an `addr_out` and not written "via the same buffers"
> (`aesdefs.h:465`). (2) `fs_input`'s **`int_out[0]` return is always 1 when the
> dialog ran** (0 only on NULL path / no memory) — the OK/Cancel outcome is
> `int_out[1]`, decoded by `inf_what`, and can be **−1** if neither button ended
> the loop. (3) `fsel_exinput`'s label is `addr_in[2]` (`FS_TITLE`), defaulting
> to the "Item Selector" string when NULL (`gemfslib.c:691-692`). See D-054.

> **NewTOS status (GEM.RSC lazy-load, D-100).** NewTOS has no in-ROM
> `rs_trees[FSELECTR]` (see the `ap_private`/D-099 note: NewTOS's AES kernel
> carries no built-in resource ensemble at all). Instead, `fsel_input`/
> `fsel_exinput` (`tos-api/src/aes/fsel.rs::load_fselectr`) load
> `C:\GEMSYS\RSC\GEM.RSC` (the EmuTOS `gem.rsc`, converted to native format by
> `tools/mkrsc.py convert`) into a **second, dedicated native-`.RSC` buffer**
> that coexists with whatever the calling application loaded via `rsrc_load`
> — the two do not share `ap_rscmem`. The load happens **lazily, on the
> first `fsel_input`/`fsel_exinput` call**, and the resolved tree root is
> cached (`SYS.root`) so later calls skip the file read. This is the concrete
> mechanism behind D-099's "the only resource NewTOS's AES loads is `GEM.RSC`,
> and only lazily, on first use of the file selector".

---

## Trap interface (parameter-block form)

All calls enter via the GEM trap (`svc #2`) with **selector 200 in `x0`** and an
**AESPB pointer in `x1`** — see [`_dispatcher.md`](./_dispatcher.md).

| Op | Function | `int_in` | `addr_in` | `int_out` / `addr_out` |
|---:|----------|----------|-----------|------------------------|
| 90 | `fsel_input`   | — | `[0]`=path (io), `[1]`=name (io) | `[0]`=ran(1/0), `int_out[1]`=button(1/0) |
| 91 | `fsel_exinput` | — | `[0]`=path (io), `[1]`=name (io), `[2]`=label | `[0]`=ran(1/0), `int_out[1]`=button(1/0) |
| 110 | `rsrc_load`  | — | `[0]`=filename | `[0]`=1/0 |
| 111 | `rsrc_free`  | — | — | `[0]`=1/0 |
| 112 | `rsrc_gaddr` | `[0]`=type, `[1]`=index | — | `[0]`=1/0, **`addr_out[0]`=address** |
| 113 | `rsrc_saddr` | `[0]`=type, `[1]`=index | `[0]`=address | `[0]`=1/0 |
| 114 | `rsrc_obfix` | `[0]`=object | `[0]`=tree | `[0]`=1 (void) |

---

## Inputs / Outputs summary

Inputs arrive in the AESPB arrays. `rsrc_gaddr` is the sole AES call that returns
through **`addr_out[0]`**; everything else returns via `int_out[0]` (`1`/`0`) or
writes through a caller pointer (`rsrc_saddr` into the resource, `fsel_*` into
`pipath`/`pisel`, coordinates into the tree for `rsrc_obfix`). `fsel_*`'s
OK/Cancel is a *second* `int_out` slot, `int_out[1]`. `rsrc_load`/`rsrc_free`
mutate the caller's `AESGLOBAL` (`ap_rscmem`/`ap_rsclen`/`ap_ptree`); the loaded
tree array itself is written into the AES-allocated blob, not the caller's
memory.

---

## Error conditions

The resource/file-selector managers return small boolean/index values, not TOS
`E*` codes:

| Condition | Source | Return |
|-----------|--------|--------|
| `rsrc_load` file not found via `sh_find` | `gemrslib.c:836-837` | `int_out[0] = 0` |
| `rsrc_load` open / short read / alloc failure | `gemrslib.c:758-783,839-841` | `int_out[0] = 0` |
| `rsrc_free` with nothing loaded, or `dos_free` failed | `gemrslib.c:699-705` | `int_out[0] = 0` |
| `rsrc_gaddr`/`rsrc_saddr` with an invalid `R_*` type | `gemrslib.c:171-172,720,735` | `int_out[0] = 0`/`FALSE` |
| `fsel_input` with a NULL path pointer | `gemfslib.c:630-631` | `int_out[0] = 0` |
| `fsel_input` work-area alloc failure (`< NM_NAMES` slots) | `gemfslib.c:655-662` | `int_out[0] = 0` (after `ALFSMEM` alert) |
| `fsel_input` Cancel pressed | `gemfslib.c:922`, `optimize.c:168` | `int_out[0] = 1`, `int_out[1] = 0` |

A directory-read error inside the selector (`fs_active`) is shown to the user via
`form_error` and retried once; `EFILNF`/`ENMFIL` (empty/no-match dir) are treated
as success — an empty list, not an error (`gemfslib.c:330-336,733-754`).

---

## Verification criteria

- **`rsrc_gaddr(R_TREE, i)` returns the tree base:** after a successful
  `rsrc_load`, `rsrc_gaddr(type=0, index=i)` sets `addr_out[0] = ap_ptree[i]`
  and returns `int_out[0] = 1`; an out-of-range **type** (e.g. 17) returns
  `int_out[0] = 0` with `addr_out[0] = -1L` (`gemrslib.c:123-124,171-172,720`).
- **`rsrc_gaddr(R_OBJECT, i)` vs `R_OBSPEC`:** `type=1` returns
  `&object[i]`; `type=7` returns `&object[i].ob_spec` — the same object, a
  different field (`gemrslib.c:125-127,144-146`).
- **`rsrc_obfix` coordinate fix:** an object with `ob_x = 0x0A05` becomes
  `10·gl_wchar + 5`; `ob_w = 0x5000` (char 80) becomes `gl_width`
  (full-screen); a low byte `0xFF` subtracts 1 px (`gemrslib.c:64-82`).
- **`rsrc_load` relocation:** after load, `ap_ptree[0]` and every object's
  relocated `ob_spec` are absolute addresses within `[ap_rscmem,
  ap_rscmem+ap_rsclen)`; a field stored as `-1L` on disk stays `-1L`
  (`gemrslib.c:581-585`).
- **`rsrc_load` not found → 0:** loading a filename `sh_find` cannot locate
  returns `int_out[0] = 0` and leaves `ap_rscmem` unchanged (`gemrslib.c:836`).
- **`rsrc_free` idempotency guard:** `rsrc_free` with `ap_rscmem == NULL`
  returns `0` (`gemrslib.c:702-704`).
- **`fsel_input` OK path:** given `pipath = "C:\*.TXT"`, on OK the call returns
  `int_out[0] = 1`, `int_out[1] = 1`, `pipath` = the final directory+mask, and
  `pisel` = the picked filename; on Cancel `int_out[1] = 0`
  (`gemfslib.c:914-922`).
- **`fsel_input` empty path default:** `pipath = ""` is rewritten to
  `"<curdrive>:\*.*"` before the dialog opens (`gemfslib.c:634-638`).
- **`fsel_input` NULL path:** `addr_in[0] == NULL` returns `int_out[0] = 0`
  without opening the dialog (`gemfslib.c:630-631`).
- **`fsel_exinput` label:** `fsel_exinput` shows the `addr_in[2]` string as the
  dialog title; `fsel_input` shows the default "Item Selector"
  (`gemfslib.c:691-692`).

> **Applicability note (D-100).** The criteria above verify the **EmuTOS
> source** (the primary specification, per CLAUDE.md Rule 1) and are the
> right target for a future classic-`.RSC`-compatible loader. Two of them do
> not hold against NewTOS's *current* native-format implementation: the
> `rsrc_obfix` coordinate-fix criterion (NewTOS's `rsrc_obfix` is an
> identity — no coordinates change), and the `sh_find`-based "not found"
> mechanism in the `rsrc_load` criterion (NewTOS fails to find a file via its
> own fixed `C:\GEMSYS\RSC\`/`C:\` search instead — the `int_out[0] = 0`
> outcome and the "leaves the loaded resource unchanged" behaviour still
> hold, just via a different lookup). See the "NewTOS status" callouts under
> `rsrc_load (110)`/`rsrc_obfix (114)` and "NewTOS implementation notes".

---

## Edge cases discovered in source

1. **`rsrc_load` searches the shell path.** It calls `sh_find` first
   (`gemrslib.c:836`), so a bare `"MYAPP.RSC"` is resolved against the app dir,
   current dir, drive root, and the AES `PATH=` — not just the current
   directory. A program relies on this to ship an `.RSC` next to its executable.
2. **`R_*` type table is 0–16, and the "sub-pointer" codes alias their parent.**
   `R_TEPTEXT`(8)=`R_TEDINFO`(2), `R_IBPMASK`(11)=`R_ICONBLK`(3),
   `R_BIPDATA`(14)=`R_BITBLK`(4) — the sub-pointer is the struct's first field
   (`gemrslib.c:129-142`). `R_FRIMG`(16) was missing from the old table.
3. **`R_STRING`/`R_IMAGEDATA` dereference; `R_FRSTR`/`R_FRIMG` don't.** Codes 5/6
   return the pointed-to data; codes 15/16 return the address of the pointer
   slot (so `rsrc_saddr` can re-point a free string) (`gemrslib.c:159-170`).
4. **`rsrc_gaddr`/`rsrc_saddr` do not range-check the index.** Only the *type* is
   validated; a too-large index yields a pointer past the array
   (`gemrslib.c:102-106`) — the resource header's counts are the real bound.
5. **`rs_obfix` low byte is signed and width 80 is special.** `char == 80` in
   `ob_w`/`ob_x`-width position maps to `gl_width` (full screen); the low-byte
   offset >128 is treated as negative (`gemrslib.c:71-82`).
6. **Object relocation skips box `ob_spec`.** `G_BOX`/`G_IBOX`/`G_BOXCHAR` keep
   `ob_spec` as a colour word (not relocated); `G_CICON` `ob_spec` is an index
   turned into a CICONBLK pointer (`gemrslib.c:622-635`).
7. **New-format (CICON) files store the true length past `rsh_rssize`.** For
   `rsh_vrsn & NEW_FORMAT_RSC`, the real file size is a LONG at offset
   `rsh_rssize`, and an extension array (true length, CICON table offset) trails
   the classic data (`gemrslib.c:484-508,764-772`).
8. **Only one resource per application.** `ap_rscmem` holds a single blob;
   a second `rsrc_load` without `rsrc_free` leaks the first (`gemrslib.c:775`).
9. **File-selector directory read is deferred + retried once.** A bad path
   triggers `form_error` and one retry (reset to the previous or the drive
   root); empty/no-match directories are *not* errors (`gemfslib.c:330-336,
   730-754`).
10. **Folders sort first in the list.** Each entry is prefixed with `\007`
    (folder) or space (file) so the shell-sort puts directories at the top
    (`gemfslib.c:241-254,313-326`).
11. **`fsel_input`'s "OK" is refused after editing the mask.** If the user edits
    the path/mask field and presses OK, TOS (and EmuTOS) treat it as a *rescan*,
    not an exit — the button is deselected and the new mask is applied
    (`gemfslib.c:762-767`).

---

## Dependencies

### Requires (must be initialized / functional)

- **GEMDOS file I/O** — `rsrc_load` uses `Fopen`/`Fread`/`Fseek`/`Fclose` +
  `Malloc` (`dos_open`/`dos_read`/`dos_lseek`/`dos_close`/`dos_alloc_anyram`);
  `fsel_input` uses `Fsfirst`/`Fsnext` (`dos_sfirst`/`dos_snext`) and `Malloc`.
  All are implemented (Phase 08/09).
- **The shell path search** (`sh_find`, [`scrap-shell.md`](./scrap-shell.md)) —
  `rsrc_load` resolves the filename through it.
- **`graf_handle`** ([`window-graphics.md`](./window-graphics.md)) — `rs_obfix`
  needs `gl_wchar`/`gl_hchar`/`gl_width`, so the VDI workstation must be open.
- **The Object Manager + Form Manager** ([`object-form.md`](./object-form.md)) —
  `fsel_input` runs `fm_do` over the FSELECTR OBJECT tree and draws it with
  `objc_draw`; `fm_dial(FMD_START/FMD_FINISH)` reserves/redraws the screen.
- **The resource `gem.rsc`** — the FSELECTR dialog tree (drive buttons, the
  scrollable file box, OK/Cancel) is a built-in resource.

### Used by

- **Every GEM application** loads its UI via `rsrc_load` + `rsrc_gaddr`, and
  runs its Open/Save dialogs via `fsel_input`/`fsel_exinput`.
- **The NewTOS desktop** loads `DESK.RSC`/the GEM resource this way.
- `form_alert`/menus/windows all draw the OBJECT trees that `rsrc_load` produces.

---

## Related data structures

- **`RSHDR`** — the `.RSC` file header (§0, `rsdefs.h:16-36`). The byte layout
  and the offset/count semantics are **contract**; the on-disk offsets are
  big-endian (m68k).
- **`AESGLOBAL` resource fields** (`gemrslib.h:15-27`): `ap_ptree`
  (`OBJECT **`, the tree table), `ap_rscmem` (`RSHDR *`, the loaded blob),
  `ap_rsclen` (`UWORD`, its length). Filled by `rsrc_load`, read by the AES.
- **`OBJECT` / `TEDINFO` / `ICONBLK` / `BITBLK`** — the arrays the resource
  contains and `rsrc_gaddr` returns (see
  [`../structures/object.md`](../structures/object.md),
  [`tedinfo`](../structures/tedinfo.md),
  [`iconblk`](../structures/iconblk.md), [`bitblk`](../structures/bitblk.md)).
  `rs_obfix` fixes their coordinates; the relocation fixes their pointers.
- **FSELECTR dialog tree** (`gem.rsc`) — reference; the observable contract is
  the path/name split + the 1/0 button return, not the resource's object indices.

---

## Cross-references

### Atari Compendium / Profibuch

- **Not present in `references/`** (no PDFs). Cross-check of the `rsrc_*`/`fsel_*`
  opcode numbers, the `RSHDR` layout, the `R_*` type codes (0–16), the
  `rs_obfix` char→pixel rule, the `rsrc_gaddr` `addr_out` return, and the
  `fsel_*` `int_out[1]` button convention against Compendium/Profibuch is
  **deferred** until the PDFs are available. EmuTOS `1a7484ab` is authoritative
  per CLAUDE.md Rule 1. See D-054.

### AES managers

- [`scrap-shell.md`](./scrap-shell.md) — `rsrc_load` calls `shel_find`; the path
  search order lives there.
- [`object-form.md`](./object-form.md) — `rsrc_load` produces the OBJECT trees
  the object/form managers draw; `fsel_input` runs `form_do` over the FSELECTR
  tree.
- [`window-graphics.md`](./window-graphics.md) — `graf_handle` provides the cell
  sizes `rs_obfix` needs.
- [`_dispatcher.md`](./_dispatcher.md) — selector-200 call model; the
  `rsrc_gaddr`-only `addr_out[0]` copy-back.
- [`_state.md`](./_state.md) §6c — the loaded-resource `AESGLOBAL` fields.

### GEMDOS

- `Fopen`/`Fread`/`Fseek`/`Fclose`/`Malloc` (`rsrc_load`), `Fsfirst`/`Fsnext`
  (`fsel_input`) — the file/directory calls both use.

---

## Configuration / version notes

- **`CONF_WITH_COLOUR_ICONS`** (default **1**, `config.h:1267`): enables the
  `G_CICON`/CICONBLK fix-up + expansion + device-transform in `rsrc_load`
  (`fix_cicons`/`transform_all_cicons`) and the new-format (`NEW_FORMAT_RSC`)
  true-length read. With it off, only classic-format resources are handled and
  `G_CICON` objects are not fixed up. This is a **reference** path (NewTOS draws
  colour icons with its own rasteriser). **Note (D-100):** unlike the earlier
  wording here, the classic on-disk `RSHDR` byte format + its offset-table
  relocation are themselves **reference** for NewTOS too (ADR-013's native
  `.RSC`) — only the relocation's *shape* (offsets → pointers, box-`ob_spec`
  skipped) and the `rsrc_gaddr`/`rsrc_saddr`/`fsel_*` ABI are contract; see
  the intro deviation note.
- **`CONF_WITH_3D_OBJECTS`** (default **1**): `fs_start` sets FL3D\* flags and
  repositions the FSELECTR objects for the 3D look; otherwise it uses the flat
  layout (`gemfslib.c:59-172`). Reference (dialog appearance), not contract.
- `rsrc_load`/`rsrc_free`/`rsrc_gaddr`/`rsrc_saddr`/`rsrc_obfix` (110–114) and
  `fsel_input`/`fsel_exinput` (90–91) are **unconditional** across all EmuTOS
  builds and TOS versions.

---

## NewTOS implementation notes

- **(D-100, ADR-013 already decided — supersedes the older guidance below.)**
  NewTOS does **not** load EmuTOS's classic big-endian on-disk `.RSC`. It
  defines and loads its own **native** format (`NrscHeader`, magic `"NRSC"`,
  little-endian, 64-bit `ob_spec`, pixel coordinates —
  `tos-api/src/aes/resource.rs`), built from a classic `.rsc` by the host
  tool `tools/mkrsc.py convert`. Consequently: `rsrc_load` does **not** call
  `sh_find` — it tries a fixed `C:\GEMSYS\RSC\<name>` then `C:\<name>`;
  `rsrc_obfix` is an **identity** (no char→pixel fix — see the callouts under
  `rsrc_load (110)` and `rsrc_obfix (114)` above). What **is** preserved
  exactly: the offset→pointer relocation *shape* (byte offsets from blob
  base → absolute pointers, type-aware for `ob_spec` per object type), the
  box-`ob_spec`-not-relocated rule, and — as contract — the `rsrc_gaddr`/
  `rsrc_saddr` ABI slots, the `R_*` type semantics, and the `1`/`0` return
  convention. A classic-Atari-`.RSC` loader (the `RSHDR` byte layout, `-1L`
  sentinel, big-endian swap, char→pixel `rs_obfix`, `sh_find` lookup — all
  accurately extracted in §0–§3 above) remains a candidate for a future
  m68k-emulator compatibility phase, per ADR-013's own "alternatives
  rejected" note; it is not implemented today.
- **`R_*` type coverage (D-054, task #108).** `rsrc_gaddr`
  (`tos-api/src/aes/resource.rs`) now implements `R_TREE` (0), `R_OBJECT` (1)
  and the **free-string / free-image family** — `R_STRING` (5), `R_IMAGEDATA`
  (6), `R_FRSTR` (15), `R_FRIMG` (16), exactly per `get_addr` (§0): `R_FRSTR`/
  `R_FRIMG` return the *address of* the pointer slot (`&frstr[i]`/`&frimg[i]`),
  `R_STRING`/`R_IMAGEDATA` return the *dereferenced* data pointer
  (`frstr[i]`/`frimg[i]`), all bounds-checked against the resource's counts.
  This unblocked QED, whose `note()`/alert path fetches its free-string table
  via `rsrc_gaddr(R_FRSTR, NOWINDOW, &alertmsg)` (verified in QEMU: `R_FRSTR`
  now returns a real user VA and QED reaches its editor + menu bar, where the
  NULL table previously triggered a cflib `set_string()` panic). To carry the
  tables the native `.RSC` format grew a **free-string** and a **free-image**
  pointer table and a bumped version (v3 = LP64/44-byte header, v4 =
  ILP32/44-byte header); the loader stays **backward-compatible** — v1/v2
  files (GEM.RSC/DESK.RSC, no tables) load unchanged with those selectors
  reporting the honest "not found". Still-unimplemented type codes
  (`R_TEDINFO`…`R_BIPDATA`, `R_OBSPEC`, 2–4/7–14) remain the documented gap
  (fall through to `addr_out[0] = 0`, `int_out[0] = 0`); QED does not use them.
  `rsrc_saddr` currently always returns `0`/`FALSE` regardless of input — a
  **no-op stub** (CLAUDE.md Rule 2 concern; flagged for the lead, a separate
  task, **not** fixed here).
- **`rsrc_gaddr` is the only `addr_out` writer.** NewTOS's dispatcher must copy
  `addr_out[0]` back for opcode 112 and no other (matching EmuTOS `xif`); this
  is unaffected by the native-format change and confirmed against
  `tos-api/src/aes/mod.rs`'s opcode 90–91/110–114 routing.
- `fsel_input`/`fsel_exinput` load the **real EmuTOS `FSELECTR` tree**
  (converted `GEM.RSC`, see the GEM.RSC-lazy-load callout under
  `fsel_input (90)` above), not a NewTOS-authored dialog — so the FSELECTR
  object indices (`FSDIRECT`, `FSSELECT`, `F1NAME`…, `FSOK`/`FSCANCEL`, the
  drive-letter grid) are the ones `aes/gem.rsc` defines, hard-coded in
  `tos-api/src/aes/fsel.rs`. The **path/mask split, the wildcard filter, the
  folders-first sort, and the 1/0 (`int_out[1]`) button return** are
  contract; the dialog's resource tree and its rendering are EmuTOS-derived
  but reference-level (a NewTOS-authored `FSELECTR` tree would be equally
  conformant as long as the objects `fsel_input` needs — path field,
  selection field, name-list slots, OK/Cancel, drive grid — exist and this
  contract behaviour holds).
- Pointers in `addr_in[]` (the filename, the path/name buffers, the store
  address) are **user (application) pointers** — apply the
  [`_dispatcher.md`](./_dispatcher.md) user/kernel boundary handling; `fsel_*`
  writes back into `pipath`/`pisel`, so those must be flushed to user memory.
- The `.RSC` loader + `rsrc_gaddr` are what let recompiled GEM apps (and the
  file selector's own `GEM.RSC`) run against a real resource tree → high
  value for compatibility, even though the on-disk byte format itself is
  NewTOS-native rather than the classic Atari one.
