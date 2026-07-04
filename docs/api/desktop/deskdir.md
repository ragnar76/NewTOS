# `deskdir.c` — recursive directory copy/move/delete/rename engine

> Phase-00 extraction, Track A. Source: EmuTOS @ `1a7484ab`,
> `desk/deskdir.c` (1085 lines) + `desk/deskfpd.c` (353 lines, for the
> "Sortierung" part of this task — see the
> [naming note](#naming-note-sort_fnodes--d_doreread-do-not-exist) below).
> Per [`docs/api/desktop/_overview.md`](_overview.md): `GLOBES`/`WNODE`/
> `PNODE`/`FNODE` are defined in
> [`_data-model.md`](_data-model.md) — not re-derived here, only the
> *behaviour* new to this task (the shell-sort algorithm, the compare
> keys, and the copy/move/delete/rename state machines) is added.
>
> **Contract vs. internal** (`docs/COMPATIBILITY.md`): `deskdir.c` and the
> sort routines in `deskfpd.c` are 100% desktop-application-internal — no
> GEMDOS/AES/VDI call takes a `PNODE`/`FNODE`/`DIRCOUNT` as a parameter.
> What *is* contract is the **observable effect on the filesystem** (a
> program can later `Fsfirst` and see the results of a copy/move/delete/
> rename) and the **GEMDOS calls made along the way** (`Fcreate`, `Fdelete`,
> `Dcreate`, `Frename`, …, already spec'd under `docs/api/gemdos/`). The
> recursion strategy, the shell-sort, the fixed on-stack path buffers, and
> the dialog-driven skip/retry/abort/overwrite UI are free for NewTOS to
> replace outright, *as long as the end-to-end filesystem effect and the
> user-facing prompts/choices are preserved* (see
> [Cross-references](#cross-references)).

---

## Naming note: `sort_fnodes` / `d_doreread` do not exist

The task brief names `d_doreread`, `sort_fnodes`, and `dir_read` as things
to extract from `deskdir.c`. None of these identifiers exist anywhere in
`references/emutos/desk/` (verified by grep across all `.c`/`.h` files).
The actual functions matching that intent are:

| Requested name | Actual EmuTOS function | File |
|---|---|---|
| "FNODE-Liste füllen" / "`dir_read`" | `pn_active()` | `desk/deskfpd.c:227-298` |
| "`pn_open`/`pn_close`" | `pn_open()` / `pn_close()` | `desk/deskfpd.c:69-89` / `:59-63` |
| "Sortierung `sort_fnodes`/Sort-Keys" | `pn_sort()` / `pn_comp()` / `pn_fcomp()` | `desk/deskfpd.c:154-206` / `:138-147` / `:99-128` |
| "`d_doreread`" | *(no match)* — closest analogue is `d_doop(0, OP_COUNT, …)` (a **counting** pass, not a rebuild), or `rebuild_window()` in `deskfun.c:135-144` (calls `pn_active()` then `desk_verify()`) | `desk/deskdir.c:528-688` / `desk/deskfun.c:135-144` |

`pn_open`/`pn_close`/`pn_active` are **already structurally documented**
in [`_data-model.md`](_data-model.md#pnode--deskfpdh69-78) (the `PNODE`
section) — including the `f_attr..f_name[]`/`DTA` mirroring quirk. This
entry does not repeat that; it adds the **sort algorithm** (`pn_sort`/
`pn_comp`/`pn_fcomp`), which `_data-model.md` only mentioned in passing
("`pn_comp()`/`pn_fcomp()` … read `G.g_isort`"), plus the full
`deskdir.c` copy/move/delete/rename engine, which no prior extraction
task covered.

---

## EmuTOS source

**Primary locations:**
- `desk/deskdir.c:871-1084` — `dir_op(op, icontype, pspath, pdst_path,
  count)` — operates on the **selected** `FNODE`s of one `PNODE`
- `desk/deskdir.c:528-688` — `d_doop(level, op, psrc_path, pdst_path,
  tree, count)` — recursively processes **one entire subtree**
  (used both directly, for a selected subfolder, and by itself recursively)
- `desk/deskfpd.c:154-206` — `pn_sort(pn)` — sorts the `FNODE` chain
- `desk/deskfpd.c:138-147` — `pn_comp(pf1, pf2)` — folder-first + sort-key compare
- `desk/deskfpd.c:99-128` — `pn_fcomp(pf1, pf2, which)` — per-key compare

**Installed handler / vector:** `(none)` — these are plain C functions
called directly from `deskfun.c`'s `fun_op()`/`fun_mkdir()` (menu/drag
dispatch, see [`deskfun.md`](deskfun.md)) and from `deskwin.c`'s
`win_srtall()`/`pn_active()` call sites (see [`deskwin.md`](deskwin.md)).

**Helpers called (`deskdir.c`, all `static` unless noted):**
- `deskdir.c:60-73` `user_abort()` — polls for the UNDO key (via
  `dos_conis()`/`dos_rawcin()`) during a multi-file operation; if pressed,
  confirms via `fun_alert(1, STABORT)`
- `deskdir.c:79-85` `draw_dial(tree)` (non-`static`, `deskdir.h:26`) —
  centers + `objc_draw`s a dialog tree
- `deskdir.c:93-125` `do_namecon()` — shows the "name conflict" alert
  (`ADCPALER` resource tree) and its OK/Skip/Stop buttons, then
  re-displays the progress dialog underneath (`ADCPYDEL`) so both remain
  correctly painted once the conflict dialog closes
- `deskdir.c:131-139` `draw_fld(tree, obj)` (non-`static`, `deskdir.h:25`)
  — redraws a single dialog field (used to live-update the file/folder
  counters and progress fields)
- `deskdir.c:146-151` `add_path(path, new_name)` (non-`static`,
  `deskdir.h:29`) — appends `\new_name\*.*` to a path (descend one folder)
- `deskdir.c:157-168` `sub_path(path)` — strips the last folder segment
  and re-appends `\*.*` (ascend one folder, restoring the path after
  recursing into a subfolder)
- `deskdir.c:177-183` `add_fname(path, new_name)` (non-`static`,
  `deskdir.h:27`) — replaces the wildcard tail of a `…\*.*` path with a
  filename, **returns a pointer to where the name was written** (used to
  `set_all_files()`/restore the wildcard afterward)
- `deskdir.c:189-208` `item_exists(path, is_folder)` — existence check via
  a **private DTA** (`G.g_wdta`, swapped in/out around `dos_sfirst`) so it
  doesn't clobber whatever DTA the caller had active
- `deskdir.c:214-217` `del_fname(pstr)` (non-`static`, `deskdir.h:28`) —
  strips a filename back to `…\*.*` (calls `util/miscutil.c:52`
  `set_all_files()`)
- `deskdir.c:220-224` `illegal_op_msg()` (non-`static`, `deskdir.h:32`) —
  `fun_alert(1, STILLOP)`, always returns `FALSE`
- `deskdir.c:237-256` `d_dofdel(ppath)` — delete one file, skip/retry/abort
  loop around `dos_delete()`
- `deskdir.c:269-288` `d_dofoldel(ppath)` — delete one (now-empty) folder,
  same loop around `dos_rmdir()`
- `deskdir.c:299-382` `output_fname(psrc_file, pdst_file)` — resolves
  overwrite conflicts for one file copy (detailed below)
- `deskdir.c:394-503` `d_dofcopy(psrc_file, pdst_file, time, date, attr)`
  — the actual byte-copy loop for one file
- `deskdir.c:512-522` `update_modified_windows(path, length)` — after a
  folder delete/move, closes-to-root any open window whose path was
  inside the deleted/moved subtree (`fun_close(pwin, CLOSE_TO_ROOT)`, see
  [`deskfun.md`](deskfun.md#fun_close))
- `deskdir.c:700-723` `get_new_name(dstpth)` — prompts for a replacement
  folder/file name via the same `ADCPALER` conflict dialog
- `deskdir.c:734-779` `output_path(op, srcpth, dstpth)` — resolves
  destination-folder conflicts for copy/move/rename
- `deskdir.c:785-825` `d_dofileren(oldname, newname, is_folder)` — rename
  loop with the "replace target with a delete" special case (see
  [Edge cases](#edge-cases-discovered-in-source))
- `deskdir.c:831-841` `d_dofoldren(oldname, newname)` — strips the
  trailing `\*.*` wildcard from both names, then calls `d_dofileren`
- `deskdir.c:850-863` `source_is_parent(src, dst)` — string-prefix test
  (with `*`-as-wildcard-match-rest) used to detect "copying/moving a
  folder into itself or a descendant of itself"

**Helpers called (other files, cited — not re-derived here):**
- `deskfun.c:75-98` `fun_alert()`/`fun_alert_merge()` — alert dialogs (see
  [`deskfun.md`](deskfun.md))
- `desksupp.c:1840`/`:1849` `desk_busy_on()`/`desk_busy_off()` — hourglass
  cursor
- `deskrsrc.c` `desktop_str_addr()` — resource string lookup
- GEMDOS wrappers (`gemdos.h`): `dos_sfirst`/`dos_snext`, `dos_delete`,
  `dos_rmdir`, `dos_mkdir`, `dos_rename`, `dos_open`/`dos_create`/
  `dos_read`/`dos_write`/`dos_close`/`dos_setdt`, `dos_alloc_stram`/
  `dos_alloc_anyram`/`dos_free`/`dos_avail_stram` — the actual GEMDOS
  contract these ultimately call is spec'd under `docs/api/gemdos/`
- `util/miscutil.c:52` `set_all_files()`, `util/optimize.c:226`
  `filename_start()` — generic path-string helpers, not re-derived
- AES dialog primitives (`inf_gindex`, `inf_sset`/`inf_sget`, `form_do`,
  `form_center`, `form_dial`, `start_dialog`/`end_dialog`, `align_title`)
  — see [`deskinf.md`](deskinf.md) for `start_dialog`/`end_dialog`;
  the rest are AES-contract (`docs/api/aes/object-form.md`)

**Headers / type definitions used:**
- `desk/deskdir.h:16-20` — `DIRCOUNT { LONG files; LONG dirs; LONG size; }`
  — running counters shared between the counting pass and the doing pass
- `desk/deskfpd.h:22-25` — `OP_COUNT`=0, `OP_DELETE`=1, `OP_COPY`=2,
  `OP_MOVE`=3
- `desk/deskdir.c:49` — `OP_RENAME`=777 (deliberately far outside the
  `OP_*` range so it can't collide; **internal to `deskdir.c`/`deskfun.c`
  only** — not in `deskfpd.h`, not visible to other modules except via
  `dir_op()`'s `op` parameter)
- `desk/deskconf.h:44` — `MAX_LEVEL`=12 (recursion depth guard, see
  [Error conditions](#error-conditions))
- `desk/deskdir.c:47` — `MAX_CLUS_SIZE`=32×1024 bytes (copy-buffer sizing
  heuristic, see `dir_op`'s `OP_COPY`/`OP_MOVE` case)
- `include/sysconf.h:51-55` — `LEN_ZPATH`=114, `MAXPATHLEN`=`LEN_ZPATH+
  LEN_ZFNAME+1`
- `desk/deskbind.h:36,43` — `ALLFILES`=`FA_SUBDIR|FA_SYSTEM|FA_HIDDEN`,
  `DISPATTR`=`FA_SUBDIR`

**System variables / globals read:**
- `G.g_wdta` (`desk/deskbind.h`) — the desktop's private DTA, swapped in
  around `dos_sfirst`/`dos_snext` calls that must not disturb the
  caller's own DTA (`item_exists`, `d_doop`)
- `G.g_covwrpref`/`G.g_cdelepref`/`G.g_ccopypref` — "confirm overwrite" /
  "confirm delete" / "confirm copy" preference booleans (set via the
  Preferences dialog, `deskinf.c:inf_pref`) — gate whether `output_fname`
  prompts at all, and whether `dir_op`'s modal confirm dialog is shown

**System variables / globals written:**
- `G.g_nfiles`/`G.g_ndirs`/`G.g_size` (`desk/deskbind.h:158-160`) —
  running totals, written by both `d_doop`'s `OP_COUNT` case and
  `dir_op`'s own `OP_COUNT` case (for the top-level selected items) —
  read back into the caller's `DIRCOUNT*` at the end of `dir_op`
  (`deskdir.c:1061-1065`)

**Build configuration affecting this function:**
- `CONF_PREFER_STRAM_DISK_BUFFERS` (`config.h:1724-1732`, default =
  `CONF_ATARI_HARDWARE` — `1` on real Atari hardware for DMA-capable
  ST-RAM, `0` otherwise) — `dir_op`'s `OP_COPY`/`OP_MOVE` case
  (`deskdir.c:932-936`) allocates the shared copy buffer via
  `dos_alloc_stram()` if set, else `dos_alloc_anyram()`. On NewTOS
  (AArch64, no ST-RAM/DMA distinction) this collapses to plain
  allocation — **free implementation detail**, not contract.
- No other `#if CONF_WITH_*` gating in `deskdir.c` itself.

---

## Source extraction summary

`deskdir.c` is the desktop's file-operation *engine*: given a `PNODE`
(one open directory, with some subset of its `FNODE`s flagged
`f_selected`) and an operation code (`OP_DELETE`/`OP_COPY`/`OP_MOVE`, or
internally `OP_RENAME`/`OP_COUNT`), `dir_op()` walks the selected items
once, dispatching files to a per-file handler (`d_dofdel`/`d_dofcopy`/
`d_dofileren`) and folders to the recursive subtree walker `d_doop()`.
`d_doop()` re-implements the same per-file dispatch **for every item in
a folder**, recursing into subfolders (bounded by `MAX_LEVEL`=12) via
`dos_sfirst`/`dos_snext` directly (it doesn't build a `PNODE`/`FNODE`
list — no memory cost for large subtrees, at the price of a second,
parallel traversal loop from `dir_op`'s own top-level one). Every
operation has an `OP_COUNT` pre-pass (`fun_op()` in `deskfun.c` always
counts first) that populates `G.g_nfiles`/`g_ndirs`/`g_size`, both to
size the progress dialog and to detect a zero-item no-op early.

Conflict handling is dialog-driven and modal: a destination file that
already exists triggers `output_fname()`, which shows the "name
conflict" dialog (`ADCPALER`) pre-filled with the current source/dest
names, letting the user edit the destination name, skip this item, or
stop the whole operation; a destination *folder* that already exists
(when creating one during a recursive copy/move) triggers the analogous
`output_path()`/`get_new_name()` pair. Every filesystem call
(`dos_delete`/`dos_rmdir`/`dos_mkdir`/`dos_rename`/`dos_create`) that can
fail for a recoverable reason (locked file, full directory, …) prompts a
skip/retry/abort alert and loops on retry.

The path strings themselves are built by mutating a **single stack
buffer per level** in place (`add_path`/`sub_path`/`add_fname`/
`del_fname`), not by allocating a new string per recursion level — this
is why `d_doop`'s recursive call is immediately followed by
`sub_path()` to restore the caller's own path suffix (`deskdir.c:638-640`)
and why every per-file branch carefully restores the original `…\*.*`
suffix afterward (`set_all_files(ptmp)`, `deskdir.c:673`,`:1049`) — a
classic 1980s memory-frugal pattern that is free for NewTOS to replace
with per-level heap strings, *as long as recursion depth and path length
limits stay observably the same* (see
[Error conditions](#error-conditions)).

---

## `dir_op(op, icontype, pspath, pdst_path, count)` — top-level dispatch

`desk/deskdir.c:871-1084`.

1. **Move-to-same-drive optimisation** (`:885-889`): if `op==OP_MOVE` and
   the source/dest drive letters match, silently converts the operation
   to `OP_RENAME` (a `Frename` is far cheaper than copy+delete across the
   same volume; `Frename` also works across directories on the same
   drive — this is **not** a "same folder" check, purely "same drive
   letter").
2. **Progress dialog setup** (`:891-965`): for anything other than
   `OP_COUNT`, opens the `ADCPYDEL` dialog (title text set per operation:
   `STDELETE`/`STCOPY`/`STMOVE`), preloads the file/folder counts from
   the caller's `DIRCOUNT*`. If the relevant "confirm" preference is set
   (`G.g_cdelepref` for delete; `G.g_ccopypref`, OR'd with
   `G.g_cdelepref` for move, since a move deletes the source — `:939-945`
   — for copy/rename), runs a modal `form_do` and requires the OK button
   (not Cancel) to proceed (`tree[CDOK].ob_state & SELECTED`).
3. **Whole-disk-delete extra prompt** (`:971-976`): if deleting
   (`OP_DELETE`) and `icontype==AT_ISDISK` (the desktop disk icon itself
   was dropped on the trash, not a folder inside it), an **additional**
   `STDELDIS` alert is required regardless of the `g_cdelepref`
   preference — deleting an entire disk's contents gets a second
   confirmation no matter what.
4. **Main loop over selected `FNODE`s** (`:978-1057`): for each
   `fnode_is_selected(pf)` entry in `pspath->p_flist`
   (`deskfun.h:50-56` — currently just `f_selected`), checks
   `user_abort()` between items (except during `OP_COUNT`, which is
   assumed fast enough not to need a cancel check), then dispatches:
   - **Folder** (`pf->f_attr & FA_SUBDIR`): builds the descended source
     path (`add_path`), and for copy/move/rename, the descended dest
     path plus a **self-containment check** (`source_is_parent`,
     `:999-1004`) — copying/moving a folder into itself or one of its
     own descendants is rejected with `STILLDIR` (Skip continues to the
     next item; any other answer aborts the whole loop). Folder creation
     at the destination (`output_path`) may prompt for a new name if the
     destination folder already exists. `strcmp(srcpth,dstpth)==0` is a
     silent no-op (copy/move onto itself). Otherwise recurses via
     `d_dofoldren` (rename) or `d_doop(0, op, …)` (copy/move/delete/count).
   - **File** (`:1022-1049`): dispatches directly to `d_dofdel`/
     `d_dofcopy`/`d_dofileren`, restoring both source and dest path
     suffixes afterward. For `OP_MOVE`, the source is deleted **only if**
     the copy step returned `>0` (success) — a failed or skipped copy
     leaves the source alone.
5. **Post-loop finalisation** (`:1059-1082`): for `OP_COUNT`, copies
   `G.g_nfiles`/`g_ndirs`/`g_size` into the caller's `DIRCOUNT*`; for
   copy/move, frees the shared copy buffer; always resets the dialog's
   button states and calls `end_dialog()` (drains any AES redraw messages
   queued by the dialog's closing, per `deskinf.md`'s `end_dialog`).

Return value: `more` — `TRUE`/nonzero if the operation ran to completion
(possibly with some items skipped), `FALSE`/`0` if the user aborted
partway or a fatal error occurred (e.g. `dos_avail_stram()` too low to
allocate a copy buffer, `:911-917`, which shows `malloc_fail_alert()` and
returns immediately without any dialog).

---

## `d_doop(level, op, psrc_path, pdst_path, tree, count)` — recursive subtree walk

`desk/deskdir.c:528-688`.

Unlike `dir_op`, this does **not** consult any `FNODE`/selection state —
it walks `dos_sfirst(psrc_path, ALLFILES)`/`dos_snext()` directly with a
**private DTA allocated per recursion level** (`dos_alloc_anyram(sizeof
(DTA))`, `:541`, freed at `:685` — i.e. one DTA allocation per folder
being processed, alive only for that folder's scan+recursion). This
means `d_doop` processes **every** entry in the folder (subject to the
`ALLFILES` attribute mask, which includes hidden/system files, unlike
the desktop's normal display filtering), not just previously-selected
ones — it's used only to process the *full contents* of an
already-selected folder, never as an alternative to `pn_active()`'s
display-filtered scan.

Per-entry logic mirrors `dir_op`'s file/folder split (folder: recurse
via `add_path`+`d_doop(level+1,…)`, with `sub_path()` restoring both
source and dest paths afterward, `:638-640`; file: dispatch to
`d_dofdel`/`d_dofcopy`, `:658-671`). The `OP_COUNT` case additionally
increments `G.g_ndirs` once **per folder visited**, at the point the
folder's `dos_sfirst`/`dos_snext` loop terminates (`:568-569`) — i.e.
folder counts are counted "on the way out" (post-order), not "on the way
in", while file counts are incremented as each file is seen (pre-order,
`:654-656`). The progress dialog's live counters (`CDFOLDS`/`CDFILES`,
via `inf_numset`+`draw_fld`) are decremented (`--(count->dirs)`/
`--(count->files)`) as each item is processed — i.e. the dialog counts
*down* from the pre-counted total to zero, one `draw_fld` redraw per
item (a naive "one server round-trip per file" progress bar, no
batching/throttling).

**Recursion guard** (`:537-549`): before recursing, checks `level >
MAX_LEVEL` (12) — if exceeded, or if the per-level DTA allocation fails
(out of memory), issues `STFO8DEE` ("folders 8 deep" — the alert text
predates the current `MAX_LEVEL`=12 value but the guard itself is live)
and returns `FALSE` **without** touching the file/folder counters for
that level.

---

## Sort keys and directory-scan ordering (`deskfpd.c`)

> This section deepens `_data-model.md`'s `PNODE`/`FNODE` write-up with
> the actual sort **algorithm**, per the task's "Sortierung" ask (the
> named function `sort_fnodes` doesn't exist — see the
> [naming note](#naming-note-sort_fnodes--d_doreread-do-not-exist)).

### `pn_comp(pf1, pf2)` — `deskfpd.c:138-147`

The master compare function `pn_sort()` uses. Folders sort before files
**unless** the current sort mode is "no sort" (`S_NSRT`):

```
if (G.g_isort != S_NSRT)
    if ((pf1->f_attr ^ pf2->f_attr) & FA_SUBDIR)
        return (pf1->f_attr & FA_SUBDIR) ? -1 : 1;   /* folder first */
return pn_fcomp(pf1, pf2, G.g_isort);
```

I.e. the folder-first rule is **not** itself a sort key choice — it's an
unconditional pre-check applied on top of whatever `G.g_isort` is,
except when `g_isort==S_NSRT`, in which case folders and files interleave
in raw directory-scan order.

### `pn_fcomp(pf1, pf2, which)` — `deskfpd.c:99-128`

The actual per-key comparison, `which` = `G.g_isort` (`deskfpd.h:38-43`):

| `which` | Compares | Tiebreak | Notes |
|---|---|---|---|
| `S_DATE`=2 | `f_date` descending (`pf2-pf1`) | `f_time` descending, then name | **newest first** |
| `S_SIZE`=3 | `f_size` descending (`pf2-pf1`) | name | **largest first** |
| `S_TYPE`=1 | `strcmp(scasb(name1,'.'), scasb(name2,'.'))` | name | compares from the **first** `.` inclusive. **Corrected 2026-07-04 (Audit #10 vs `util/optimopt.S:19-38`):** `scasb(str,c)` returns the pointer to the **first** occurrence of `c`, or to the terminating `\0` (an **empty** string) when `c` is absent — so the key is `".EXT"` (leading dot included) for a name with a dot, and `""` for a name with **no** dot. Extensionless files (`README`, `MAKEFILE`) therefore sort **before** all dotted files (immediately after folders), **not** by their whole name. The earlier "last dot / whole name" reading was wrong — irrelevant for single-dot 8.3 names but wrong for extensionless files. Folders are already split out by `pn_comp`. |
| `S_NSRT`=4 | `f_seq - g_seq` (ascending) | *(none — sequence numbers are unique)* | raw pre-sort directory order, i.e. exactly what `Fsfirst`/`Fsnext` returned |
| `S_NAME`=0 | *(falls through — `chk` stays `0`)* | `strcmp(name1,name2)` | name is **always** the final tiebreak for every key above too — the `if (chk) return chk; return strcmp(ps1,ps2);` structure at `:124-127` means `S_NAME` alone just skips straight to the name compare |

All string compares are plain `strcmp` on `f_name` (case-**sensitive** —
EmuTOS/TOS filenames are stored upper-case by convention but `pn_fcomp`
itself does no case-folding).

### `pn_sort(pn)` — `deskfpd.c:154-206`

1. **Short-circuit** (`:161-162`): `pn->p_count < 2` → already sorted,
   return the existing `p_flist` unchanged (no allocation, no-op).
2. **Build an index array** (`:167-175`): `dos_alloc_anyram(p_count *
   sizeof(FNODE*))`; on allocation failure, issues `malloc_fail_alert()`
   and **returns the list unsorted** (`:170-171`) — a sort that silently
   doesn't happen under memory pressure, not a crash.
3. **Shell sort** (`:177-191`) — explicitly cites "K&R page 108" in the
   source comment; gap sequence `count/2, /2, /2, …` down to `1`,
   standard insertion-sort-with-gap. `pn_comp(ml_pfndx[j], ml_pfndx[j+gap])
   <= 0` is the "already in order" break condition, i.e. **stable for
   equal keys under this comparator** only insofar as shell sort with
   this gap sequence happens to be (shell sort is not a stable sort in
   general — ties broken purely by the comparator's own tiebreak chain,
   never by original position, **except** for `S_NSRT` where the
   comparator *is* the original position).
4. **Re-link the chain** (`:193-201`) from the sorted index array back
   into a singly-linked `f_next` chain, then frees the index array
   (`:203`) — the `FNODE` structs themselves are never moved or copied,
   only re-chained.

**Contract:** none of the above (sort algorithm, tiebreak order,
allocation-failure fallback) is contract — `docs/api/desktop/_data-model.md`
already establishes that `FNODE`/`PNODE` never cross an ABI boundary.
What a program (or a human) *can* observe is the **resulting display
order** for a given `View > Sort by …` menu selection, which NewTOS must
reproduce (folder-first + the five key semantics in the table above) —
that ordering is the de-facto contract here, not the shell-sort mechanism
that produces it.

---

## Edge cases discovered in source

- **`OP_RENAME` is a silent, undocumented alias for move-on-same-drive**
  (`deskdir.c:885-889`): a user "moving" a file/folder within the same
  drive never sees a copy+delete — the desktop detects same-first-letter
  paths and substitutes `Frename`. This changes observable timestamps
  differently than copy+delete would (rename preserves the original
  file's timestamp/attributes entirely; a copy+delete via `d_dofcopy`
  explicitly re-sets `dos_setdt` from the source's date/time, so the two
  paths *should* end up equivalent from a program's viewpoint — but only
  because `d_dofcopy` takes care to copy the timestamp; this is worth a
  regression test).
- **`d_dofileren`'s self-replace-by-delete special case**
  (`deskdir.c:820-821`): if a rename target already exists and the user
  provides a *different* name than the original destination but then
  **doesn't change it again** on a subsequent retry (`ret==1` — "OK,
  unchanged" — after having *just* changed it), the code silently
  `dos_delete()`s the pre-existing file at the new name before retrying
  the rename. This only happens because `oldname != newname` at that
  point (comment `:812-819` explains: "user wants to move a file to
  replace an existing file… makes no sense when oldname==newname" — so
  the delete is skipped when there was no rename in progress at all).
- **Whole-disk delete requires a *second*, preference-independent
  confirmation** (`deskdir.c:971-976`) — `STDELDIS`, shown even if
  "confirm delete" is turned off in Preferences.
- **`d_doop`'s per-level DTA allocation can fail independently of the
  overall operation succeeding so far** (`deskdir.c:541-549`): a deeply
  nested folder structure can exhaust memory mid-operation (not just at
  the start), aborting only the affected subtree with `STFO8DEE`, not the
  whole `dir_op` — items already processed at shallower levels are not
  rolled back.
- **Folder counts in the progress dialog are counted post-order, file
  counts pre-order** (`deskdir.c:568-569` vs `:654-656`) — an
  implementation detail that affects exactly what number the live
  progress counters show mid-operation, not the final total.
- **`pn_sort`'s allocation failure degrades gracefully to "unsorted"**
  rather than failing the caller (`deskfpd.c:168-172`) — the window still
  displays, just in whatever order `pn_active()` last chained the
  `FNODE`s (i.e. raw `Fsfirst`/`Fsnext` order).
- **`S_NSRT`'s tiebreak *is* the primary key** (`deskfpd.c:120-122`): note
  that for every other sort key, ties fall back to name; for "no sort",
  the "tiebreak" (`f_seq`) is used as the *only* signal — there is no
  further tiebreak because `f_seq` values are unique per directory scan.

---

## Error conditions

| Condition | Source location | Result |
|---|---|---|
| Recursion depth > `MAX_LEVEL` (12) | `deskdir.c:537-538` | `STFO8DEE` alert, `d_doop` returns `FALSE` for that subtree |
| Per-level DTA `dos_alloc_anyram` fails | `deskdir.c:541-544` | same as above |
| `dos_avail_stram()` insufficient for copy buffer (`OP_COPY`/`OP_MOVE`) | `deskdir.c:911-917` | `malloc_fail_alert()`, `dir_op` returns `FALSE` immediately, no dialog shown |
| Destination folder can't be created (not "already exists") | `deskdir.c:756-757` | `illegal_op_msg()` (`STILLOP` alert), `output_path` returns `0` (stop) |
| Copying/moving a folder into itself or a descendant | `deskdir.c:999-1004` | `STILLDIR` alert; Skip → continue loop, else → abort |
| Per-file GEMDOS error (`dos_delete`/`dos_rmdir`/`dos_create`/`dos_open`/`dos_rename` all return `<0`) | `deskdir.c:241-256` etc. | skip/retry/abort alert (`STDELFIL`/`STDELDIR`/`STCRTFIL`/`STOPFAIL`/generic rename failure), looped on retry |
| Write shorter than read during copy (disk full) | `deskdir.c:467-472` | `STDISKFU` alert, partial destination file deleted (`dos_delete(pdst_file)`, `:497-498`), `dir_op`/`d_doop` treat as failure |
| User presses UNDO mid-operation | `deskdir.c:60-73`, checked at `:609-614`/`:983-984` | `STABORT` confirm; if confirmed, current loop breaks (partial completion, no rollback) |

---

## Verification criteria

- **Nominal copy**: selecting one file and choosing Copy to a folder with
  no name conflict results in exactly one `dos_open`+`dos_create`+
  `dos_read`/`dos_write` loop + `dos_setdt` call, no dialog prompts
  beyond the (optional, preference-gated) progress dialog, and the
  source file untouched.
- **Nominal move, same drive**: `dir_op(OP_MOVE, …)` with
  `psrc_path[0]==pdst_path[0]` is silently converted to `OP_RENAME`
  (`deskdir.c:887`) — verify via a single `dos_rename` call, no
  `dos_open`/`dos_create`/`dos_read`/`dos_write` sequence at all.
  Cross-drive move must instead perform copy-then-delete-on-success.
  Cross-reference: `docs/api/gemdos/` `Frename`/`Fdelete`/`Fcreate`.
- **Sort order**: given a folder containing one subfolder and files with
  distinct names/sizes/dates, `View > Sort by Size` must show the
  subfolder first (unless "no sort"), then files largest-to-smallest,
  ties broken by name ascending (`pn_fcomp`, `S_SIZE` case).
- **Recursion guard**: a folder tree 13 levels deep triggers `STFO8DEE`
  for the 13th level's contents; levels 1-12 must still have completed
  their own file operations before the guard fires (no rollback).
- **Self-copy protection**: dragging a folder onto itself, or into one of
  its own subfolders, must produce `STILLDIR` and perform no filesystem
  mutation for that item (verified via `source_is_parent`, prefix compare
  against the wildcard-terminated source path).

---

## Dependencies

### Requires (must be initialized / functional)
- GEMDOS filesystem calls (`Fsfirst`/`Fsnext`/`Fdelete`/`Dcreate`/
  `Drename`/`Fcreate`/`Fopen`/`Fread`/`Fwrite`/`Fclose`/`Fdatime`) — see
  `docs/api/gemdos/`
- `G.g_wdta` initialised (desktop's private DTA slot)
- AES dialog primitives (`form_do`/`form_alert`/`form_dial`) — see
  `docs/api/aes/object-form.md`
- `desk_rs_trees[ADCPALER]`/`[ADCPYDEL]` resource trees (desktop-internal
  UI layout, not contract — `desktop.def`/`.rsc`, generated at build time,
  not present as source in this checkout)

### Used by
- `deskfun.c:782-803` `fun_op(op, icontype, pspath, pdest)` — the single
  entry point every menu/drag-and-drop file operation goes through (see
  [`deskfun.md`](deskfun.md)); always runs `dir_op(OP_COUNT,…)` first,
  then `dir_op(op,…)` if the count was nonzero
- `deskinf.c:476-481` `count_ffs(path)` — calls `d_doop(0, OP_COUNT, …)`
  directly (not through `dir_op`) to count a folder's contents for the
  "Show Info" dialog (see [`deskinf.md`](deskinf.md))
- `deskwin.c` `win_srtall()` calls `pn_sort()` per open window (see
  [`deskwin.md`](deskwin.md))
- `deskfpd.c:288` `pn_active()` calls `pn_sort()` itself after every
  directory scan (see `_data-model.md`)

## Related data structures

- `docs/api/desktop/_data-model.md` — `PNODE`/`FNODE`/`WNODE` field
  layout (this entry only adds the sort algorithm on top)
- `docs/api/structures/dta.md` — the `DTA` fields `d_doop`'s per-level
  scan reads directly (via its own private `DTA*`, not `G.g_wdta`)

## Cross-references

### Atari Compendium / Profibuch
Not consulted for this entry — the copy/move/delete/rename **UI flow**
(skip/retry/abort, overwrite prompts, recursion depth limit) is not part
of the documented GEMDOS/AES contract; it is desktop-application
behaviour, cross-checked only against the EmuTOS source per
`docs/COMPATIBILITY.md`'s "EmuTOS is the reference, not the template"
principle. A NewTOS desktop must produce the *same net filesystem effect*
and *broadly equivalent* user prompts (skip/retry/abort/overwrite/rename,
whole-disk-delete extra confirmation, self-copy protection,
recursion-depth guard) — the exact dialog text, layout, and internal
algorithm are free.

## Configuration / version notes

- `CONF_PREFER_STRAM_DISK_BUFFERS` — see
  [Build configuration](#emutos-source) above; irrelevant on NewTOS/
  AArch64 (no ST-RAM/DMA distinction), collapses to plain heap allocation.
- No TOS-version-specific behaviour identified in this file (it is
  EmuDesk-specific, not present in the same form in any original Atari
  TOS ROM desktop, which had its own, less-recoverable copy engine per
  Profibuch's general description — not independently verified here).

---

## NewTOS implementation notes

(Empty during Phase 00. Filled in when the corresponding implementation
phase begins.)
