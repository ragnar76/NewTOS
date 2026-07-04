# `deskins.c` — icon/application install dialogs, `ANODE` lifecycle

> Phase-00 extraction, Track A. Source: EmuTOS @ `1a7484ab`,
> `desk/deskins.c` (1412 lines). Per
> [`docs/api/desktop/_overview.md`](_overview.md)/[`_data-model.md`](_data-model.md):
> `ANODE` is defined there (rendering-relevant fields only); this entry
> documents the *installation* behaviour that creates/edits/removes
> `ANODE`s, which `_data-model.md` explicitly deferred ("full semantics
> … are in `docs/api/desktop.md`" — that file does **not** cover install
> dialogs either; this is the first extraction of `deskins.c`).
>
> **Contract vs. internal** (`docs/COMPATIBILITY.md`): entirely
> desktop-application-internal. No GEMDOS/AES/VDI trap takes an `ANODE*`.
> What a program *can* eventually observe is the resulting
> `EMUDESK.INF` file format (see [`deskinf.md`](deskinf.md)'s appendix)
> if NewTOS chooses to support reading real EMUDESK.INF files — out of
> scope unless that goal is adopted (flagged in `_data-model.md` already).

---

## EmuTOS source

**Primary locations (public, `deskins.h`):**
- `deskins.c:71-80` `is_installed(pa)`
- `deskins.c:87-111` `is_viewer(pa)` (`CONF_WITH_VIEWER_SUPPORT`)
- `deskins.c:290-339` `ins_devices(void)`
- `deskins.c:721-756` `ins_app(void)`
- `deskins.c:1234-1311` `ins_icon(sobj)`
- `deskins.c:1341-1411` `ins_shortcut(wh, mx, my)` (`CONF_WITH_DESKTOP_SHORTCUTS`)
- `deskins.c:1317-1335` `rmv_icon(sobj)`
- `deskins.c:146-184` `snap_icon(x, y, px, py, sxoff, syoff)`
- `deskins.c:989-1004` `is_executable(filename)` (`CONF_WITH_DESKTOP_SHORTCUTS
  || CONF_WITH_WINDOW_ICONS`)

**Installed handler / vector:** `(none)` — called from `deskmain.c`'s
`do_optnmenu()` (`IDSKITEM`/`IAPPITEM`/`IICNITEM`/`RICNITEM`, see
`deskmain.md`), from [`deskfun.md`](deskfun.md)'s `fun_drag`/
`fun_win2desk`/etc. (`ins_shortcut`), and from `deskmain.c`'s startup
sequence (`ins_devices`, once at boot — cross-check against `deskmain.md`
if it doesn't already cite this).

**Helpers called (this file, `static`):**
- `deskins.c:118-134` `grid_free(x,y)` — is a desktop grid cell occupied
  by another `AF_ISDESK` `ANODE`?
- `deskins.c:197-222` `ins_posdisk(dx, dy, pdx, pdy)` — scan rightward/
  downward from a starting grid cell for the first free one
- `deskins.c:230-255` `install_drive(drive)` / `:263-281`
  `remove_drive(drive)` — the two `ins_devices()` helpers
- `deskins.c:345-351` `clear_all_autorun(void)` — only one `ANODE` may
  carry `AF_AUTORUN`
- `deskins.c:359-372` `remove_all_viewers(void)` (`CONF_WITH_VIEWER_SUPPORT`)
- `deskins.c:381-393` `atow(s)` — ASCII-decimal-to-`WORD`, stops at first
  non-digit (used for the function-key text field)
- `deskins.c:404-444` `get_funkey(tree, pa, installed)` — validates a
  function-key dialog field, checking range and duplicates
- `deskins.c:455-600` `ins_app_dialog(pa, pathname, pfname, installed)` —
  the "Install Application" dialog's full field read/write logic
- `deskins.c:611-710` `ins_one_app(curr, pf)` — resolves which `ANODE` an
  "Install Application" invocation targets (desktop shortcut, window
  file, or default-viewer match) before calling `ins_app_dialog`
- `deskins.c:762-770` `insert_icon(tree, obj, nicon)` — swaps a dialog's
  preview-icon object to a different `G.g_iblist[]` entry (uses a
  **single file-`static` `ICONBLK ib`** scratch instance, see
  [Edge cases](#edge-cases-discovered-in-source))
- `deskins.c:781-978` `install_desktop_icon(pa)` — the "Install desktop
  icon" dialog (disk/trash/printer *and* file/folder-shortcut icon
  editing)
- `deskins.c:1012-1022` `set_icon(pa, icon)` (`CONF_WITH_WINDOW_ICONS`) —
  sets `a_dicon`, and `a_aicon` too iff the file is executable by
  extension
- `deskins.c:1028-1050` `allocate_window_anode(type)`
  (`CONF_WITH_WINDOW_ICONS`)
- `deskins.c:1061-1222` `install_window_icon(pf)` (`CONF_WITH_WINDOW_ICONS`)
  — the "Install window icon" dialog

**Helpers called (other files, cited — not re-derived here):**
- `deskapp.c` `app_alloc`/`app_free`/`app_afind_by_name`/`app_afind_by_id`/
  `app_afind_viewer`/`app_blddesk` (`ANODE` pool + rebuild — see
  `deskapp.md`)
- `deskwin.c` `win_isel`/`win_find`
- `desksupp.c` `deselect_all`/`i_find`
- [`deskinf.md`](deskinf.md) `inf_sset`/`inf_sget`/`set_tedinfo_name`/
  `draw_dial`/`draw_fld`/`start_dialog`/`end_dialog`
- [`deskfun.md`](deskfun.md) `fun_alert`/`illegal_op_msg`
- `util/optimize.c` `scan_str` (declared `deskapp.h`, defined `deskapp.c:258-289`
  — copies a string into the shared `G.g_atext[]` ANODE-text arena)

**Headers / type definitions used:**
- `deskins.h:16-18` — `FIRST_FUNKEY`=1, `LAST_FUNKEY`=20, `NUM_FUNKEYS`=20
- `deskapp.h:27-38` — `ANODE` bit flags: `AF_ISCRYS`/`AF_ISDESK`/
  `AF_ISPARM`/`AF_APPDIR`/`AF_ISFULL`/`AF_VIEWER`/`AF_AUTORUN`/
  `AF_ISEXEC`/`AF_WINDOW`
- `deskapp.h:43-49` — `AT_ISFILE`=0/`AT_ISFOLD`=1/`AT_ISDISK`=2/
  `AT_ISTRSH`=3/`AT_ISPRNT`=4 (`CONF_WITH_PRINTER_ICON`)
- `deskapp.h:54-62` — `IG_HARD`=0/`IG_FLOPPY`=1/`IG_FOLDER`=2/`IG_TRASH`=3/
  `IG_PRINT`=4/`IG_REMOV`=5/`IG_APPL`=6/`IG_DOCU`=7 (builtin icon indices
  into `G.g_iblist[]`)

**System variables / globals read:**
- `G.g_ahead` — the live `ANODE` list, walked by `ins_devices`/
  `grid_free`/`clear_all_autorun`/`remove_all_viewers`/`get_funkey`
- `G.g_icw`/`G.g_ich`/`G.g_desk` — desktop grid pitch + work area (see
  `_data-model.md`), read by `snap_icon`/`ins_posdisk`/`grid_free`
- `G.g_numiblks`/`G.g_iblist[]` — icon count/table, read by
  `install_desktop_icon`/`install_window_icon`'s up/down icon-picker and
  by `insert_icon`
- `G.g_cwin` (`CONF_WITH_DESKTOP_SHORTCUTS`/`CONF_WITH_WINDOW_ICONS`) —
  distinguishes "installing from a desktop shortcut" vs. "installing from
  a window selection" in `ins_one_app`/`ins_app`/`ins_icon`

**System variables / globals written:**
- New/edited `ANODE` fields throughout (`a_flags`/`a_type`/`a_letter`/
  `a_aicon`/`a_dicon`/`a_xspot`/`a_yspot`/`a_funkey`/`a_pappl`/`a_pdata`/
  `a_pargs`) — see per-function detail below

**Build configuration affecting this function:**
- `CONF_WITH_VIEWER_SUPPORT` — `is_viewer`, the "default viewer" concept
  in `ins_one_app`/`ins_app_dialog` (`AF_VIEWER`, `remove_all_viewers`)
- `CONF_WITH_DESKTOP_SHORTCUTS` — desktop file/folder shortcut installs
  (`ins_shortcut`, the `AT_ISFILE`/`AT_ISFOLD` branches in
  `install_desktop_icon`, and `is_executable`'s first gate)
- `CONF_WITH_WINDOW_ICONS` — the entire "install window icon" feature
  (`install_window_icon`/`set_icon`/`allocate_window_anode`, and
  `is_executable`'s second gate)
- `CONF_WITH_PRINTER_ICON` — the printer icon type throughout
  `install_desktop_icon`/`ins_devices` (indirectly, via `AT_ISPRNT`)
- `CONF_WITH_3D_OBJECTS` — purely cosmetic dialog Y-offset adjustments
  around the up/down icon-picker buttons (`install_desktop_icon`,
  `install_window_icon`) — **not** re-derived further, layout-only
- All default **on** in the general/Atari build except
  `CONF_WITH_3D_OBJECTS` (default `0`, see `_overview.md`'s config-variant
  convention — this one is genuinely off by default even outside
  `TARGET_192`, confirmed at `config.h` — **needs deeper analysis** if a
  later task needs the exact default line; not re-verified here since it
  only affects pixel offsets, not behaviour)

---

## Source extraction summary

`deskins.c` covers everything that creates, edits, or removes an
`ANODE` interactively (as opposed to `deskapp.c`'s `app_parse`/`app_save`,
which (de)serialize the `ANODE` list to/from `EMUDESK.INF` — see
[`deskinf.md`](deskinf.md)'s appendix). Three logically separate features
share the file:

1. **`ins_devices()`** — non-interactive, called at startup (and
   presumably after a media-change/rescan, cross-check `deskmain.md`):
   reconciles the desktop's disk-drive icons against the actual set of
   mounted drives (`dos_sdrv(dos_gdrv())`), removing icons for
   now-absent drives and adding icons for newly-present ones.
2. **`ins_app()`** — the "Install Application" dialog: associates a
   document type (wildcard) and launch behaviour with a `.PRG`/`.APP`/
   `.TOS`/`.TTP`/`.GTP` file, so that double-clicking a matching data file
   later launches this application with it as an argument.
3. **`ins_icon()`/`rmv_icon()`/`ins_shortcut()`** — desktop icon and
   (optionally) window-icon and desktop-shortcut management: create/edit/
   remove the disk/trash/printer/shortcut icons that live directly on the
   desktop surface, and (separately, `CONF_WITH_WINDOW_ICONS`) custom
   icons for specific files/folders shown *inside* windows.

All three ultimately mutate `G.g_ahead`'s `ANODE` chain via
`deskapp.c`'s `app_alloc()`/`app_free()` (a fixed-size pool of
`NUM_ANODES`=64 slots, see `deskapp.h:75`) and, for anything
desktop-visible, trigger `app_blddesk()` to rebuild the desktop's
`OBJECT` tree.

---

## `snap_icon`/`ins_posdisk`/`grid_free` — desktop grid placement

`deskins.c:146-184` (`snap_icon`), `:118-134` (`grid_free`), `:197-222`
(`ins_posdisk`). Already referenced from `_data-model.md`'s
"`a_xspot`/`a_yspot` unit gotcha" section (flagged there as **needs
deeper analysis** — this section resolves that flag).

- **`snap_icon(x, y, px, py, sxoff, syoff)`**: converts a raw pixel
  position (typically the current mouse position, minus the cursor's
  offset within the dragged icon, `sxoff`/`syoff`) into a grid-aligned
  pixel position. Computes how many whole columns/rows fit in
  `G.g_desk.g_w`/`g_h` at pitch `G.g_icw`/`g_ich`, and how many "spare"
  pixels are left over after fitting that many whole columns/rows
  (`spare_pixels = g_w - columns*icw`) — these spare pixels are
  distributed **evenly as a single leading margin** (`spare_pixels /
  columns`, integer division — any remainder beyond that is simply lost,
  not distributed further), i.e. the icon grid is not flush to the
  desktop's top-left corner but instead centred by whole-cell excess.
  The target column/row are clamped to `[0, columns-1]`/`[0, rows-1]`
  (`:164,181` — `min(...)`) as a safety measure against
  out-of-range results from a mouse position outside the desktop.
- **`grid_free(x, y)`** (grid coordinates, not pixels): `TRUE` unless
  some other `AF_ISDESK` `ANODE`'s **current** `a_xspot`/`a_yspot`
  (pixels, per the dual-unit lifecycle already documented in
  `_data-model.md`) divides back to the same grid cell.
- **`ins_posdisk(dx, dy, pdx, pdy)`**: starting from the grid cell
  containing `(dx,dy)`, scans rightward, then wraps to the next row
  (resetting the column to 0, not to the original starting column) via
  a **row-major** search, returning the first free cell found via
  `grid_free`. **If the entire remaining grid is occupied** (loop falls
  through without finding one), returns the **input position
  unchanged** (`:220-221`) — i.e. a completely full desktop grid places
  the new icon exactly on top of an existing one, with no further
  fallback (no scrolling, no error, no alert).

`install_drive`/`install_desktop_icon`/`ins_shortcut` all call
`ins_posdisk(0, 0, &x, &y)` — i.e. **always starting the search from grid
cell `(0,0)`**, not from any "last placed" position — meaning
successively-installed icons are placed by scanning from the top-left
every time, not by remembering where the last one landed. `ins_shortcut`
is the one exception among these three: for multiple simultaneously
dragged icons, it calls `ins_posdisk(x, y, &x, &y)` **after** placing
each one, using the *just-placed* position as the next search's start,
so multiple icons dropped together fill left-to-right/top-to-bottom
starting from the drop point rather than from `(0,0)` each time
(`deskins.c:1396`).

---

## `ins_devices` — reconcile disk icons with mounted drives

`deskins.c:290-339`. Builds a bitmask of `installed` drives (from
existing `AT_ISDISK` `ANODE`s' `a_letter`) and compares against
`current = dos_sdrv(dos_gdrv())` (the live drive bitmap). Removes icons
for `installed & ~current` (drives that vanished), installs icons for
`current & ~installed` (drives that appeared), in that order (removals
before installs). Returns `TRUE` iff any change was made (used by the
caller to decide whether to rebuild the desktop). `install_drive`
(`:230-255`) sets `a_aicon = (drive > 1) ? IG_HARD : IG_FLOPPY` — **A
and B are always floppies, C and up are always hard disks**, matching
`deskapp.md`'s `app_blddesk`/`build_inf` observation for the built-in
EMUDESK.INF generator (same rule, independent code path).

---

## `ins_app`/`ins_app_dialog`/`ins_one_app` — install application

`ins_app()` (`:721-756`) branches on whether the current context is the
desktop (`CONF_WITH_DESKTOP_SHORTCUTS`, `G.g_cwin==DESKWH`) or a window:
on the desktop, loops every selected desktop icon
(`win_isel(G.g_screen, G.g_croot, curr)`); in a window, loops every
selected `FNODE`. Each iteration calls `ins_one_app(curr, pf)`, stopping
early (`break`) if it returns negative (user cancelled further installs)
— crucially, **the loop does not stop on `rc==0`** ("nothing changed for
this one," e.g. user hit Skip), only on cancel. Returns `-1` if
**any** item actually changed (window context only tracks `change++`;
the desktop-shortcut context, per the comment at `:735`, "doesn't track
changes since we don't rebuild the desktop" — installing an application
via a desktop shortcut **never triggers a rebuild**, only via a window
file selection does).

`ins_one_app` (`:611-710`) resolves *which* `ANODE` the dialog will
edit:
- **Desktop-shortcut context** (`CONF_WITH_DESKTOP_SHORTCUTS`): looks up
  the shortcut's own path via `app_afind_by_id`, then searches for an
  **already-installed, non-shortcut** `ANODE` for the same full path
  (`app_afind_by_name`) — if found, edits *that* one instead of the
  shortcut's own `ANODE`, so installing-an-application through a desktop
  shortcut and through the equivalent window file converge on the same
  underlying record.
- **Window context**: if `CONF_WITH_VIEWER_SUPPORT` and a default viewer
  is already installed and matches this exact file, uses the viewer's
  `ANODE`; otherwise searches by name (`app_afind_by_name`, ignoring
  `AF_ISDESK|AF_WINDOW|AF_VIEWER` flags when matching — i.e. matches
  purely on path+name, not on how the existing entry was categorized).

`ins_app_dialog` (`:455-600`) is the actual dialog loop
(`ADINSAPP` resource): pre-fills name/args/doctype/funkey/flags fields
from the existing `ANODE` if any, loops `form_do` until a terminal
button:
- **`APINSTAL`**: allocates a new `ANODE` if not already installed,
  validates the function key (`get_funkey` — invalid or duplicate-and-
  user-declined both **request a retry without exiting the dialog**,
  `:521-529`, by setting `exitobj=-1` and looping the outer `do…while`),
  sets `a_flags` from the GEM-app/needs-parms/app-dir/full-path/autorun
  radio/checkbox groups (`AF_ISPARM`/`AF_ISCRYS`/`AF_APPDIR`/`AF_ISFULL`/
  `AF_AUTORUN` — **choosing autorun clears every other `ANODE`'s
  autorun flag first**, `clear_all_autorun()`, enforcing "at most one
  autorun app"), builds the full pathname only if newly installed
  (`installed` case keeps the existing `a_pappl`), and — if
  `CONF_WITH_VIEWER_SUPPORT` and the resulting `ANODE` now qualifies as a
  viewer (`is_viewer`) — removes any other existing default viewer first.
- **`APREMOVE`**: frees the `ANODE`, but **only if `installed`** — you
  cannot "remove" the generic placeholder `ANODE` for a file type that
  was never actually installed (`:585-586`).
- **`APSKIP`**/**`APCANCEL`**: no state change; cancel additionally sets
  `change=-1` to stop the outer loop in `ins_app`.

---

## `ins_icon`/`rmv_icon` — desktop and window icon install/remove

`ins_icon(sobj)` (`:1234-1311`):
- **`sobj==0`** (menu-invoked with nothing selected): prompts
  `STICNTYP` (a choice: desktop / window / cancel, `CONF_WITH_WINDOW_ICONS`
  only offers the choice — without that config, always goes straight to
  `install_desktop_icon(NULL)`) and installs a **brand-new** icon of the
  chosen kind. Returns `1` (rebuild desktop) or `-1` (rebuild windows)
  or `0` (nothing changed/cancelled).
- **`sobj!=0` and it resolves to a desktop `ANODE`**
  (`app_afind_by_id(sobj)` succeeds): loops every selected desktop icon
  (again via `win_isel`), calling `install_desktop_icon(pa)` **per
  existing icon** — i.e. selecting multiple desktop icons and choosing
  "Install icon" edits each one in turn via repeated dialogs, summing how
  many were actually changed into the return value; a negative
  (cancelled) result from any one stops the loop early.
- **Else, if `CONF_WITH_WINDOW_ICONS`**: assumes the selection is inside
  the current window, loops every selected `FNODE`, calling
  `install_window_icon(pf)` per file/folder — return sign convention is
  **inverted** for this branch (`rebuild--` on success, i.e. **negative**
  = "rebuild windows", matching the desktop-menu handler's sign
  convention documented in `deskmain.md`: "`IICNITEM` rebuilds only if
  `rebld>0`... and instead does a window rebuild if `rebld<0`" — this
  confirms `deskmain.md`'s cross-reference from the caller side).

`install_desktop_icon(pa)` (`:781-978`) handles **both** creating a new
desktop icon (`pa==NULL`, defaults to a disk-type placeholder at the
first free grid slot from `(0,0)`) and editing an existing one
(`icon_exists=TRUE`). Field editability depends on the icon's *current*
type: file/folder shortcuts (`CONF_WITH_DESKTOP_SHORTCUTS`) only allow
editing the label and icon glyph (ID/drive/trash/print fields are
`DISABLED`); disk/trash/printer icons allow changing *everything*,
including re-categorizing the icon's type via the ID/Drive/Trash/Print
radio group (`inf_gindex(tree, ID_DRIVE, 3)` at commit time). The
up/down icon-picker buttons (`ID_UP`/`ID_DOWN`) cycle `curr_icon` through
`[0, G.g_numiblks-1]` clamped (not wrapped) at both ends, live-swapping
the preview via `insert_icon`+`draw_fld`. Disk-type icons additionally
**require** a non-blank drive letter (`retry=TRUE` if the `ID_ID` field
is empty or a space, `:917-918`) — the dialog **redisplays itself**
(`draw_dial`) with an alert (`STDRIVID`) rather than accepting an
incomplete disk icon. On `ID_SKIP`/`ID_CNCL` for a **newly-allocated**
icon (`!icon_exists`), the allocated `ANODE` is explicitly freed
(`:942-943`) — a cancelled "create new" leaves no dangling `ANODE`.

`install_window_icon(pf)` (`CONF_WITH_WINDOW_ICONS`, `:1061-1222`) is the
window-icon analogue: `pf==NULL` means "create a new file/folder-name
association not tied to a specific currently-visible file" (the name
field becomes editable, file/folder type becomes choosable); `pf!=NULL`
edits the icon for that specific already-existing `FNODE`'s match (name
fixed, type fixed to whatever the file actually is). `IW_INST`
(install/reinstall) versus `IW_REMV` (remove) both first resolve the
target `ANODE` by name (`app_afind_by_name(type, AF_ISDESK, "",
new_name,…)` when `pf==NULL`, else `pf->f_pa` directly) and compare
`pa->a_pdata` against the (possibly just-edited) name field
(`identical`) — **removing** a window icon requires the name to be
unchanged from the matched `ANODE`'s (`STNOMTCH` alert otherwise,
`:1183-1185`); **installing** only allocates a genuinely new `ANODE` if
the name is **not** identical to an existing match (`:1166-1177` —
i.e. re-submitting the same name/icon just calls `set_icon` on the
already-found `ANODE`, no duplicate is created).

`rmv_icon(sobj)` (`:1317-1335`): loops every selected desktop object,
frees the `ANODE` **only if it's flagged `AF_ISDESK`** (`:1327` — a
selected object that resolves to something else, e.g. this function
being called on the wrong root, is silently skipped, not erred).

---

## Edge cases discovered in source

- **A completely full desktop icon grid silently overlaps a new icon
  onto an existing one** (`ins_posdisk`, `:220-221`) — no error, no
  scroll, no alert; this is a genuine "can't happen in practice" gap
  (64 `ANODE` slots vs. typically far more grid cells) but is
  reproducible if a NewTOS reimplementation ever has a much smaller
  desktop resolution with many more installed icons than fit.
- **`insert_icon` uses a single file-`static` `ICONBLK ib` scratch
  instance** (`deskins.c:52`, written by `:762-770`) shared by **every**
  dialog's icon-preview object across `install_desktop_icon` and
  `install_window_icon` — safe only because dialogs are strictly modal
  (one at a time) and each dialog fully re-`insert_icon`s before
  displaying; a reimplementation with concurrent/re-entrant dialogs
  would need per-dialog storage instead.
- **`ins_app`'s desktop-shortcut branch never signals "rebuild needed"**
  even when an install actually happened (`:735-736` comment) — this
  appears to rely on the fact that installing-an-application changes
  only `a_pdata`/`a_pargs`/`a_flags`/icon numbers on an **existing**
  `ANODE` that's already correctly placed and drawn; no new icon needs
  adding to any tree. Worth a regression check that this assumption
  holds (i.e. that `ins_one_app`'s desktop-shortcut path never allocates
  a *new* `ANODE` that would then be missing from the desktop until the
  next unrelated rebuild).
- **`ins_icon`'s window-icon-loop return sign is inverted relative to
  its desktop-icon-loop** (`rebuild++` vs. `rebuild--`, `:1282` vs.
  `:1303`) — this is a **deliberate protocol** with the caller
  (`deskmain.c`'s `do_optnmenu`, per its own documented sign convention),
  not an accidental inconsistency; a reimplementation must preserve the
  sign, not just the magnitude.
- **Function-key validation can force a dialog retry mid-install**
  (`get_funkey`, invalid range or user-declined duplicate) — the dialog
  clears the funkey field, deselects the `ID_OK`-equivalent button, and
  redraws in place (`draw_dial`) rather than closing — a stateful retry
  loop inside what looks like a single `form_do` call from the outside.

---

## Error conditions

| Condition | Source location | Result |
|---|---|---|
| `app_alloc()` returns `NULL` (pool of 64 `ANODE`s exhausted) | `deskapp.c:207-222`, checked at e.g. `deskins.c:239-240`,`822-823`,`513-517` | `STAPGONE` alert (inside `app_alloc` itself); callers here treat it as "stop trying" (`return -1` from `install_drive`, `change=-1` from `ins_app_dialog`) |
| Function key out of `[FIRST_FUNKEY,LAST_FUNKEY]` range | `deskins.c:421-425` | `STINVKEY` alert, `get_funkey` returns `-1`, dialog retries |
| Function key already used by another `ANODE`, user declines to steal it | `deskins.c:436-437` | `STDUPKEY` alert; declining returns `-1` (retry); accepting clears the other `ANODE`'s key |
| Disk icon install with blank drive-letter field | `deskins.c:916-918` | `STDRIVID` alert, dialog redisplays, no commit |
| Window-icon remove with a changed (non-identical) name | `deskins.c:1183-1185` | `STNOMTCH` alert, no removal |

---

## Verification criteria

- **`ins_devices` idempotence**: calling `ins_devices()` twice in a row
  with no drive-mount changes between calls must return `FALSE` the
  second time and make no `ANODE` changes.
- **Grid placement determinism**: `ins_posdisk(0,0,…)` on an otherwise-
  empty desktop must return `(0,0)`; on a desktop with cell `(0,0)`
  occupied, must return `(1,0)` (row-major scan).
- **Autorun exclusivity**: installing a new application with the autorun
  checkbox selected must leave **at most one** `ANODE` with
  `AF_AUTORUN` set afterward, regardless of how many previously had it
  (`clear_all_autorun` runs unconditionally before setting the new one).
- **Cancelled new-icon install leaves no orphan `ANODE`**: choosing
  Skip/Cancel on a freshly-allocated (not-yet-committed) desktop or
  window icon must return the `ANODE` to the free pool
  (`G.g_aavail` count unchanged from before the dialog opened).

---

## Dependencies

### Requires (must be initialized / functional)
- `deskapp.c` — `app_alloc`/`app_free`/`app_afind_by_id`/
  `app_afind_by_name`/`app_afind_viewer`/`app_blddesk` (see `deskapp.md`)
- [`deskinf.md`](deskinf.md) — dialog primitives (`inf_sset`/`inf_sget`/
  `set_tedinfo_name`/`start_dialog`/`end_dialog`/`draw_dial`/`draw_fld`)
- [`deskfun.md`](deskfun.md) — `fun_alert`/`illegal_op_msg`
- `docs/api/desktop/_data-model.md` — `G.g_icw`/`g_ich`/`g_desk`/
  `g_iblist[]`/`g_numiblks`

### Used by
- `deskmain.c`'s `do_optnmenu` (`IDSKITEM`→`ins_devices`(?, cross-check),
  `IAPPITEM`→`ins_app`, `IICNITEM`→`ins_icon`, `RICNITEM`→`rmv_icon` — per
  `deskmain.md`'s own citation of this file)
- [`deskfun.md`](deskfun.md)'s `fun_drag` (`ins_shortcut`, drop-onto-
  empty-desktop case, `CONF_WITH_DESKTOP_SHORTCUTS`)
- [`deskact.md`](deskact.md)'s `move_drvicon` (`snap_icon`)
- `deskapp.c`'s `app_start()` (`snap_icon`, converting parsed
  `EMUDESK.INF` grid-unit spots to pixels — see `_data-model.md`'s
  "`a_xspot`/`a_yspot` unit gotcha", now resolved above)

## Related data structures

- `docs/api/desktop/_data-model.md` — `ANODE` (rendering-relevant
  fields), `a_xspot`/`a_yspot` dual-unit lifecycle
- `docs/api/structures/iconblk.md` — `ICONBLK` (the `ib` scratch instance,
  `G.g_iblist[]`)

## Cross-references

Not independently cross-checked against Compendium/Profibuch — icon
installation dialogs are desktop-application UI, not documented TOS API
surface. The resulting `ANODE` semantics (icon type, application
association, autorun) are cross-referenced against the `EMUDESK.INF`
format in [`deskinf.md`](deskinf.md), which **is** partially documented
in period sources (Atari's own EMUDESK.INF format was informally
published) — no such cross-check was performed in this pass; flagged as
a possible follow-up if NewTOS ever targets EMUDESK.INF file
compatibility.

## Configuration / version notes

See [Build configuration](#emutos-source) above. No TOS-version-specific
behaviour identified (EmuDesk-specific file; original Atari TOS desktop
had a broadly similar but not identical install-icon feature set per
general knowledge — not independently verified against a TOS ROM
disassembly, out of scope).

---

## NewTOS implementation notes

(Empty during Phase 00. Filled in when the corresponding implementation
phase begins.)
