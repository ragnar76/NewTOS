# `deskfun.c` — desktop function dispatch: alerts, drag-and-drop, delete, rebuild

> Phase-00 extraction, Track A. Source: EmuTOS @ `1a7484ab`,
> `desk/deskfun.c` (1447 lines). Per
> [`docs/api/desktop/_overview.md`](_overview.md) and
> [`_data-model.md`](_data-model.md) (`ANODE`/`FNODE`/`WNODE`/`PNODE` —
> not re-derived here). Builds directly on
> [`deskdir.md`](deskdir.md) (`dir_op`/`fun_op`'s engine) and is itself a
> dependency of [`deskact.md`](deskact.md) (drag release calls
> `fun_drag`) and `deskmain.md` (menu items call `fun_del`/`fun_mkdir`/
> `fun_search`/…).
>
> **Contract vs. internal** (`docs/COMPATIBILITY.md`): `deskfun.c` is
> desktop-application-internal glue — no function here is called via a
> GEMDOS/AES/VDI trap. What *is* contract is the same as in
> [`deskdir.md`](deskdir.md#emutos-source): the filesystem side effects
> (which GEMDOS calls happen, in what order, with what error handling)
> and, loosely, the observable UI behaviour (what a drag-and-drop of a
> given icon combination does). The dispatch tables, `ANODE`/`FNODE`
> lookups, and internal helper names are free.

---

## EmuTOS source

**Primary locations (public, `deskfun.h`):**
- `deskfun.c:75-78` `fun_alert(defbut, stnum)` / `:88-98`
  `fun_alert_merge(defbut, stnum, ...)` — alert dialog wrappers
- `deskfun.c:101-113` `fun_msg(type, w3, w4, w5, w6, w7)` — synthesize an
  internal AES-shaped message
- `deskfun.c:560-602` `fun_search(pw, curr)` (`CONF_WITH_SEARCH`)
- `deskfun.c:607-631` `fun_selectall(pw)` (`CONF_WITH_SELECTALL`)
- `deskfun.c:639-673` `fun_mask(pw)` (`CONF_WITH_FILEMASK`)
- `deskfun.c:206-226` `add_one_level(pathname, folder)`
- `deskfun.c:862-893` `fun_close(pw, closetype)`
- `deskfun.c:1302-1339` `fun_drag(wh, dest_wh, sobj, dobj, mx, my, keystate)`
- `deskfun.c:119-129` `fun_mark_for_rebld(path)` / `:150-167`
  `fun_rebld_marked(void)` / `:173-190` `fun_rebld(ptst)`
- `deskfun.c:680-736` `fun_mkdir(pw_node)`
- `deskfun.c:782-803` `fun_op(op, icontype_src, pspath, pdest)`
- `deskfun.c:1376-1424` `fun_del(pw, sobj)`
- `deskfun.c:1433-1446` `wants_to_delete_files(void)`

**Installed handler / vector:** `(none)` — called directly from
`deskmain.c`'s menu dispatchers (`do_filemenu`, see `deskmain.md`) and
from `deskact.c`'s drag-release handler (see [`deskact.md`](deskact.md)).

**Helpers called (this file, `static`):**
- `deskfun.c:135-144` `rebuild_window(pwin)` — `pn_active` + `desk_verify`
  + `win_sinfo` + a synthesized `WM_REDRAW`
- `deskfun.c:236-250` `remove_one_level(pathname)` — inverse of
  `add_one_level`
- Search subsystem (`CONF_WITH_SEARCH`): `deskfun.c:264-311` `convert()`
  (name → wildcard), `:319-347` `search_prompt()`, `:357-385`
  `mark_matching_fnodes()`, `:391-433` `search_display()`, `:441-504`
  `search_recursive()`, `:512-554` `search_icon()`
- `deskfun.c:746-774` `fun_print(sobj, bufsize, iobuf)`
  (`CONF_WITH_PRINTER_ICON`) — calls `print_file()` (`desksupp.c`, out of
  scope)
- `deskfun.c:809-825` `w_setpath(pw, pathname)`, `:828-856`
  `fun_full_close(pw)` — the two `fun_close` sub-cases
- `deskfun.c:903-928` `build_selected_path(pn, pathname)` — first
  selected `FNODE`'s path, filename-only or full depending on
  `G.g_fullpath` (`CONF_WITH_DESKTOP_CONFIG`)
- The drag-and-drop dispatch tree (all `static`, detailed
  [below](#fun_drag--the-drag-and-drop-dispatch-tree)): `fun_win2win`
  (`:942-1001`), `fun_file2desk` (`:1004-1091`), `fun_file2win`
  (`:1094-1114`), `fun_win2desk` (`:1117-1132`), `fun_file2any`
  (`:1135-1195`), `fun_desk2win` (`:1198-1257`), `fun_desk2desk`
  (`:1260-1299`)
- `deskfun.c:1349-1371` `delete_ffd(path, icontype)` — delete-everything-
  in-a-folder-or-disk helper for `fun_del`'s desktop-icon case

**Helpers called (other files, cited — not re-derived here):**
- `deskdir.c:871` `dir_op()` (see [`deskdir.md`](deskdir.md)) — via
  `fun_op()`
- `deskwin.c` `win_find`/`win_alloc`/`win_isel`/`win_sinfo`/`win_dispfile`
  (see [`deskwin.md`](deskwin.md))
- `deskapp.c` `app_afind_by_id()`/`app_afind_by_name()` (`ANODE` lookup —
  see `deskapp.md`)
- `desksupp.c` `desk_verify`/`desk_clear`/`do_wredraw`/`do_diropen`/
  `do_aopen`/`i_find`/`refresh_drive`/`valid_drive`/`build_root_path` —
  **`do_aopen`/`do_open` themselves are not yet extracted anywhere**
  (added 2026-07-21 — see "Missing coverage" under
  [Dependencies](#dependencies) below); what *is* now covered is the
  next link in their call chain, `deskpro.c`'s `pro_run`/`pro_exec`, see
  [`deskpro.md`](deskpro.md) (new, 2026-07-21)
- `deskins.c:146-184` `snap_icon()` (see [`deskins.md`](deskins.md)) —
  not called directly by `deskfun.c`, only by `deskact.c`'s drag code,
  but `fun_drag`'s `ins_shortcut` branch (`CONF_WITH_DESKTOP_SHORTCUTS`)
  is in `deskins.c` too
- `deskinf.c` `set_tedinfo_name`/`inf_sset`/`inf_sget`/`inf_show`/
  `inf_what` (dialog helpers, see [`deskinf.md`](deskinf.md))

**Headers / type definitions used:**
- `deskfun.h:16-18` — `CLOSE_FOLDER`=0, `CLOSE_WINDOW`=1,
  `CLOSE_TO_ROOT`=2 (the `closetype` argument to `fun_close`)
- `deskfun.h:50-56` — `fnode_is_selected(fn)` inline, currently just
  `return fn->f_selected;` (a trivial wrapper — the header comment
  implies room for a more complex "is selected" test in the future,
  e.g. filtering by filemask, that hasn't been added yet)
- `deskfpd.h:22-25` — `OP_COUNT`/`OP_DELETE`/`OP_COPY`/`OP_MOVE` (see
  [`deskdir.md`](deskdir.md))
- `deskapp.h` — `ANODE`, `AT_ISFILE`/`AT_ISFOLD`/`AT_ISDISK`/`AT_ISTRSH`/
  `AT_ISPRNT`, `AF_ISDESK`

**System variables / globals read:**
- `G.g_cwin`/`G.g_croot` — current window handle / object-tree root (used
  throughout to distinguish "acting on the desktop" from "acting in a
  window")
- `G.g_screen` — the `OBJECT` array (`win_isel` walks it; `ob_spec` is
  read directly for disk-icon drive letters, `deskfun.c:1043,1157`)
- `G.g_fullpath` (`CONF_WITH_DESKTOP_CONFIG`) — whether
  `build_selected_path` returns a bare filename or a full path

**System variables / globals written:**
- `exit_desktop` (`deskfun.c:61`, file-`static`) — set `TRUE` by
  `fun_file2desk`/`fun_win2desk`'s `do_aopen()` success path when a
  dropped file launches a program; `fun_drag`'s return value, which
  callers (`deskact.c`) use to end the desktop process and let the
  launched program run
- `fnodes_found`/`search_window` (`CONF_WITH_SEARCH`, file-`static`) —
  search-session state

**Build configuration affecting this function:**
- `CONF_WITH_SEARCH` — gates the entire Search feature
  (`fun_search`/`search_*`)
- `CONF_WITH_SELECTALL` — gates `fun_selectall`
- `CONF_WITH_FILEMASK` — gates `fun_mask`
- `CONF_WITH_PRINTER_ICON` — gates `fun_print` and the `AT_ISPRNT`
  dispatch case in `fun_file2desk`
- `CONF_WITH_DESKTOP_SHORTCUTS` — gates the desktop-shortcut cases in
  `fun_file2desk`/`fun_win2desk`, and `fun_drag`'s
  `ins_shortcut`/`STNODRA1` branch on drop-to-empty-desktop
- `CONF_WITH_DESKTOP_CONFIG` — gates `G.g_fullpath`'s use in
  `build_selected_path`
- All default **on** in the general/Atari build (`config.h`, same
  section as `_overview.md`'s config-variant note); forced off only under
  `TARGET_192`.

---

## Source extraction summary

`deskfun.c` is the desktop's *action layer*: every File-menu command and
every drag-and-drop gesture ends up here. It has three largely
independent groups of functionality:

1. **Alert/message plumbing** (`fun_alert`, `fun_alert_merge`, `fun_msg`)
   — thin wrappers other modules use pervasively; `fun_msg` in
   particular is how the desktop injects a synthetic AES message into
   its *own* message handler (`hndl_msg()`, `deskmain.c`) without going
   through the AES at all — see [Edge cases](#edge-cases-discovered-in-source).
2. **Window rebuild tracking** (`fun_mark_for_rebld`/`fun_rebld_marked`/
   `fun_rebld`) — after a filesystem-mutating operation, any open window
   whose path matches the affected directory needs its `FNODE` list
   re-scanned and redrawn; these three functions are the "which windows,
   when" bookkeeping for that.
3. **The drag-and-drop dispatch tree** (`fun_drag` and its seven
   `fun_*2*` helpers) — the largest and most intricate part of the file,
   covering all four source/destination combinations (window↔window,
   window↔desktop, desktop↔window, desktop↔desktop) each with further
   sub-dispatch on the destination *icon type* (file/folder/disk/trash/
   printer/application).

`fun_op`/`fun_del`/`fun_mkdir` are the File-menu-facing entry points that
ultimately call into [`deskdir.md`](deskdir.md)'s `dir_op()` engine.

**Gap flagged 2026-07-21 (re-validation):** the "three groups" above is
not exhaustive — the `CONF_WITH_SEARCH`/`CONF_WITH_SELECTALL`/
`CONF_WITH_FILEMASK`/`CONF_WITH_PRINTER_ICON` functions (`fun_search` and
its six static helpers `convert`/`search_prompt`/`mark_matching_fnodes`/
`search_display`/`search_recursive`/`search_icon`; `fun_selectall`;
`fun_mask`; `fun_print`) are correctly **located and cited** (file:line,
[Helpers called](#emutos-source) above, all verified against source this
task) but have **no narrative section** of their own here, unlike
`fun_drag`/`fun_op`/`fun_mkdir`. Per `_rebuild-plan.md` §8, Search/
Select-All/Set-mask/Printer-icon are all **"In"** for the 1:1 rebuild, so
a NewTOS `act.rs`/`dir.rs`/`inf.rs` implementer will need a deeper pass on
these six-plus functions before implementing them — this doc gives
correct entry points, not yet a full behavioural narrative for that
subsystem. Flagged rather than rushed, per protocol.

---

## `fun_drag` — the drag-and-drop dispatch tree

`deskfun.c:1302-1339`. Called once per completed drag gesture (from
`deskact.c`'s `act_bdown`, see [`deskact.md`](deskact.md)), with `wh` =
source window handle (`DESKWH` for the desktop), `dest_wh` = destination
window handle (`0` if dropped on the desktop), `sobj`/`dobj` = source/
destination object indices, `keystate` = modifier keys at drop time
(`MODE_CTRL` toggles Copy↔Move, tested throughout).

```
fun_drag(wh, dest_wh, sobj, dobj, mx, my, keystate):
    if wh != DESKWH:                      # dragging FROM a window
        if dest_wh:        fun_win2win(wh, dest_wh, dobj, keystate)
        elif dobj==DROOT:  ins_shortcut(wh, mx, my)   # CONF_WITH_DESKTOP_SHORTCUTS
                       else fun_alert(1, STNODRA1)     # else: "can't drag here"
        else:              fun_win2desk(wh, dobj, keystate)
    else:                                  # dragging FROM the desktop
        if dest_wh:        fun_desk2win(dest_wh, dobj, keystate)
        else:              fun_desk2desk(dobj, keystate)
    return exit_desktop
```

- **`fun_win2win`** (`:942-1001`): copy/move between two windows (never
  called with either being the desktop). Resolves the destination
  `ANODE` at `dst_ob` via `i_find`; if it's an **application** icon
  (`a_aicon>=0`), this is a launch-via-drop, not a copy — builds the
  selected source path and calls `do_aopen()`, setting `exit_desktop` on
  success and returning **without** any copy/move (`:962-969`).
  Otherwise resolves the destination path (into the target folder if the
  drop target is itself a folder icon, `:982-983`) and calls
  `fun_op(op, -1, &psw->w_pnode, destpath)`. On success, clears the
  source window's selection if source≠dest window, rebuilds the source
  window on move, always rebuilds the destination window, and — if the
  drop target was a folder — **also** rebuilds any other open window
  showing that folder's path (`fun_rebld(destpath)`, `:998-999`).
- **`fun_file2desk`** (`:1004-1091`): drop **onto a desktop icon**
  (`an_dest` may be `NULL` for "drop-to-empty" callers, though `fun_drag`
  never calls it that way — only `fun_win2desk`/`fun_desk2desk` do, both
  with a resolved `an_dest`). Switches on `an_dest->a_type`:
  - `AT_ISFILE` (`CONF_WITH_DESKTOP_SHORTCUTS`): only acts if the target
    file is itself a program (`a_aicon>=0`) — launches it with the
    dragged item's path as the tail argument via `do_aopen`.
  - `AT_ISFOLD` (`CONF_WITH_DESKTOP_SHORTCUTS`): sets `operation` =
    Move/Copy per `keystate`, destination = the shortcut's folder path.
  - `AT_ISDISK`: reads the **drive letter directly out of the target's
    live `ICONBLK`** (`(ICONBLK*)G.g_screen[dobj].ob_spec`, then
    `LOBYTE(dicon->ib_char)`, `:1043-1044`) rather than from the `ANODE`
    — the drive letter displayed *is* the letter used, even if it
    diverged from `a_letter` somehow (it shouldn't, but this is the
    actual code path).
  - `AT_ISTRSH`: prompts `wants_to_delete_files()` (**only** if
    `icontype_src>=0`, i.e. the *source* was a desktop icon too — a
    window-to-trash drag's confirmation instead happens inside `fun_op`/
    `dir_op`'s own delete-confirm preference, not here) — the desktop
    trash icon has no per-drive-letter target, `pathname[0]` is set from
    `pn_src->p_spec[0]` (the source's own drive letter).
  - `AT_ISPRNT` (`CONF_WITH_PRINTER_ICON`): allocates a 16 KB print
    buffer, then **loops over every currently-selected desktop icon**
    (`win_isel(G.g_screen, G.g_croot, sobj)`, `:1065`) calling
    `fun_print` for each — i.e. dropping *any* selected icon onto the
    printer prints **all** currently-selected icons, not just the one
    dragged (a deliberate multi-select convenience, matching Atari TOS
    per the surrounding logic's structure — not independently verified
    against Compendium).
  - default: `illegal_op_msg()` ("can't happen" case, e.g. dropping onto
    another disk icon goes through `AT_ISDISK` above, not here — this
    default only fires for icon types with no drop handling at all).
  If an `operation` was set, calls `fun_op` and, on success, rebuilds
  windows matching the destination path.
- **`fun_file2win`** (`:1094-1114`): drop a desktop-originated file set
  into a window — always `OP_COPY` (dropping a desktop icon into a
  window never moves/deletes the desktop original), destination is
  either the dropped-on folder (if `an_dest->a_type==AT_ISFOLD`) or the
  window's own path.
- **`fun_win2desk`** (`:1117-1132`): resolves the destination `ANODE` and
  source `WNODE`, calls `fun_file2desk`, rebuilds the source window on
  success.
- **`fun_file2any`** (`:1135-1195`): the common core both
  `fun_desk2win`/`fun_desk2desk` route through for a *single* dragged
  desktop icon — resolves its full path (from `a_pappl` for a
  file/folder shortcut, or synthesizes `X:\*.*` from the icon's own
  `ICONBLK` drive-letter char for a disk icon, `:1157-1159`), opens it as
  a fresh `PNODE` (`pn_open`+`pn_active`, **not** an already-open
  window's `PNODE` — a disk-icon drag builds a throwaway directory scan
  just for this operation), marks **every** entry in it selected
  (`:1170-1172` — dragging a disk/folder icon always means "all its
  contents"), then dispatches to `fun_file2win` or `fun_file2desk`
  depending on whether there's a destination window. **Orphan desktop
  shortcuts** (`CONF_WITH_DESKTOP_SHORTCUTS`, `p_flist==NULL` after the
  scan — the shortcut's target no longer exists) trigger
  `remove_locate_shortcut(sobj)` instead (prompts the user, not traced
  further — `desksupp.c`, out of scope).
- **`fun_desk2win`** (`:1198-1257`): drag **from** the desktop **to** a
  window. Iterates every selected desktop icon (`win_isel(…, DROOT,
  sobj)`), skipping non-existent-drive disk icons and trash/printer
  icons entirely (`continue`, `:1219-1227` — you cannot drag the trash
  or printer icon itself anywhere). If the destination is an
  application icon in the window (`CONF_WITH_DESKTOP_SHORTCUTS`), builds
  a launch tail (drive-root path for a disk icon, else the shortcut's
  own path) and calls `do_aopen`, **then unconditionally `return`s**
  (`:1249-1250`) — **corrected 2026-07-21 (Researcher re-validation vs.
  source)**: the previous entry here claimed `do_aopen` is invoked "once
  per selected source icon"; that is **wrong**. `an_dest` is resolved
  **once**, before the `while` loop (`:1209`), and never changes between
  iterations, so `if (an_dest && (an_dest->a_aicon >= 0))` is either true
  on every iteration or false on every iteration. The `return` at
  `:1250` is a genuine C `return` from `fun_desk2win` itself (a `void`
  function), not a `continue` — it exits the loop **and the function**
  on the very **first** selected source icon that reaches this check, so
  `do_aopen` fires **exactly once** per `fun_desk2win` call, using only
  that first icon's tail; every other selected source icon is left
  completely unprocessed (not even passed to `fun_file2any`). Multi-select
  drag-to-application-icon-in-a-window is therefore a **single-launch,
  first-icon-wins** operation, not a multi-launch one. Otherwise (`an_dest`
  not an application, or `CONF_WITH_DESKTOP_SHORTCUTS` off), calls
  `fun_file2any` per source icon, rebuilding the destination window
  after each successful copy.
- **`fun_desk2desk`** (`:1260-1299`): drag between two desktop icons.
  Rejects self-drop (`source==target`, `:1278`) and disk-icon-onto-
  itself (`:1285-1289`, `illegal_op_msg`). Same non-existent-drive/trash/
  printer skip rule as `fun_desk2win`. Delegates to `fun_file2any` with
  `wn_dest=NULL` (signals "desktop-to-desktop" inside `fun_file2any`).
  **This — not `fun_desk2win` — is where the genuine multi-launch case
  lives** (added 2026-07-21, see the correction above): unlike
  `fun_desk2win`, this `while` loop has **no early `return`** and never
  inspects `exit_desktop` or `fun_file2any`'s return value between
  iterations (`:1297`, bare call, result discarded) — so if `target` is
  an application file icon, dragging *N* selected desktop icons onto it
  calls `fun_file2any` → `fun_file2desk`'s `AT_ISFILE`+`a_aicon>=0`
  branch → `do_aopen` **once per selected source icon**, each overwriting
  the file-static `exit_desktop`. Whether the desktop actually exits
  after `fun_drag` returns therefore depends on the **last** icon
  processed by `win_isel`'s iteration order, not "the first successful
  launch" (there is no early exit at all in this function).

---

## `fun_op`/`fun_del`/`fun_mkdir` — File-menu entry points

- **`fun_op(op, icontype, pspath, pdest)`** (`:782-803`): the single
  choke point for Copy/Move/Delete from any caller. Always runs
  `dir_op(OP_COUNT, …)` first; if that fails, shows `illegal_op_msg()`
  and returns `FALSE` without attempting the real operation. If the
  count is zero files+folders, returns `FALSE` silently (nothing
  selected/matched — no alert). Otherwise runs `dir_op(op, …)` for real
  and returns `TRUE` **regardless of whether every individual item
  succeeded** — `dir_op`'s own per-item skip/retry/abort handling is
  already complete by the time `fun_op` returns; `fun_op`'s `TRUE` only
  means "the operation was attempted," not "every item succeeded."
- **`fun_del(pw, sobj)`** (`:1376-1424`): the "Delete" menu item /
  Delete-key handler. If the current window is the desktop
  (`G.g_cwin==DESKWH`), this is **icon removal**, not necessarily file
  deletion: prompts `wants_to_delete_files()` (Delete vs. Remove vs.
  Cancel — see below) once for the whole batch, then loops over every
  selected desktop icon, builds each one's path, and calls
  `delete_ffd()` (which does its own `fun_op(OP_DELETE,…)` on that
  path's full contents) — `refresh_drive()` is called after each
  successful delete-ffd. If not on the desktop, simply calls
  `fun_op(OP_DELETE, -1, &pw->w_pnode, NULL)` on the window's own
  selection and rebuilds on success.
- **`wants_to_delete_files()`** (`:1433-1446`): shows `STRMVDEL` (a
  three-choice alert: Cancel/Remove/Delete per the button numbering
  implied by `ret==1`→Remove, `ret==2`→Delete). "Remove" doesn't touch
  the filesystem at all — it instead **synthesizes a menu message**
  (`fun_msg(MN_SELECTED, OPTNMENU, RICNITEM, 0,0,0)`, `:1443`) to invoke
  the "Remove desktop icon" function (`deskins.c`'s `rmv_icon`, see
  [`deskins.md`](deskins.md)) as if the user had picked that menu item
  directly — i.e. `fun_del`'s desktop-icon path can silently redirect
  into an entirely different code path (icon removal, not file
  deletion) based on the user's alert choice.
- **`fun_mkdir(pw_node)`** (`:680-736`): "New Folder" — a small
  dialog+retry loop around `dos_mkdir`; on success, calls `fun_rebld()`
  on the containing window's path. Deeply-nested-path guard: if the
  computed full path would be within 3 characters of `LEN_ZPATH`, shows
  `STDEEPPA` and gives up rather than retrying (`:718-724`) — distinct
  from `deskdir.c`'s `MAX_LEVEL` recursion-depth guard, this is a
  **string-length** guard specific to `fun_mkdir`'s single-level
  creation.

---

## Edge cases discovered in source

- **`fun_msg` never calls the AES** (`deskfun.c:101-113`): it writes
  directly into `G.g_rmsg[]` and calls `hndl_msg()` in-process — "keep
  DESKTOP messages internal to DESKTOP" per the source comment
  (`:103`). This means a synthesized `WM_REDRAW`/`MN_SELECTED` via
  `fun_msg` is processed **synchronously**, before `fun_msg` returns —
  unlike a real AES-originated message, which is only processed on the
  next `evnt_multi` drain in the main loop. Any code relying on
  "queued, processed later" semantics for these synthetic messages would
  be wrong.
- **`wants_to_delete_files`'s "Remove" branch changes the operation
  entirely** (`deskfun.c:1442-1444`) — see above; a NewTOS reimplementation
  must preserve this redirection (Delete removes files; Remove removes
  just the desktop icon and leaves the file alone), not merely the alert
  text.
- **Multi-select drag onto an application icon triggers multiple
  launches only in `fun_desk2desk`, not `fun_desk2win`** (**corrected
  2026-07-21** — see the detailed note in the `fun_desk2win`/
  `fun_desk2desk` section above; the previous version of this entry had
  the two functions' behaviour backwards). `fun_desk2win` (`:1234-1251`)
  calls `do_aopen` **at most once** — an unconditional `return` at
  `:1250` ends the whole function immediately after the first qualifying
  selected icon, so remaining selected icons are never even passed to
  `fun_file2any`. `fun_desk2desk` (`:1260-1299`) has no such early exit:
  every selected source icon reaches `fun_file2any`/`fun_file2desk` in
  turn, so dropping *N* selected desktop icons onto a desktop
  application icon **does** call `do_aopen` once per icon, each
  overwriting `exit_desktop`; the final exit decision reflects whichever
  icon `win_isel` visits last, not "the first successful launch." Not
  fully traced beyond `do_aopen`'s call site (`do_aopen` itself is
  `desksupp.c`, out of scope for this task — see the new "Missing
  coverage" note under Dependencies).
- **Copying a disk icon reads the drive letter from the live `ICONBLK`,
  not the `ANODE`** (`fun_file2desk`'s `AT_ISDISK` case, `:1043-1044`,
  and `fun_file2any`'s disk-icon path, `:1157-1159` via
  `build_root_path`) — two different code paths for "what's this disk
  icon's letter," one via `ib_char`, one via `a_letter`
  (`build_root_path`'s implementation is `util/miscutil.c`, not traced,
  but is presumably `ANODE`-driven through the caller). Worth a
  regression test if NewTOS's disk-icon letter can ever diverge between
  the two representations.
- **`fun_op` returning `TRUE` does not mean "no errors"** — it means
  "the count was nonzero and the operation was attempted"; per-item
  failures are entirely swallowed by `dir_op`'s own alert loop before
  `fun_op` returns.

---

## Error conditions

| Condition | Source location | Result |
|---|---|---|
| `dir_op(OP_COUNT,…)` fails (deep-recursion/OOM during counting) | `deskfun.c:793-795` | `illegal_op_msg()`, `fun_op` returns `FALSE` |
| Count is zero (nothing selected/matched) | `deskfun.c:796-797` | `fun_op` returns `FALSE` silently, no alert |
| `fun_mkdir`'s path would exceed `LEN_ZPATH-3` | `deskfun.c:720-724` | `STDEEPPA` alert, loop breaks (dialog stays open for another attempt only if user hasn't broken out — actually breaks the `while(1)`, ending the dialog) |
| `dos_mkdir` fails for a recoverable reason | `deskfun.c:730` | `STFOFAIL` alert, Cancel breaks, else retries |
| Search: `dos_sfirst` returns neither 0/ENMFIL/EFILNF (`search_recursive`) | `deskfun.c:471-472` | "some strange kind of error, ignore silently", returns `TRUE` (continue) |
| Search: user cancels folder-display prompt (`STCNSRCH`) | `deskfun.c:427-428` | `search_display`/`search_recursive`/`fun_search` all unwind returning `FALSE` |
| Print buffer allocation fails (`CONF_WITH_PRINTER_ICON`) | `deskfun.c:1060-1063` | `malloc_fail_alert()`, print loop skipped entirely |

---

## Verification criteria

- **`fun_op` count-then-act**: given a selection whose `dir_op(OP_COUNT)`
  reports 0 files/folders, `fun_op` must perform **no** filesystem calls
  and return `FALSE`.
- **Move-vs-copy modifier**: `fun_win2win`/`fun_file2desk`/… must select
  `OP_MOVE` iff `keystate & MODE_CTRL`, else `OP_COPY` — a program cannot
  observe the desktop's internal dispatch, but a user test (drag with/
  without Ctrl held) must reproduce this.
- **Trash-drop confirmation only for desktop-sourced drags**:
  `fun_file2desk`'s `AT_ISTRSH` case only calls
  `wants_to_delete_files()` when `icontype_src >= 0` (desktop source);
  a window-to-trash drag must instead rely on `dir_op`'s own
  delete-confirmation preference path.
- **"Remove" redirect**: choosing "Remove" in `wants_to_delete_files`'s
  alert must produce the same result as picking Options > Remove desktop
  icon directly — the underlying file must be untouched.

---

## Dependencies

### Requires (must be initialized / functional)
- [`deskdir.md`](deskdir.md) — `dir_op()` (via `fun_op`)
- `deskapp.c` — `ANODE` lookups (`app_afind_by_id`/`app_afind_by_name`)
- `deskwin.c` — `WNODE`/`win_find`/`win_isel`/`win_sinfo`
- `desksupp.c` — `desk_verify`/`desk_clear`/`do_wredraw`/`do_aopen`/
  `do_diropen`/`i_find`/`valid_drive`/`build_root_path`/`refresh_drive`
- [`deskinf.md`](deskinf.md) — dialog helpers (`inf_sset`/`inf_sget`/
  `inf_show`/`set_tedinfo_name`)
- [`deskpro.md`](deskpro.md) — `pro_run`/`pro_exec` (new, 2026-07-21) —
  the desktop-side wrapper every `do_aopen` launch ultimately calls,
  terminating in the AES `shel_write(SHW_EXEC,…)` call already
  contract-documented in `docs/api/aes/scrap-shell.md`

**Missing coverage (flagged 2026-07-21):** `do_aopen`/`do_open`
(`desksupp.c:838-1090`/`:1191-…`) — the function that every launch path
in this file (`fun_win2win`, `fun_file2desk`, `fun_desk2win`) calls
directly, and which resolves the installed-application lookup, builds
the command tail, and finally calls `pro_run` — is **referenced by name
throughout this document and `deskmain.md` but has never itself been
extracted**. `desksupp.c` is a large file only partially covered
(`do_wredraw` alone, in `desksupp-redraw.md`); a full `do_aopen`/`do_open`
extraction (their `ANODE`/`is_installed`/`app_afind_by_id`/
`opn_appl`/`file_exists`/viewer-fallback logic) is out of scope for this
task and should be a dedicated follow-up (candidate: extend
`desksupp-redraw.md` into a general `desksupp.md`, or a new
`desksupp-launch.md`) before the 1:1 rebuild's `dir.rs`/launch path can
claim full call-sequence fidelity for "Open"/double-click/drag-to-app.

### Used by
- `deskmain.c`'s `do_filemenu` (`OPENITEM`/`SHOWITEM`/`SRCHITEM`/
  `NFOLITEM`/`CLOSITEM`/`CLSWITEM`/`SLCTITEM`/`MASKITEM`/`DELTITEM`) —
  see `deskmain.md`
- [`deskact.md`](deskact.md) — `act_bdown`'s drag-release path calls
  `fun_drag` (via `deskmain.c`'s `hndl_button`, not directly — cross-check
  during implementation)
- [`deskins.md`](deskins.md) — `fun_alert`/`illegal_op_msg` reused
  throughout the install dialogs

## Related data structures

- `docs/api/desktop/_data-model.md` — `ANODE`/`FNODE`/`WNODE`/`PNODE`
- `docs/api/desktop/deskdir.md` — `DIRCOUNT`, `OP_*` codes

## Cross-references

Not independently cross-checked against Compendium/Profibuch — this is
desktop-application UI/dispatch behaviour, not documented TOS API surface
(`docs/COMPATIBILITY.md`: EmuTOS is the reference, not the template; the
*filesystem effect* of a drag-and-drop is the only piece that could be
"contract" in the loose sense of "a program can observe files changing
underneath it," and that reduces to the already-spec'd GEMDOS calls).

## Configuration / version notes

See [Build configuration](#emutos-source) above for the five
`CONF_WITH_*` gates in this file. No TOS-version-specific (1.x/2.x/3.x/
4.x) behaviour identified — `deskfun.c` is EmuDesk-specific.

---

## NewTOS implementation notes

(Empty during Phase 00. Filled in when the corresponding implementation
phase begins.)
