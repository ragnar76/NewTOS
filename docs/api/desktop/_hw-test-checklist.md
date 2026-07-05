# Desktop — Real-Hardware Interaction Test Checklist (Task #7/#8)

> **Why this file exists.** Interactive input is **not** drivable in QEMU for the
> desktop build: `qemu raspi4b` has the SD/EMMC storage (so `C:` and directory
> windows exist) but no wired-up mouse/keyboard input path, and `qemu-virt` has
> input but no `C:` drive. Per the project decision of 2026-07-04, all
> **interactive** verification of the desktop happens on **real Pi4 hardware**.
>
> The static/source audit (Round 2, Audit #10) verified the desk-side *logic*
> against the EmuTOS extraction and drove every non-interactive check it could
> (compile both boards, clippy, a QMP boot screendump on `qemu raspi4b` with a
> representative SD image). This checklist is the **primary verification
> instrument** for everything that needs a live pointer/keyboard.
>
> **How to run:** boot the current `kernel8.img` on the Pi4 with the 4×500 MB
> FAT16 SD card (C–F) attached (see [[sd-card-real-hw-layout]]). Use a directory
> with **≥ 20 entries incl. sub-folders and 12-char 8.3 names** for the
> adversarial rows. Tick each row; note the actual result next to it. A row that
> fails is a defect — file it against the desk module.
>
> Legend: **[E]** = expected result. **[A #n]** = watch here for audit finding
> #n (see the Round-2 audit report). Rows marked **(residue)** are the
> historically fragile paths.

---

## A. Button / click manager (`hndl_button`, D3)

- [ ] **A1 — Single-click an icon in the top window** → the icon inverts
  (selected); every other icon in that window de-selects. **[E]** exactly one
  item highlighted.
- [ ] **A2 — Single-click empty work-area** in the top window → all icons in
  that window de-select, nothing else changes.
- [ ] **A3 — Open a second window** (double-click a folder), then **click an
  icon in the now-*background* window** (the one behind, still visible) → that
  background window becomes the current window **and** the clicked icon is
  selected (the click is *consumed*, not ignored). **[E]** background window is
  now current; item selected. *This is the core D3 fix.*
- [ ] **A4 — (was audit #1, now FIXED in #9/F1) Two windows, select an icon in
  window 1, then click an icon (or empty area) in window 2** → **window 1's
  selection clears**; only window 2 shows a highlighted item. **[E per EmuTOS]**
  only ONE window ever shows a selection. The `desk_clear_all` on `wh != g_cwin`
  (deskmain.c:957-958) was added in #9 — confirm the correct single-window
  behaviour on HW.
- [ ] **A5 — Single-click a desktop drive icon** (outside any window) → the
  drive icon selects; open windows de-select as appropriate; menu updates.
- [ ] **A6 — Double-click a folder icon** → a new directory window opens showing
  that folder's contents, staggered from the parent, titled `X:\SUB\`.
- [ ] **A7 — Double-click a drive icon** → a window opens at that drive's root
  (`X:\`), listing its contents.
- [ ] **A8 — Double-click a plain file** → **nothing happens** (launch is Task
  #9; must be an honest no-op, not a crash).
- [ ] **A9 — Click in the menu bar area / on the closer/fuller/mover/sizer** →
  routed as the corresponding `WM_*` (see sections D/E), not as a work-area
  click.

## B. Keyboard manager (`hndl_kbd`, D4)

- [ ] **B1 — Down-arrow / Up-arrow** with a scrollable top window → content
  scrolls one line; selection in that window clears first. **[E]** no
  `men_update` re-gray on arrow (EmuTOS quirk — Open/Delete stay as they were).
- [ ] **B2 — Up-arrow at the very top / Down-arrow at the very bottom**
  (boundary) → no scroll; **note:** any icons that were selected get
  *internally* de-selected but the screen is **not** repainted until the next
  redraw (faithful EmuTOS quirk — verify it matches, not a NewTOS-only bug).
- [ ] **B3 — Left/Right-arrow** with `Size to fit` **off** and a horizontally
  scrollable window → horizontal scroll by one column. (With `Size to fit`
  *on* — the default — there is no horizontal scroll; arrows are inert
  horizontally.)
- [ ] **B4 — Enter/Return (CR)** → de-selects **all** icons in all windows +
  desktop. **[E]** no menu re-gray (CR bypasses `men_update`, EmuTOS quirk):
  Open/Delete may remain enabled though nothing is selected.
- [ ] **B5 — ESC** → the top window re-scans its directory and repaints (use it
  after changing the SD contents to confirm a fresh scan).
- [ ] **B6 — Ctrl-O with exactly one item selected** → opens it (same as File →
  Open). With **zero** or **≥2** selected → Ctrl-O is **inert** (item disabled).
- [ ] **B7 — Ctrl-H (Close folder)** on a sub-folder window → navigates up one
  level; on a drive-root window → closes the window.
- [ ] **B8 — Ctrl-U (Close top window)** → closes the top window entirely.
- [ ] **B9 — Ctrl-W (Cycle)** with **≥2** windows → the bottom-most window comes
  to the top. With **1** window → inert (disabled).
- [ ] **B10 — Disabled-item shortcuts are inert:** Ctrl-D (Delete), Ctrl-A
  (Select-all), Ctrl-Z (EmuCON), Ctrl-R (Change-rez), Ctrl-S (Save-desktop),
  Ctrl-I (Info), Ctrl-N (New-folder) → **nothing happens** (all are Task-#9
  items kept disabled; the shortcut must not dispatch a disabled handler).
- [ ] **B11 — F1…F20** → no-op (no installed applications yet — Task #9).
- [ ] **B12 — Alt-A…Alt-Z** (fixed in #13: the keymap folds Alt+letter → ASCII 0
  and the Alt bit is now in `shift_state`) → opens that drive letter's root
  window (e.g. Alt-C → `C:\`). Confirm the correct drive opens (and no crash).

## C. Menu manager + `men_update` (D6)

*(Open each drop-down with the pointer and read the item states.)*

- [ ] **C1 — Desk menu** → `Desktop info...` enabled; selecting it shows the
  About alert; dismissing it repaints the screen underneath cleanly **(residue)**.
- [ ] **C2 — View menu checkmarks** → exactly one of `Show as icons` /
  `Show as text` is checked (Icon at boot); exactly one sort key checked
  (`Sort by name` at boot); `Size to fit` checked at boot. Switch each and
  re-open the menu — the checkmark must follow.
- [ ] **C3 — File menu, no window open, nothing selected** → `Open`, `Close
  folder`, `Close top window`, `Cycle windows` are **grey**; `Info/Rename`,
  `New folder`, `Select all`, `Set file mask`, `Delete`, `Execute EmuCON`,
  `Shutdown`, `Search` are **grey** (Task #9).
- [ ] **C4 — File menu, one window open, nothing selected** → `Close folder`
  and `Close top window` become **enabled**; `Open` still grey; `Cycle` still
  grey (needs ≥2).
- [ ] **C5 — File menu, exactly one icon selected** → `Open` **enabled**.
- [ ] **C6 — File menu, two+ icons selected** (Task #9 multi-select may be
  unavailable; if reachable) → `Open` **grey** again (can't open two at once).
- [ ] **C7 — File menu, two+ windows open** → `Cycle windows` **enabled**.
- [ ] **C8 — Options menu** → **every** item grey (all Task #9): `Set
  background`, `Install icon/application/devices`, `Remove desktop icon`, `Set
  preferences`, `Desktop configuration`, `Change resolution`, `Read .INF`,
  `Save desktop`. (Present-but-disabled, **not** clickable no-ops.)
- [ ] **C9 — Separators** in every drop-down render as grey dash lines and are
  never selectable.
- [ ] **C10 — Menu title flash:** selecting any item (mouse or shortcut) flashes
  the title highlighted then returns it to normal; the drop-down closes cleanly
  with **no residue** on the desktop underneath **(residue)**.

## D. View / sort (`do_viewmenu` → `desk_all`)

- [ ] **D1 — View → Show as text** → the top window switches to the text list
  (indicator / NAME.EXT / size / date / time columns); **no stale icons left
  behind (residue)**. Switch back to icons — same, clean.
- [ ] **D2 — (residue, the headline symptom) Two overlapping windows, switch
  View→text then View→icons** → **neither** window shows leftover pixels from
  the previous view in its own work area, and the covered part of the lower
  window is **not** repainted over the upper window. *This is the D2/R2 residue
  case that must be structurally gone.*
- [ ] **D3 — Sort by size** → files listed **largest first**; folders always
  precede files. (On the test SD: LYNXBOOT.IMG / KERNEL8.IMG should lead the
  files.)
- [ ] **D4 — Sort by type** → grouped by extension; **extension-less files sort
  first** (before any dotted name), right after the folders.
- [ ] **D5 — Sort by date** → **newest first**; folders first.
- [ ] **D6 — No sort** → raw directory (scan) order; **folders and files
  interleave** (no folder-first grouping).
- [ ] **D7 — Sort by name** (default) → folders first alphabetically, then files
  alphabetically. *(Statically + boot-screendump confirmed already; re-confirm
  interactively.)*
- [ ] **D8 — Size to fit toggle off** → a horizontal scrollbar becomes
  meaningful; the H-slider thumb now **renders** (fixed in #12, a mirror of the
  vertical slider). Confirm the thumb reflects the horizontal position and that
  Left/Right arrows scroll by a column.

## E. Window management (`hndl_msg`: TOPPED/MOVED/SIZED/FULLED/CLOSED/ARROWED/SLID)

- [ ] **E1 — Click a background window's title/body → it comes to the top**
  (frame drawn on top, content repainted, occlusion of the now-lower window
  correct).
- [ ] **E2 — Drag the mover (title bar)** → window moves; on release the content
  re-settles with no residue at the old position **(residue)**; items stay
  correctly placed relative to the new work-area origin.
- [ ] **E3 — Drag the sizer** → window resizes; the icon/text grid **re-flows**
  into the new column count; **shrinking** to fewer columns re-lays the grid
  (no clipped half-column left over) **(residue)**.
- [ ] **E4 — Click the fuller** → toggles full-screen extent ↔ previous size;
  grid rebuilds for each; the window is topped by the full action.
- [ ] **E5 — Click the closer / File→Close folder / WM_CLOSED** on a sub-folder
  → goes up one directory level; on a drive-root window → closes it entirely.
- [ ] **E6 — Scroll arrows (up/dn/lf/rt gadgets)** → scroll one line/column;
  content repaints cleanly; the vertical slider thumb tracks the position.
- [ ] **E7 — Drag the vertical slider** → jumps to the proportional scroll
  position; content correct; **at the extreme top/bottom** the last row is fully
  backed (no blank strip below the last item).
- [ ] **E8 — (was audit #3, fixed in #17/F3) Any full repaint
  (open/close/move/size/top/cycle, or dismissing the About box)** → the pointer
  should stay hidden for the whole repaint and reappear **once**, cleanly (the
  nested `graf_mouse` M_OFF/M_ON was collapsed to a single outer bracket in
  `full_redraw`/`win_shwall`). Confirm **no** mid-repaint cursor flicker and
  **no** persistent cursor droppings (holes/ghosts).
- [ ] **E9 — Open the maximum number of windows (7), each on a full directory,
  then close and reopen several times** → every window keeps showing its full
  content (no items silently missing). *This exercises the lazy
  `obj_wfree`-on-reuse arena path (audit finding #4, proven benign at 640×480 —
  confirm it holds on HW).*

## F. Adversarial / boundary rows (drive these deliberately)

- [ ] **F1 — Directory of 12-char 8.3 names** (`KEYBOARD.KBD`, `LYNXBOOT.IMG`)
  in icon view → labels do **not** overlap horizontally between columns, and do
  not bleed into the neighbour cell. *(Boot-screendump confirmed clean; re-check
  a denser directory.)*
- [ ] **F2 — Full grid** (more items than fit) → the fringe row/column shows a
  partial sliver (deliberate `+1`), the vertical slider thumb shrinks
  proportionally, scrolling reveals the rest with no gaps/dupes.
- [ ] **F3 — Empty directory** → window opens, shows `0 items   0 bytes used`,
  no phantom icons, scrollbars full-range/inert.
- [ ] **F4 — Directory with > 64 entries** → **all** entries listed and scrollable
  (no silent 64-cap; the rebuild dropped the old fixed buffer — confirm on a
  large real folder, D9).
- [ ] **F5 — Scroll to the exact bottom, then resize larger** → the scroll
  position re-clamps so no blank area appears below the last row (win_ocalc
  decrement-only re-clamp).
- [ ] **F6 — View-switch while scrolled past row 1** → the scroll position stays
  coherent between icon and text views (single virtual-grid scroll model, D5) —
  the same files remain in view, not a units-mismatch jump.

## G. Task-#9 selection, drag & file operations (`act.rs` / `dir.rs`)

> **Why here.** The **backend** (`d_doop`/`dir_op`) is proven programmatically
> (`desk::dir::selftest`, run once against a fresh `/tmp/sd.img` on `qemu
> raspi4b`: Dcreate / Fcreate+Fwrite / Copy / Rename / recursive Delete / Count,
> each confirmed by a dir rescan — 12/12 pass, image intact + test folder
> cleaned up afterwards). Everything below needs a **live pointer/keyboard** and
> so is real-HW only. **Test exclusively against a fresh
> `tools/make-sd-image.sh /tmp/sd.img` — these rows mutate the image.**

### G.1 Selection (`act_bsclick` / `act_chkobj` — D7 hit-test)

- [ ] **G1 — Single-click an icon's glyph or its label** → selects it; the dead
  space *between* glyph and label (icon view) is a **miss** (`act_chkobj`
  sub-rectangle hit-test, not the bounding cell). Every other item de-selects.
- [ ] **G2 — Shift-click a second icon** → **adds** it to the selection (both
  highlighted); shift-click a selected icon → **removes** just it; shift-click
  empty space → no-op.
- [ ] **G3 — Single-click an already-selected icon (no shift)** → **no change**
  (does *not* collapse a multi-selection) — this is what lets a held button then
  drag the whole group (`deskact.c:590`).
- [ ] **G4 — Rubber-band: press on empty work area and drag a box** → every item
  whose glyph/label the box touches is selected; an item the box only grazes in
  its margin (not the glyph/label) is **not** selected.

### G.2 Drag & drop (`act_bdown` → `fun_drag` dispatch tree)

- [ ] **G5 — Drag a file icon onto a folder icon (same window)** → the file is
  **copied** into that folder; the folder window (if open) rebuilds. Hold **Ctrl**
  while dragging → **moved** instead (source disappears).
- [ ] **G6 — Drag a selection from one window into another window** → copied to
  the target window's directory (Ctrl = move); the source selection clears; both
  windows rebuild. A **folder** Ctrl-moved this way keeps a correct `..` entry
  (fixed in #15 — spot-check the moved folder still opens and its parent chain is
  intact; `fsck` clean).
- [ ] **G7 — Drag a window selection onto the desktop Trash icon** → a confirm
  alert appears; OK **deletes** the files; rebuild the source window.
- [ ] **G8 — Drag a window selection onto a desktop Disk icon** → copied to that
  drive's root (Ctrl = move).
- [ ] **G9 — Drag desktop Disk C onto Disk D** → C's whole contents copied to
  D:\ (heavy — use the small test image). Drop onto Trash → confirm + delete.
- [ ] **G10 — (move_drvicon, bypasses fun_drag) Drag a desktop drive icon to an
  empty desktop cell** → the icon snaps to the nearest grid cell and stays there;
  **no** file operation happens, **no** confirm. Position now **persists** via
  Options→Save desktop (Task #14, see H4/H5).
- [ ] **G11 — Drag an icon and release it back on itself (same window)** →
  treated as a **cancelled** drop; nothing copies/moves.
- [ ] **G12 — Drag a window item onto empty desktop space** → "Can't drop items
  here" alert (desktop shortcuts are Task #14).

### G.3 File-menu actions (menu wired via `men_update`)

- [ ] **G13 — Select one/more items, File→Delete (or Ctrl-D)** → confirm alert;
  OK deletes them; window rebuilds; dir rescan shows them gone. Delete is
  **grey** with nothing selected.
- [ ] **G14 — File→New folder (a window open)** → name-entry dialog; type a name,
  OK → `Dcreate`; the folder appears in the window. Empty name / Cancel → no-op,
  screen restored. Grey with **no** window open.
- [ ] **G15 — Select exactly one item, File→Info/Rename (Ctrl-I)** → the
  **Show-Info** dialog (`inf_file_folder`, Task #14): shows the item's
  size/date/time/attributes with an editable name; a changed name on OK →
  `Frename`, window rebuilds; Cancel / no change → screen restored. Grey unless
  exactly one item is selected. **[E]** size = byte count for a file, `(folder)`
  for a directory; date = `MM/DD/YYYY  HH:MM`; attributes list `DIR`/`R`/`H`/
  `S`/`A` as set.
- [ ] **G16 — File→Select all (a window open)** → every item in the current
  window highlights; menu state updates (Open greys at >1 selected). Grey with
  no window open.
- [ ] **G17 — Delete a desktop drive icon (desktop is current window)** → the
  three-choice alert **Cancel / Remove / Delete**; "Remove" must route to
  Options→Remove-desktop-icon (icon removal, files untouched — the handler is
  Task #14, so it is currently a no-op redirect); "Delete" deletes the drive's
  contents (dangerous — only on the throw-away test image).

## H. NEWDESK.INF read/save + Show-Info / Set-background (Task #14, deskinf)

> **Non-interactive parts already verified on `qemu raspi4b`** (serial log,
> `/tmp/sd.img`): boot **reads** a hand-crafted `C:\NEWDESK.INF` and applies
> `#E` view/sort/size-to-fit, `#Q` background (`g_patcol`), `#M`/`#T`
> desktop-icon grid positions, and the `#W` window rectangle; the
> `inf::selftest()` save→read round-trip passed 11/11 (in-memory **and** through
> the SD image). The rows below are the **interactive** dialogs, which need a
> live pointer/keyboard. Run only against a **fresh** `/tmp/sd.img` (writes
> mutate the image).

- [ ] **H1 — Boot with a `C:\NEWDESK.INF` present** → the desktop comes up with
  the file's saved view/sort (check the View menu check-marks), background
  pattern/colour, and desktop drive/trash icon positions. **[E]** menu checks +
  icon grid spots match the file.
- [ ] **H2 — Boot with NO `NEWDESK.INF`** → the desktop uses its built-in
  defaults (Icon view, name sort, drives down column 0, trash/printer at the
  right). **[E]** no error, clean boot.
- [ ] **H3 — Options→Set background** → the pattern/colour stepper dialog opens;
  `-`/`+` change the Pattern (0-7) and Colour (0-15) numbers; **OK** repaints the
  desktop with the new background **and** writes `NEWDESK.INF`; **Cancel** leaves
  it unchanged. **[E]** desktop background visibly changes on OK; re-boot keeps
  it (persisted).
- [ ] **H4 — Options→Save desktop** → writes `C:\NEWDESK.INF` from the current
  state (view/sort/background/desktop-icon positions/open windows). **[E]** no
  alert on success; an "`Could not save…`" alert only on a write failure.
- [ ] **H5 — Move a drive icon (G10), Options→Save desktop, then Options→Read
  .INF (or reboot)** → the icon returns to the saved grid cell. **[E]** the
  moved position round-trips through the file.
- [ ] **H6 — Change view (icon↔text) / sort, Options→Save desktop, reboot** →
  the desktop boots into the saved view/sort. **[E]** View-menu checks match
  what was saved.
- [ ] **H7 — Options→Read .INF file** (after an external edit or a Save) →
  re-applies the on-disk `NEWDESK.INF`: view/sort re-checked, background
  repainted, desktop icons re-placed, open windows re-laid. A missing file is a
  silent no-op.

---

## What the Round-2 audit already confirmed (do NOT re-derive on HW)

Compile clean on **both** boards, zero clippy/compiler warnings under
`kernel/src/desk/`, no stubs/`todo!`/`unwrap`/panics. Boot on `qemu raspi4b`
(QMP screendump, `/tmp/sd.img`): clean boot (no panic), menu bar (4 titles),
`C:\` window content, correctly occluded desktop drive icon, **default view =
Icon**, **folder-first name sort**, info-line format, **no label overlap at
12-char names**, frame intact, no boot residue. Shortcut table (11 entries)
matches `deskmain.c:269-307`; `men_update` live-item gating (Open/Close/Cycle)
matches `ILL_NOSEL`/`ILL_MULTSEL`/`ILL_NOWIN`/`BTOPITEM`; Task-#9 items disabled
(not stubbed); the self-clearing white item-root box erases the old view within
the `do_wredraw` clip. These are the static/screendump-verified items — the HW
rows above cover only what needs a live pointer/keyboard.
