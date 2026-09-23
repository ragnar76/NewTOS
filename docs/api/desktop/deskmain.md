# `deskmain.c` — main event loop, menu dispatch, keyboard shortcuts

> Per [`docs/SOURCE-ANALYSIS-PROTOCOL.md`](../../SOURCE-ANALYSIS-PROTOCOL.md)
> Track A and [`docs/api/_template.md`](../_template.md). The desktop is a
> GEM **application**, not a trap-dispatched syscall group, so this entry
> documents *control flow* (event loop, menu dispatch, keyboard handling,
> redraw wiring) rather than a per-function register ABI. Every claim below
> cites EmuTOS `file:line`. Source: EmuTOS `desk/deskmain.c` (2172 lines) +
> its direct helpers in `desksupp.c`/`deskwin.c`, pinned commit `1a7484ab`
> (`docs/STATUS.md`).
>
> **Contract vs. internal** (`docs/COMPATIBILITY.md`): the desktop is an
> *application* built entirely on the AES/VDI/GEMDOS contract that Phase 00
> already extracted (`docs/api/aes/`, `docs/api/vdi/`, `docs/api/gemdos/`).
> Nothing in this file is TOS-API contract in the trap-ABI sense — a program
> cannot observe *how* the desktop dispatches its own menu, only what it does
> on screen and to the filesystem. The measuring stick here is **EmuTOS
> behavioural fidelity** (per ADR-016: "reimplement the desktop behaviour"),
> not ABI compatibility. Where a subsection is genuinely AES contract
> (`evnt_multi`'s parameter block), it is marked as such and cross-referenced
> to `docs/api/aes/event.md` rather than re-derived here.

---

## EmuTOS source

**Primary location:** `desk/deskmain.c` — `deskmain()` (`:1938-2172`), the
desktop's entire startup + main loop + shutdown sequence.

**Helpers called (this file, `static`):**
- `deskmain.c:394` — `detect_features()` — probes `rez_changeable()`,
  `Blitmode(-1)`, `cache_exists()` once at startup for menu-item graying.
- `deskmain.c:438` — `desk_all(flags)` — the sort/rebuild/show pipeline
  driven by View-menu changes.
- `deskmain.c:453` — `men_list(mlist, dlist, enable)` — batch
  enable/disable of a `0`-terminated `UBYTE` item-id array.
- `deskmain.c:464` — `men_update()` — recomputes every menu item's
  enabled/disabled/checked state from current selection + window count.
- `deskmain.c:612` — `do_deskmenu(item)` — Desk-menu dispatch (About box).
- `deskmain.c:654` — `do_filemenu(item)` — File-menu dispatch.
- `deskmain.c:786` — `do_viewmenu(item)` — View-menu dispatch.
- `deskmain.c:836` — `do_optnmenu(item)` — Options-menu dispatch.
- `deskmain.c:947` — `hndl_button(clicks, mx, my, button, keystate)` —
  raw mouse-button event handler.
- `deskmain.c:1012` — `hndl_menu(title, item)` — routes an `MN_SELECTED`
  message to the four `do_*menu` functions.
- `deskmain.c:1043` — `kbd_arrow(type)` — simulates a `WM_ARROWED` message
  for the topped window from an arrow-key press.
- `deskmain.c:1067` — `check_arrow_key(thechar)`.
- `deskmain.c:1087` — `process_funkey(funkey)`, `deskmain.c:1117` —
  `check_function_key(thechar)`.
- `deskmain.c:1136` — `check_alt_letter_key(thechar)`.
- `deskmain.c:1169` — `lookup_ascii_shortcut(ascii, itemptr)`.
- `deskmain.c:1201` — `hndl_kbd(thechar)` — the keyboard message handler.
- `deskmain.c:1250` — `hndl_msg(void)` — the `MU_MESAG` (AES message pipe)
  handler; the largest of the four event handlers.
- `deskmain.c:1371` — `cnx_put(void)`, `deskmain.c:1430` — `cnx_get(void)`
  — in-memory desktop-preferences/window-layout save/restore, the staging
  area between `G.g_cnxsave` and `EMUDESK.INF`.
- `deskmain.c:1511` — `adjust_menu(obj_array)`, `deskmain.c:1755` —
  `copy_menu_items(void)`, `deskmain.c:1825` — `install_shortcuts(void)`
  (declared in `deskmain.h:35`, called from `app_start()` — see
  `deskapp.md`), `deskmain.c:1864` — `desk_xlate_fix(void)` — one-time menu
  resource geometry/string setup at startup.
- `deskmain.c:1680` — `align_objects(obj_array, nobj)`, `deskmain.c:1731`
  — `align_title(root)` (declared in `deskmain.h:34`) — dialog-title/
  translated-text alignment, run once at startup over every resource tree.
- **Missing-coverage fix**: `deskmain.c:1647-1656` —
  `adjust_3d_positions(void)` (`CONF_WITH_3D_OBJECTS`, called from
  `desk_xlate_fix`, `:1904`) — nudges two Desktop-configuration-dialog
  button objects (`DCFUNPRV`/`DCMNUPRV`) left by `3*ADJ3DSTD` "to avoid
  button overlap"; not previously cited in this file. Since NewTOS has
  `CONF_WITH_3D_OBJECTS` **On** (`docs/HARDWARE-PROFILE.md:401`), this
  fixup is in scope for a 1:1 rebuild, though it only affects one dialog
  (`ADDESKCF`) outside the redraw/event-loop core this task validates.
- **Missing-coverage fix**: `deskmain.c:1921-1931` —
  `close_desktop_windows(void)` (`CONF_WITH_READ_INF`) — was already
  described in prose under "Shutdown sequence" but not listed here as a
  helper; walks `win_ontop()` closing each window (`wind_close` +
  `dos_free(p_fbase)` + `win_free`) until none remain.
- `deskmain.c:375-390` — `play_sound(frequency, duration)`
  (`CONF_WITH_EASTER_EGG`) — builds a YM2149 register-write sequence and
  calls `Dosound`; called only from `do_deskmenu`'s About-box Easter egg.
  Out of scope (no AArch64 sound-chip equivalent; tracked as an open
  `CONF_WITH_EASTER_EGG` question below, not decided here).
- `deskmain.c:404-420` — `display_free_stack(void)`
  (`CONF_DEBUG_DESK_STACK`, debug build only) — measures unused desktop
  stack via a stack-marker scan; not previously cited, debug-only, no
  1:1-rebuild relevance.

**Helpers called (other `desk/` files — cited, not re-derived here; see
their own module spec files):**
- `desksupp.c:122` — `desk_clear(wh)` — deselect every object in a window.
- `desksupp.c:160` — `desk_clear_all(void)` — `desk_clear` over every open
  window + the desktop.
- `desksupp.c:175` — `desk_verify(wh, changed)` — the **current-window**
  state machine: sets `G.g_croot`/`G.g_cwin`/`G.g_wlastsel`; if `changed`,
  rebuilds the window's view via `win_bldview` first.
- `desksupp.c:202` — `do_wredraw(w_handle, gptr)` — the **damage-rect-list
  redraw** (see [Redraw wiring](#redraw-wiring-desk_all--do_wredraw) below).
- `desksupp.c:262` — `do_xyfix(px, py)` — window position snap (16-px
  horizontal grid, below the menu bar).
- `desksupp.c:1840`/`:1849` — `desk_busy_on()`/`desk_busy_off()` —
  hourglass/arrow mouse form (`graf_mouse`).
- `deskwin.c:63` — `win_view(void)` — recomputes `G.g_iwext/g_ihext/
  g_iwint/g_ihint/g_iwspc/g_ihspc` (and, `CONF_WITH_SIZE_TO_FIT`,
  `G.g_icols`) from `G.g_iview`.
- `deskwin.c:105` — `win_start(void)`, `deskwin.c:156` — `win_alloc(obid)`,
  `deskwin.c:198` — `win_find(wh)`, `deskwin.c:218` — `win_top(thewin)`,
  `deskwin.c:247`/`:265` — `win_onbottom`/`win_ontop`, `deskwin.c:280` —
  `win_count(void)`, `deskwin.c:392` — `win_bldview(pwin, r)`,
  `deskwin.c:682` — `win_slide`, `deskwin.c:716` — `win_arrow`,
  `deskwin.c:768`/`:786`/`:805` — `win_srtall`/`win_bdall`/`win_shwall`,
  `deskwin.c:825` — `win_isel`.
- `deskact.c:566` — `act_bsclick(...)`, `deskact.c:609` —
  `act_bdown(...)` — the click/drag state machine `hndl_button` drives.
- `desksupp.c:1191` — `do_open(pwin, curr)`, `desksupp.c:1288` —
  `do_info(curr)`, `desksupp.c:422` — `do_diropen(...)`,
  `desksupp.c:1733` — `i_find(wh, item, ppf, pisapp)`,
  `desksupp.c:1810` — `valid_drive(drive)`.
- `deskfun.c:101` — `fun_msg(type, w3..w7)` — pushes a synthetic message
  into `G.g_rmsg`-shaped call (used by `win_shwall`, `BTOPITEM`, the
  shrink/redraw fixup in `hndl_msg`).
- `deskfun.c:560/607/639/680/1302/1376` — `fun_search`/`fun_selectall`/
  `fun_mask`/`fun_mkdir`/`fun_drag`/`fun_del`.
- `deskfpd.c:335` — `pn_count(pw, psel, papp)`.
- `deskobj.c:249` — `obj_get_obid(drive)`.
- `deskapp.c:1337` — `app_blddesk(void)`, `deskapp.c:781` —
  `app_start(void)`, `deskapp.c:1106` — `app_save(todisk)`,
  `deskapp.c:1492` — `app_read_inf(void)` (`CONF_WITH_READ_INF`),
  `deskapp.c:439` — `app_tran(bi_num)`.
- `deskrez.c:316` — `change_resolution(newres, newmode)`.
- AES binds (`aesbind.h`): `evnt_multi` (`:37-41`), `menu_ienable`,
  `menu_icheck`, `menu_tnormal`, `menu_bar`, `graf_mouse`, `graf_mkstate`,
  `wind_get`/`wind_set` (+ the `_grect` wrappers), `objc_draw`, `form_do`,
  `appl_init`/`appl_exit`, `graf_handle`, `Keytbl`, `Dosound` (XBIOS),
  `Supexec`, `Blitmode`.

**Headers / type definitions used:**
- `include/aesdefs.h:227-261` — `MN_SELECTED=10`, `WM_REDRAW=20` …
  `WM_MOVED=28`, `MU_KEYBD=0x0001`, `MU_BUTTON=0x0002`, `MU_MESAG=0x0010`,
  `MU_TIMER=0x0020`, `WA_UPPAGE=0` … `WA_RTLINE=7`.
- `include/scancode.h:31-50` — `ARROW_UP/DOWN/LEFT/RIGHT`,
  `SHIFT_ARROW_*`, `FUNKEY_01`/`FUNKEY_10`/`FUNKEY_11`/`FUNKEY_20`.
- `desk/deskbind.h:130-245` — `GLOBES` (the `G` struct — every field
  `deskmain.c` reads/writes is defined here; see `_state.md`, produced by
  the sibling task, for the full field-by-field account).
- `desk/desktop.def` — the resource item-id constants (`ADMENU`,
  `DESKMENU`/`FILEMENU`/`VIEWMENU`/`OPTNMENU`, `OPENITEM` … `CACHITEM`);
  generated into `obj/header.h` at build time by the resource compiler —
  **not present as source** in `references/emutos/` (build artifact).
  `deskmain.c:87-88` derives `TITLE_ROOT=(DESKMENU-1)`/
  `ITEM_ROOT=(ABOUITEM-1)` from the assumption that these are the first
  title/item objects in the `ADMENU` tree.
- `desk/deskmain.h` — `CHAR_WIDTH=8`, `SHORTCUT_SIZE=3`,
  `NUM_SHORTCUTS=33`, the `hndl_msg`/`deskmain`/`align_title`/
  `install_shortcuts` prototypes.

**System variables / globals read (this file):** `G.g_cwin`, `G.g_croot`,
`G.g_wlastsel`, `G.g_rmsg[8]`, `G.g_iview`, `G.g_isort`, `G.g_ifit`
(`CONF_WITH_SIZE_TO_FIT`), `G.g_desk`, `G.g_ahead` (ANODE chain, function-key
lookup), `G.g_blitter`, `G.g_cache`, `G.g_stdrv`, `menu_shortcuts[]`,
`shortcut_mapping[]` (`const`), `desk_rs_trees[ADMENU]`, `gl_apid`,
`gl_amstr`/`gl_pmstr`, `enable_ceh` (`bios` global — the graphical critical
error handler on/off switch), `restart_desktop` (`CONF_WITH_READ_INF`).

**System variables / globals written:** `G.g_cwin`, `G.g_croot`,
`G.g_wlastsel` (via `desk_verify`, not directly), `G.g_iview`, `G.g_isort`,
`G.g_ifit`, `G.g_stdrv` (`= dos_gdrv()` once, at startup), `G.g_rmsg[0]=0`
(message consumed, `deskmain.c:1355`), `menu_shortcuts[]` (copied from
`default_shortcuts[]` at startup, `deskmain.c:2013`, and installed into menu
text by `install_shortcuts`), `G.g_cnxsave` (allocated + filled by `cnx_put`),
`enable_ceh` (`= TRUE` once the menu bar is up, `deskmain.c:2082`; `= FALSE`
just before shutdown, `deskmain.c:774`), `restart_desktop`
(`CONF_WITH_READ_INF`).

**Build configuration affecting this file:** `CONF_WITH_SEARCH`,
`CONF_WITH_BOTTOMTOTOP`, `CONF_WITH_SELECTALL`, `CONF_WITH_FILEMASK`,
`CONF_WITH_FORMAT`, `CONF_WITH_EJECT`, `WITH_CLI`, `CONF_WITH_SHUTDOWN`,
`CONF_WITH_SIZE_TO_FIT`, `CONF_WITH_BACKGROUNDS`, `CONF_WITH_DESKTOP_CONFIG`,
`CONF_WITH_READ_INF`, `CONF_WITH_BLITTER`, `CONF_WITH_CACHE_CONTROL`,
`CONF_WITH_EASTER_EGG`, `CONF_WITH_PRINTER_ICON`, `CONF_WITH_3D_OBJECTS`,
`CONF_WITH_ALT_DESKTOP_GRAPHICS`, `CONF_WITH_VIDEL`, `CONF_WITH_ATARI_VIDEO`,
`CONF_WITH_PCGEM`, `CONF_WITH_DESKTOP_SHORTCUTS`. See
[Configuration / version notes](#configuration--version-notes) for the
per-flag gate and the NewTOS decision (where one is already on record) —
**several flags below have no recorded NewTOS decision yet; flagged as open
questions**, per CLAUDE.md Rule 3 (STOP on uncertainty) rather than guessed.

---

## Source extraction summary

`deskmain()` (`deskmain.c:1938-2172`) is the desktop process's `main()`:
it initializes the AES connection (`appl_init`), loads and fixes up the
desktop's own resource (menus + dialogs), builds the desktop's icon/window
state from `EMUDESK.INF` or built-in defaults (`app_start`), takes over the
desktop window (`wind_set(DESKWH, WF_NEWDESK, ...)`), installs the menu bar,
then runs a single event loop until a menu action requests exit (Shutdown,
or — `CONF_WITH_READ_INF` — a successful "Read .INF file" that must restart
the whole process). The loop blocks in `evnt_multi()` for keyboard, mouse
button, or AES system-message events, dispatches whichever fired, then
drains any *additional* queued messages with a second, message-only
`evnt_multi()` before blocking again. Every event handler ends by calling
`men_update()` to re-enable/disable menu items for the new selection/window
state, **except** the `WM_REDRAW` case (menu items don't change on a mere
redraw — `deskmain.c:1357`, `if (!menu) men_update();`).

Menu dispatch is a **two-level switch**: `hndl_menu(title, item)`
(`deskmain.c:1012`) routes on the *menu title* (`DESKMENU`/`FILEMENU`/
`VIEWMENU`/`OPTNMENU`) to one of four `do_*menu(item)` functions, each a
flat `switch` on the *item* id. Three of the four dispatchers act directly
and return a `done`/`WORD` "exit the desktop" flag (`do_deskmenu`,
`do_filemenu`, `do_optnmenu`); `do_viewmenu` instead returns a **bitmask**
of what changed (`VIEW_HAS_CHANGED`/`SORT_HAS_CHANGED`/
`BACKGROUND_HAS_CHANGED`/`SIZETOFIT_HAS_CHANGED`), and `hndl_menu` feeds
that bitmask into `desk_all(rc)` — the one place View-menu changes turn into
an actual window rebuild + redraw. Every dispatch path (mouse-selected menu,
keyboard shortcut) ends by un-highlighting the menu title
(`menu_tnormal(desk_rs_trees[ADMENU], title, 1)`, `deskmain.c:1036`,
`:1240`).

Keyboard shortcuts are handled *without* going through the AES menu message
path at all: `hndl_kbd(thechar)` (`deskmain.c:1201`) checks, in order, for
CR (deselect all), ESC (refresh top window), an arrow key (simulate
`WM_ARROWED` on the topped window), a function key (F1-F20, launch the
matching ANODE's application), Alt+A..Z (open that drive letter if present),
then falls through to a Ctrl-character menu-shortcut lookup
(`lookup_ascii_shortcut`) that maps the character to a `(title, item)` pair
and calls `hndl_menu` directly — the same function the mouse path calls,
so a keyboard shortcut and a mouse menu-click are indistinguishable once
dispatched.

The message-pipe handler `hndl_msg()` (`deskmain.c:1250`) is a flat switch
on `G.g_rmsg[0]` (the AES message type) with `G.g_rmsg[3]` as the window
handle for every window-manager message. It is the only place `desk_verify`
is called with `changed=TRUE` for `WM_FULLED`/`WM_SIZED` (a size change
requires rebuilding the window's item grid) — `WM_MOVED` and `WM_TOPPED`
call it with `changed=FALSE` (position doesn't affect the item grid).
`WM_REDRAW` does **not** call `desk_verify` at all; it calls `do_wredraw`
directly with the **exact damage rectangle the AES sent**
(`(GRECT *)&G.g_rmsg[4]`), because a redraw never changes desktop state,
only what's on screen.

---

## The main loop (`deskmain`, `deskmain.c:2077-2123`)

### `evnt_multi` usage (contract: see `docs/api/aes/event.md`)

The outer loop's blocking call:

```
flags = MU_BUTTON | MU_MESAG | MU_KEYBD;
...
ev_which = evnt_multi(flags, 0x02, 0x01, 0x01,
                        0,0,0,0,0, 0,0,0,0,0,
                        G.g_rmsg, 0, 0,
                        &mx, &my, &button, &kstate, &kret, &bret);
```
(`deskmain.c:2078-2091`)

- `flags` requests all three of keyboard, button, and message events —
  **never** mouse-rect events (`MU_RECT`) or a timer (`MU_TIMER`) in the
  *outer* loop.
- `bclk=0x02, bmsk=0x01, bst=0x01` — the button parameters mean "fire on
  any button transition matching mask `0x01`" (left button) "in state
  `0x01`" (down) with click-count reporting (`bclk=2` = report up to a
  double click) — see `docs/api/aes/event.md` for the exact `evnt_multi`
  slot semantics; this is AES contract, not desktop-specific.
- The message buffer is `G.g_rmsg` (8 `WORD`s — the fixed AES message
  size); the desktop **owns this buffer for its entire lifetime** (it's a
  `GLOBES` field, not a stack local), so a message handler can stash a
  value from it across calls (only actually done via `handle =
  G.g_rmsg[3]` inside `hndl_msg`).
- The returned `ev_which` is a bitmask of which classes actually fired,
  independent per class (a button press *and* a queued message can both be
  reported from one call).

### Per-iteration dispatch (`deskmain.c:2085-2122`)

Each iteration: `wind_update(BEG_UPDATE)` (grab the screen — AES contract,
prevents other AES clients from drawing while the desktop paints), dispatch
whichever of `MU_KEYBD`/`MU_BUTTON`/`MU_MESAG` fired, then
`wind_update(END_UPDATE)`. `MU_KEYBD` → `hndl_kbd(kret)`; `MU_BUTTON` →
`hndl_button(bret, mx, my, button, kstate)` (`bret` = click count: 1 or 2).
Either returning nonzero sets `done=TRUE`, ending the outer loop **after**
the current iteration finishes (not immediately — `MU_MESAG` is still
processed if it also fired).

### The message-drain sub-loop (`deskmain.c:2107-2118`)

```
while (ev_which & MU_MESAG)
{
    if (done) break;
    if (hndl_msg()) done = TRUE;
    ev_which = evnt_multi(MU_MESAG | MU_TIMER, 0x02, 0x01, 0x01,
                        0,0,0,0,0, 0,0,0,0,0,
                        G.g_rmsg, 0, 0,
                        &mx, &my, &button, &kstate, &kret, &bret);
}
```

After handling one message, the loop **immediately** issues a second,
**non-blocking-in-practice** `evnt_multi` requesting only `MU_MESAG |
MU_TIMER` with a **zero timeout is not specified here** — the comment in
the code calls this the "quick-out to clean out all messages": because
`MU_TIMER` is requested with no timer actually armed (`evnt_timer` never
called in this path), this call returns as soon as either another message
is already queued or (implicitly) not at all until one arrives — in
practice the AES message pipe is drained one message at a time until empty,
each iteration re-entering `hndl_msg`. This is how a single user action that
produces *multiple* AES messages (e.g. `win_shwall()` issuing one
`WM_REDRAW` per open window, `deskwin.c:816`) gets fully processed inside
one outer-loop pass before the desktop blocks again waiting for new input.

### Startup sequence (`deskmain.c:1938-2082`)

1. `G.g_stdrv = dos_gdrv()` — remember the boot drive.
2. `gl_apid = appl_init()` — become an AES client.
3. `gl_handle = graf_handle(&gl_wchar,&gl_hchar,&gl_wbox,&gl_hbox)` — the
   VDI handle + character/box cell metrics used throughout the desktop's
   own layout math (`adjust_menu`, icon-grid spacing in `deskapp.c`).
4. `wind_get_grect(DESKWH, WF_WXYWH, &G.g_desk)` — the desktop work area
   (below the menu bar) becomes the reference rect for every later layout
   computation.
5. `desk_busy_on()` bracketed by `wind_update(BEG/END_UPDATE)` — hourglass
   cursor while resources load.
6. `detect_features()` (`:394`) — one-time hardware probes.
7. Allocate + zero `G.g_cnxsave` (`CSAVE`) — `nomem_alert()` (infinite
   loop) on failure; **same pattern repeats** for every subsequent
   allocation failure in this function (resource objects, desktop
   strings, window nodes) — the desktop has **no recoverable
   out-of-memory path at startup**, only a hang with (presumably) an
   alert box drawn once before the infinite loop.
8. `desk_rs_init()` (copies ROM `OBJECT`/`TEDINFO` resource to RAM) then
   `desk_xlate_fix()` (`:1864` — translation strings, `copy_menu_items`,
   version-string insertion, `adjust_menu`, `rsrc_obfix` per object,
   `CONF_WITH_3D_OBJECTS` position tweaks, `align_objects`).
9. `align_title()` over every resource tree (`RS_NTREE`); `app_tran()`
   over every built-in bit image (`RS_NBB`).
10. Copy `gl_amstr`/`gl_pmstr` (AM/PM strings) out of the resource — done
    here (not in `dr_code`, the `G_USERDEF` draw callback) because
    `dr_code` cannot re-enter the AES to look up a resource string.
11. `menu_shortcuts[] = default_shortcuts[]` (memcpy) — the **live**
    shortcut table, later possibly overwritten per-item by `app_start()`
    (custom `EMUDESK.INF` shortcuts) before `install_shortcuts()` bakes
    them into the menu item text.
12. `app_start()` — load icons/apps from `EMUDESK.INF` or built-in
    defaults (see `deskapp.md`).
13. `install_shortcuts()` (`:1825`) — writes `^X`/`  ` into each menu
    item's already-ROM-to-RAM-copied text, 3 bytes before the item's
    right edge (`SHORTCUT_SIZE=3`, "`^X `" or "`  `").
14. `win_start()` — allocate `NUM_WNODES` `WNODE`s, call `win_view()`
    (initializes `G.g_iview`/`g_isort`/grid metrics from `START_VIEW`/
    `START_SORT` — see `deskwin.md`).
15. `desk_verify(DESKWH, FALSE)` — initializes `G.g_croot=DROOT`,
    `G.g_cwin=DESKWH`, `G.g_wlastsel=DESKWH`.
16. Check the current view/sort menu items (`menu_icheck`).
17. `app_blddesk()` — build the desktop's own icon object tree
    (`deskapp.md`).
18. `wind_set(DESKWH, WF_NEWDESK, G.g_screen, 1, 0)` — **hand the desktop
    surface's object tree to the AES**; from this point the AES's own
    window-manager redraw of `DESKWH` uses `G.g_screen`.
19. `cnx_get()` (`:1430`, bracketed by `wind_update`) — re-open any windows
    saved in `G.g_cnxsave` (from a prior `cnx_put`, e.g. across a
    "Read .INF file" restart) at their saved position/size/scroll.
20. `CONF_WITH_BLITTER`: re-apply `Blitmode` — done again here (not only
    in `geminit.c`) because an AUTO-folder program may have changed blitter
    state before the desktop started.
21. `men_update()` once, then `menu_bar(menutree, 1)` (install the menu
    bar — bracketed by `wind_update`, `desk_busy_off()` inside the
    bracket) — **the menu bar is not visible until this call**, i.e.
    everything above happens with no menu bar on screen.
22. `enable_ceh = TRUE` — only now does the graphical critical-error
    handler become active; a disk error during startup would not show the
    graphical alert.

### Shutdown sequence (`deskmain.c:2124-2172`)

- **Normal exit** (Shutdown selected, `enable_ceh=FALSE` set first at
  `do_filemenu` `QUITITEM`, `:774`): `cnx_put()` (save state to
  `G.g_cnxsave`), `app_save(FALSE)` (do **not** force a disk write —
  `EMUDESK.INF` is only written back if it was already dirty; see
  `deskapp.md`), `rc = TRUE`.
- **`CONF_WITH_READ_INF` restart path** (`app_read_inf()` returned success
  from `READITEM`, `:919-925`, setting `restart_desktop=TRUE`): close every
  open window (`close_desktop_windows`, `:1921-1931` — `wind_close` +
  free the FNODE array + free the WNODE), then free **every** desktop
  allocation (`G.g_wlist`, `g_screen`, `g_iblist`, `g_alist`, `g_atext`,
  `g_cnxsave`, `desk_rs_obj`, `desk_rs_strings`) and return `rc=FALSE` — the
  caller (`_deskstart`, `deskstart.S`) re-enters `deskmain()` from scratch.
- Either way: `menu_bar(NULL, 0)` (remove the menu bar), `wind_set(DESKWH,
  WF_NEWDESK, NULL, 0, 0)` (give up the desktop surface), `appl_exit()`
  (leave the AES); on the restart path only, `wind_new()` (AES-side window
  cleanup) before returning.

---

## Menu bar setup + dropdown / menu-item dispatch

### Static resource fixup (once, at startup)

`desk_xlate_fix()` (`:1864-1914`) calls, in order: `xlate_obj_array`
(translate every string in `desk_rs_obj`), `copy_menu_items()` (`:1755`),
version-string + about-box insertion, `adjust_menu(desk_rs_trees[ADMENU])`
(`:1511`), `rsrc_obfix` per object, `CONF_WITH_3D_OBJECTS` adjustment,
`align_objects`.

- **`copy_menu_items()`** (`:1755-1814`): for every drop-down under
  `ADMENU`, computes `max_width` (longest item text) `+ 1 + SHORTCUT_SIZE`
  and allocates a single RAM block (`desk_rs_strings`) sized to hold every
  non-separator item's text **padded with spaces to that drop-down's
  `max_width`**; each item's `ob_spec` is repointed at its slice of that
  block, and `ob_width` becomes `max_width` for **every** item (including
  separators). Failure returns `-1` (checked by the caller,
  `nomem_alert()`).
- **`adjust_menu()`** (`:1511-1641`): computes `ob_x`/`ob_width` for every
  menu title from `strlen(ob_spec)` (assumes the ROM's fixed system-font
  `CHAR_WIDTH=8`px — comment explicitly notes this is a *documented*
  `lineavars.h` assumption, not computed from the actual font); if the
  title bar is too wide for the screen (`mbar->ob_width >= width`, screen
  width in **characters**), shrinks every title by half a character
  (byte/`CHAR_WIDTH/2` sub-pixel trick, `ob_x |= (CHAR_WIDTH/2)<<8`) — this
  exists **specifically for the French desktop** (comment, `:1546-1548`)
  where the translated title bar is slightly wider than ST-Low screen
  width; if *still* too wide, truncates the last heading. Then, per
  drop-down: finds the widest item, forces `dropbox->ob_x` byte-aligned
  (`& 0x00ff`) "for faster display", builds the separator-line string by
  pointing `ob_spec` at an offset into a shared 40-dash `separator[]`
  buffer sized to exactly that drop-down's width, and — if the drop-down
  would extend off the right edge of the screen — repositions it and
  encodes a sub-pixel width adjustment the same way as the title-bar case.
  **`MIN_DESKMENU_WIDTH=20`** chars is the *minimum* Desk-menu width,
  "compatible with Atari TOS" (comment, `:80-81` — i.e. a deliberate
  historical-compatibility floor unrelated to content).
  **`THEACTIVE` (A100/A102, 2026-09-21):** the title loop ends with
  `mbar->ob_width = x` (`:1534`) — `THEACTIVE` is exactly the titles' sum,
  not the screen width (it is the rectangle `menu_bar` hands the Control
  Manager as `gl_ctwait`, `aes/gemmnlib.c:717`) — and its `ob_x` stays the
  RSC's 2 character cells (`desk_rsc.c:255-259`, `{2, 0, 28, 769}`) unless
  the bar overflows (`:1584-1589`, then 1): "the menu bar cannot start at
  character offset 0 because, in that case, the outline of the Desk dropdown
  box will not be drawn properly" (`:1537-1541`). Every ADMENU drop-down box
  carries `ob_spec = 0xFF1100` (`desk_rsc.c:294,348,450,528`): thickness
  byte `0xFF` = `th = -1`, a 1-px border drawn **outside** the box
  rectangle, border/text BLACK, `IP_HOLLOW` interior (painted white by
  `gr_rect`); the entries sit at `(0, row)` spanning the box's width
  (`desk_rsc.c:297-341`), so a hover's item XOR (`ob_change`,
  `gemoblib.c:1295-1304`) never reaches the border and nothing repaints it.
  Each box's `ob_x` is `THEACTIVE.ob_x + title.ob_x` (`:1617`).
  **Widths and the bar box (A108, 2026-09-21):** `adjust_menu` runs before
  `rsrc_obfix` (`:1895-1900`), so its units are character cells of
  `CHAR_WIDTH` = 8 px (`:1509`). Each title is exactly `strlen(ob_spec)`
  cells wide and laid left to right (`title->ob_x = x; title->ob_width =
  n; x += n`, `:1528-1533`) — no padding, the blanks in `" Desk "` are the
  gaps; on the English tree the titles are 6/6/6/9 cells (48/48/48/72 px)
  at 0/48/96/144 and `THEACTIVE` is 216 px wide at x = 16. Each drop-down
  is `m + 1` cells wide where `m` is the widest entry (`:1608-1614`) and
  the `+ 1` is the `m++` of the separator loop (`:1620`); the Desk box's
  search is seeded at `MIN_DESKMENU_WIDTH - 1` (`:1602`), so it is never
  under 20 cells (160 px). Every entry of a box gets the box's width
  (`item->ob_width = m`, `:1633-1635`); a box that would reach the right
  screen edge (`ob_x + m >= width`) is moved to `width - m` and made
  `(m - 1) | ((CHAR_WIDTH - 1) << 8)` = `m · 8 - 1` px wide so its outside
  border lands on the last column (`:1626-1631`). `THEBAR` is the RSC's
  `{0, 0, 80, 513}` (`desk_rsc.c:249-253`): `ob_height = 0x0201` = one cell
  + 2 px = `gl_hchar + 2` = 18 after `rsrc_obfix` (`aes/gemrslib.c:76-82`),
  one row above the separator `mn_bar` draws on `gl_hbox - 1` = 18
  (`aes/gemmnlib.c:718-722`); `THEACTIVE`, the titles and `THEMENUS`'s `y`
  are `769` = `gl_hchar + 3` = `gl_hbox` = 19 (`desk_rsc.c:255-289`).
  **Entry height (A109, 2026-09-22):** a drop-down entry is `{0, row, 20, 1}`
  (`desk_rsc.c:297-341`) and its box `{x, 0, w, n}` (`:291-295,345-349`) —
  character cells, which `rsrc_obfix` multiplies by `gl_hchar`
  (`fix_chpos` offsets 1 and 3, `aes/gemrslib.c:53-99`) = **16 px** on the
  8×16 font, not the 19 px of the bar row (`gl_hbox` = `gl_hchar + 3`,
  `aes/gemgraf.c:412-415`). `adjust_menu` rewrites widths only
  (`:1608-1635`), so every entry sits at `row · 16` and every box is
  `n · 16` tall: on the English tree Desk 32, File 240, View 160, Options
  192 (measured on the board, `measurement-2026-09-22-a107-a109`). NewTOS's
  `build_menu` used `MENU_H` = 19 until A109, which made the File box 285.
- **`install_shortcuts()`** (`:1825-1858`, called from `app_start()` after
  `menu_shortcuts[]` may have been customized): for each of the
  `NUM_SHORTCUTS=33` shortcut slots, if `shortcut_mapping[i]` is `0` (item
  doesn't exist in this build — `#if`'d out), **clears** `menu_shortcuts[i]`
  too, so the live shortcut table always matches what's actually
  displayed; otherwise writes `^X` (or two spaces, blanking a previously
  set shortcut) 3 bytes before the item text's right edge
  (`ob_width` is already **pixels** at this point:
  `(ob_width+CHAR_WIDTH-1)/CHAR_WIDTH - SHORTCUT_SIZE`).
- **`align_title()`** (`:1731-1750`, exported, called once per tree in
  `RS_NTREE`): if object 1 of a tree is a `G_STRING` at
  `ob_y == gl_hchar` (one char-height down — the desktop's convention for
  "this object is the dialog title"), centres it (or, `CONF_WITH_
  ALT_DESKTOP_GRAPHICS`, left-aligns + underlines) — a *heuristic*
  position-based test, not an explicit "this is the title" flag.

### Dispatch tables (`file:line` = where the `#define`/array lives)

- **`shortcut_mapping[NUM_SHORTCUTS]`** (`:169-259`, `const`) — index →
  menu-item id, `0` if the item is compiled out. **Order defines the
  shortcut-slot semantics**; `deskmain.c:166-168` warns any change here
  must stay in sync with `tools/draftexc.c`'s `exclude_items[]`.
- **`default_shortcuts[NUM_SHORTCUTS]`** (`:269-307`) — the built-in
  Ctrl-key per slot (uppercase display char; actual match is the control
  character, `ascii|0x40` in `lookup_ascii_shortcut`). **Correction** (the
  prior "9 of 33" count was wrong — verified by counting every entry in
  `deskmain.c:269-307`): **22 of 33** slots are `0x00` (no default
  shortcut) — in the File menu: `SRCHITEM`, `MASKITEM`, `FORMITEM`,
  `QUITITEM` (4 of 13); **every** View-menu item (9 of 9:
  `ICONITEM`…`BACKGRND`); and every Options-menu item **except**
  `RESITEM`('R') and `SAVEITEM`('S') (9 of 11). The 11 slots that *do*
  have a default shortcut are `OPENITEM`('O'), `SHOWITEM`('I'),
  `NFOLITEM`('N'), `CLOSITEM`('H'), `CLSWITEM`('U'), `BTOPITEM`('W'),
  `SLCTITEM`('A'), `DELTITEM`('D'), `CLIITEM`('Z'), `RESITEM`('R'),
  `SAVEITEM`('S').
- **`arrow_table[]`** (`:316-325`) — scancode → `WA_*` pairs,
  `0`-terminated, consulted by `check_arrow_key`.

### `hndl_menu(title, item)` (`:1012-1039`) — the top-level menu switch

Four cases on `title` (`DESKMENU`/`FILEMENU`/`VIEWMENU`/`OPTNMENU`) each
call the matching `do_*menu(item)`; for `VIEWMENU` **only**, the return
value (`VIEW_HAS_CHANGED|SORT_HAS_CHANGED|SIZETOFIT_HAS_CHANGED|
BACKGROUND_HAS_CHANGED`, `:93-96`) feeds `desk_all(rc)` if nonzero — this is
the **only** path from a menu action to a window rebuild in this file; the
other three dispatchers act immediately, no batching. Every call
un-highlights the title afterward (`menu_tnormal(...,title,1)`, `:1036`).

- **`do_deskmenu(item)`** (`:612-651`): a single case, `ABOUITEM` — opens
  the About dialog with `start_dialog`/`form_do` loop; a click on the
  desktop-icon object inside the dialog (`DEICON`) plays the Easter-egg
  tune (`CONF_WITH_EASTER_EGG`) and loops (`form_do` again) instead of
  closing; any *other* touched object ends the dialog.
- **`do_filemenu(item)`** (`:654-783`): `pw = win_ontop()`, `curr =
  win_isel(G.g_screen, G.g_croot, 0)` (first selected icon in the current
  root) computed **once**, shared across the whole switch. `OPENITEM` only
  acts `if (pw || curr)`. `SHOWITEM` with `curr` loops
  `do_info(curr)`/`win_isel` over **every** selected icon, then
  `fun_rebld_marked()`; with no selection, shows the *disk* info for the
  topped window's drive (`inf_disk(pw->w_pnode.p_spec[0])`) — **matches
  Atari TOS's behaviour** (comment, `:682-683`). `CLSWITEM` = close the
  window entirely (`CLOSE_WINDOW`); `CLOSITEM` = close back toward the
  root folder (`CLOSE_FOLDER`) — **two different close semantics**, not a
  duplicate. `CLIITEM` (`WITH_CLI`) launches `EmuCON`
  (`pro_run(FALSE, DEF_CONSOLE, ...)`); on return, `CONF_WITH_PCGEM`
  additionally sets the shell's default directory to the topped window's
  path via `shel_wdef`. `QUITITEM` (`CONF_WITH_SHUTDOWN`) disables the
  critical-error handler *first* ("avoid possibility of useless
  `form_alert()`s", `:774`) then calls `shel_write(SHW_SHUTDOWN,...)` and
  sets `done=TRUE`.
- **`do_viewmenu(item)`** (`:786-833`): `ICONITEM`/`TEXTITEM` — no-op if
  already the current view; else flip the menu checkmarks
  (`menu_icheck` off old, on new), set `G.g_iview`, call `win_view()`
  (recompute grid metrics — **not** a window rebuild itself), return
  `VIEW_HAS_CHANGED`. `NAMEITEM..NSRTITEM` — identical pattern for
  `G.g_isort`, returns `SORT_HAS_CHANGED`. `FITITEM`
  (`CONF_WITH_SIZE_TO_FIT`) — **toggles** (no "already selected" guard,
  unlike view/sort), returns `SIZETOFIT_HAS_CHANGED`. `BACKGRND`
  (`CONF_WITH_BACKGROUNDS`) — opens the background-picker dialog
  (`inf_backgrounds()`), returns `BACKGROUND_HAS_CHANGED` only if the user
  actually changed something (dialog return value).
- **`do_optnmenu(item)`** (`:836-944`): `IDSKITEM`/`IAPPITEM`/`IICNITEM`/
  `RICNITEM` all call an `ins_*`/`rmv_icon` function (see `deskins.md`)
  and conditionally rebuild the desktop object tree
  (`app_blddesk()`) — **note the differing rebuild trigger per item**:
  `IDSKITEM`/`RICNITEM` rebuild `if (rebld)` (any nonzero), `IICNITEM`
  rebuilds only `if (rebld > 0)` and instead does a **window** rebuild
  (`win_bdall`/`win_shwall`, no `app_blddesk`) `if (rebld < 0)` — the sign
  of the return value distinguishes "desktop icon changed" from "window
  icon changed" for icon installs specifically (`CONF_WITH_WINDOW_ICONS`).
  `PREFITEM` → `if (inf_pref()) desk_all(FALSE);` (`deskmain.c:885-887`) —
  **conditional**, not unconditional: `inf_pref` returns `TRUE` only when the
  view is `V_TEXT` *and* the time or date format changed (`deskinf.c:807-814`),
  and only then is `desk_all` called. *(Erratum, DESK02: this paragraph
  previously said "unconditionally". Re-read against the pinned source
  `references/emutos/desk/deskmain.c:885-887`.)* The argument is `FALSE`, i.e.
  `flags = 0`, which skips both `if`s inside `desk_all` and calls only
  `win_shwall()` between busy-on/off — a **redraw** of every window, not a
  rebuild, because EmuTOS's text lines are formatted at draw time by the
  `G_USERDEF` `dr_fnode` (`deskinf.c:310-326`) and so pick the new format up
  from a repaint alone. `SAVEITEM` → `cnx_put()` +
  `app_save(TRUE)` (force disk write), bracketed by `desk_busy_on/off`.
  `RESITEM` → `change_resolution`; on success (`rebld==1`), issues
  `shel_write(SHW_RESCHNG,...)` (Falcon/Videl or Atari-video variant,
  `CONF_WITH_VIDEL`/`CONF_WITH_ATARI_VIDEO`) and sets `done=TRUE` — **a
  resolution change ends the desktop process** (it restarts at the new
  resolution; contrast with `IDSKITEM` etc., which rebuild in place).
  `BLITITEM`/`CACHITEM` toggle + immediately apply
  (`Blitmode`/`Supexec(desktop_set_cache)`).

### `men_update()` (`:464-609`) — menu item enable/disable/check state

Runs after **every** dispatched event except a bare redraw. Algorithm:
1. Re-enable every item, but **disable every separator** (`ob_spec[0]=='-'`
   — the same convention `adjust_menu`'s separator-string setup relies on).
2. Count selected icons (`nsel`), of which how many are applications
   (`napp`), desktop/disk-type icons (`ndesk`), trash/printer icons
   (`ntrash`) — **the desktop pseudo-window (`G.g_cwin==DESKWH`) is walked
   icon-by-icon via `win_isel`/`i_find`**; a **real** window instead calls
   `pn_count(pw,&nsel,&napp)` (a different, FNODE-based counting path —
   see `deskfpd.md`). `nwin = win_count()`.
3. `ntrash` → disable `DELTITEM` (can't delete the trash/printer icon
   itself). `!napp` → disable `IAPPITEM`. `!ndesk` → disable `RICNITEM`.
4. `nsel` switch: `0` → disable `ILL_NOSEL[]` (`OPENITEM`, `DELTITEM`) and,
   **only if also `!nwin`**, additionally disable `SHOWITEM`/`SRCHITEM`
   ("like Atari TOS, disabled only if there are neither open windows nor
   selected icons", comment `:549-551`); `1` → no change; `>1` → disable
   `ILL_MULTSEL[]` (`OPENITEM` only — you can't "Open" two things at once,
   but you *can* still "Delete" or "Search" multiple).
5. `nwin` switch: `0` → disable `ILL_NOWIN[]` (`NFOLITEM`/`CLOSITEM`/
   `CLSWITEM`/`SLCTITEM`/`MASKITEM`, config-gated) **and** (fallthrough)
   `1` → additionally disable `BTOPITEM` (`CONF_WITH_BOTTOMTOTOP`) — i.e.
   `BTOPITEM` needs **two or more** windows, not just one.
6. `QUITITEM` enabled iff `can_shutdown()`; `RESITEM` iff
   `can_change_resolution` (cached from `detect_features()` at startup —
   **not re-probed per menu open**). `BLITITEM`/`CACHITEM`: enabled +
   checkbox reflects `G.g_blitter`/`G.g_cache` iff the hardware feature was
   detected at startup, else force-disabled (+ `CACHITEM` force-unchecked).

---

## Mouse button handling (`hndl_button`, `deskmain.c:947-1009`)

**Gap filled in this validation pass** — the prior extraction only listed
`hndl_button` in the "Helpers called" table (one line); the task's special
focus on the click path requires the exact call sequence, so it is spelled
out here, verified against `deskmain.c:947-1009`.

```c
static WORD hndl_button(WORD clicks, WORD mx, WORD my, WORD button, WORD keystate)
{
    wh = wind_find(mx, my);                          /* :955 */
    if (wh != G.g_cwin)
        desk_clear_all();                            /* :957-958 */
    desk_verify(wh, FALSE);                           /* :960 */
    wind_get_grect(wh, WF_WXYWH, &c);                 /* :962 */

    if (clicks == 1) {
        act_bsclick(G.g_cwin, G.g_croot, mx, my,
                    keystate, &c, FALSE);              /* :968-969 */
        graf_mkstate(&junk, &junk, &button, &junk);    /* :970 */
        if (button & 0x0001) {
            dest_wh = act_bdown(G.g_cwin, G.g_croot, &mx, &my,
                                &keystate, &c, &dobj);  /* :973-974 */
            if (dest_wh != NIL) {
                done = fun_drag(wh, dest_wh, 0, dobj, mx, my, keystate); /* :977 */
                desk_clear(wh);                        /* :978 */
            }
        }
        pw = win_find(wh);
        win_sinfo(pw, TRUE);                           /* :982-983 */
    } else {                                           /* clicks == 2 */
        act_bsclick(G.g_cwin, G.g_croot, mx, my, keystate, &c, TRUE); /* :987 */
        done = do_filemenu(OPENITEM);                  /* :988 */
        evnt_button(1, 0x01, 0x00, &junk, &junk, &junk, &junk);       /* :1003 */
    }

    men_update();                                       /* :1006 */
    return done;
}
```

Step by step, precisely as the source has it:

1. **`wh = wind_find(mx, my)`** (`:955`) — resolves the AES window under
   the click from **screen coordinates**, unconditionally, before any
   other state is touched. This is the mechanism the current NewTOS
   desktop is missing entirely (see `_current-defects.md` D3): every click
   anywhere on screen first asks the AES "which window (if any) is here?"
   — it does not consult any desktop-tracked "current/topped window"
   variable to decide *whether* to act.
2. **`if (wh != G.g_cwin) desk_clear_all()`** (`:957-958`) — if the click
   landed in a **different** window than the one the desktop currently
   considers current, deselect everything **everywhere** (every open
   window **and** the desktop) before processing the click. Clicking
   inside the *same* already-current window does **not** clear other
   windows' selections.
3. **`desk_verify(wh, FALSE)`** (`:960`) — makes `wh` the new
   `G.g_croot`/`G.g_cwin`/`G.g_wlastsel` (`desksupp.c:175-198`,
   `changed=FALSE` — a click never rebuilds the item grid, only a size
   change does).
4. **`wind_get_grect(wh, WF_WXYWH, &c)`** (`:962`) — the (possibly just-
   switched-to) window's current work-area rect, passed to every
   selection/drag helper below as the clip rect for their own redraws.
5. **Single click (`clicks==1`, `:964-984`):**
   - `act_bsclick(G.g_cwin, G.g_croot, mx, my, keystate, &c, FALSE)`
     (`:968-969`, `deskact.c:566-596`) — the click/selection state
     machine (rules cited in its own header comment, `deskact.c:552-559`):
     click on empty space deselects all; click on an unselected object
     deselects all + selects it; Shift-click toggles just that object;
     Shift-click on empty space is a no-op. **Out of scope for this
     entry** (cited, not re-derived — see `deskact.md`), but its call
     **signature and position in the sequence** is exactly what a 1:1
     rebuild must reproduce.
   - `graf_mkstate(&junk, &junk, &button, &junk)` (`:970`) — **re-reads the
     live mouse button state** right after the click-selection call (not
     the `button` parameter `hndl_button` was originally invoked with) —
     i.e. the drag-start decision below uses a **fresh** read, not the
     event's own `button` value.
   - **If the (freshly re-read) left button is still down**
     (`button & 0x0001`, `:971`): `act_bdown(G.g_cwin, G.g_croot, &mx, &my,
     &keystate, &c, &dobj)` (`:973-974`, `deskact.c:609-...`) — the
     drag-tracking state machine; returns the **destination** window
     handle (`dest_wh`) the object was dropped on, or `NIL` if it was
     simply a click-and-release with no drag. **Only if a real drag
     happened** (`dest_wh != NIL`): `fun_drag(wh, dest_wh, 0, dobj, mx, my,
     keystate)` (`:977`, out of scope — `deskfun.c`) performs the actual
     copy/move, then `desk_clear(wh)` (`:978`) deselects the **source**
     window (the item that was dragged is no longer "selected" after the
     drop).
   - **Always** (drag or not): `pw = win_find(wh); win_sinfo(pw, TRUE)`
     (`:982-983`) — refreshes that window's info line (selected-count/size
     or whole-directory count/size, `deskwin.c:867-906`) since a click may
     have changed the selection. **NewTOS (A110, 2026-09-23):** implemented
     at the same place in `kernel/src/desk/main.rs` `hndl_button`, together
     with the `STINFST2` format string and the other three selection-change
     call sites — see `deskwin.md` § `win_sinfo`.
6. **Double click (`clicks==2`, `:985-1004`):** `act_bsclick(...,TRUE)`
   (`:987`, `dclick=TRUE` changes rule 3 above — see the cited comment:
   with `dclick` true, even an **already-selected** object is
   deselect-all-then-reselect, ensuring a clean single-selection state
   before the open) then **unconditionally** `do_filemenu(OPENITEM)`
   (`:988`) — a double-click **is** "File → Open" on whatever
   `act_bsclick` just made current, dispatched through the exact same
   function a menu click would use. The trailing `evnt_button(1, 0x01,
   0x00, ...)` (`:1003`) is a **deliberate wait-for-button-up**, documented
   in-source (`:989-1002`) as a fix for AES double-click event replay and
   a `.TOS`-program-launch mouse-blink artifact — **not** part of the
   click/selection semantics itself, a defensive idiom a 1:1 rebuild
   should reproduce but need not treat as behaviourally significant beyond
   "don't return to the loop until the button is physically released
   after a double-click".
7. **Tail** (`:1006`): `men_update()` — **every** call to `hndl_button`
   ends with a menu-state recompute, unconditionally (unlike `hndl_kbd`,
   which skips it for CR/ESC/arrow-key).

**Contrast with the current NewTOS desktop** (`_current-defects.md` D3):
none of steps 1-4 exist — clicks are resolved exclusively against a single
tracked "top" handle, never against screen coordinates via an
AES-equivalent `wind_find`.

---

## Keyboard shortcut handling

`hndl_kbd(thechar)` (`:1201-1247`) — `thechar` is the AES `kret` from
`evnt_multi` (ASCII in the low byte, scancode in the high byte). Checked
**in this exact order**, first match wins:

1. `LOBYTE(thechar) == CR` (0x0d) → `desk_clear_all()` (deselect
   everything), return `FALSE`. **No menu action, no `men_update`
   call for this path** — it returns directly.
2. `LOBYTE(thechar) == ESC` (0x1b) → `refresh_window(pw, TRUE)` on the
   topped window if any, return `FALSE` (also skips `men_update`).
3. `check_arrow_key(thechar)` (`:1067-1081`) — scans `arrow_table[]` by
   **exact scancode+ASCII match** (the full 16-bit `thechar`, not just the
   high byte) for `ARROW_UP/DOWN/LEFT/RIGHT` and their `SHIFT_ARROW_*`
   variants; on match, `kbd_arrow(type)` (`:1043-1059`): no-op if the
   topped window is the desktop itself (`wh==DESKWH`) or doesn't resolve
   to a `WNODE`; else `desk_clear(wh)` (deselect first!) then
   `win_arrow(wh, type)`. Returns `TRUE` → `hndl_kbd` returns `FALSE`
   **without calling `men_update`** (an arrow scroll doesn't change
   selection-dependent menu state — though it *did* just deselect
   everything via `desk_clear`, which arguably should re-gray `DELTITEM`/
   `OPENITEM`; EmuTOS does not re-run `men_update` here).
4. `check_function_key(thechar)` (`:1117-1128`) — `FUNKEY_01..10`/
   `FUNKEY_11..20` (`scancode.h`) map to funkey number 1-20 via
   `HIBYTE(thechar-FUNKEY_0x)+offset`; `process_funkey` (`:1087-1106`)
   linearly scans `G.g_ahead` (the ANODE chain) for `a_funkey==funkey`,
   and if found launches it via `do_aopen` — **the path is built from the
   ANODE's `a_pappl`, split at the last backslash** (`filename_start`),
   so a function-key launch always resolves relative to the *application's
   own stored path*, not the current window. Returns `-1` if not a
   function key or no match (falls through to the next check); `0`/`1`
   (launched) is returned **directly as `hndl_kbd`'s result** — i.e. a
   function-key launch can set `done=TRUE` in the outer loop exactly like
   a menu Quit, if `do_aopen`'s return value says so.
5. `check_alt_letter_key(thechar)` (`:1136-1160`) — only matches if
   `LOBYTE(thechar)==0` (a "bare Alt" scancode with no ASCII — i.e. Alt
   held with another key that produced no character on its own is *not*
   how this is detected; rather, this only fires for keys whose ASCII
   byte is genuinely 0). Looks up the **shifted** keyboard-table entry
   (`Keytbl(-1,-1,-1)`, `KEYTAB.shift[HIBYTE(thechar)]`) for the scancode
   in the high byte — i.e. **it reads what the Shift-shifted character
   for that scancode would be**, not an Alt-table (there is no separate
   Alt table; Alt+letter is *inferred* from the scan code assuming a
   US/similar layout maps Alt+key to the same scancode as the bare key).
   If that resolves to `'A'..'Z'` and `valid_drive()` accepts it,
   `do_dopen(-drive)` (negative = "this is a drive letter, not an object
   id") opens that drive, `men_update()` is called explicitly (this is
   the **one** shortcut path that calls it inline rather than falling
   through to the bottom), returns `TRUE`.
6. Otherwise: `lookup_ascii_shortcut(ascii, &item)` (`:1169-1198`) — only
   examines **control characters** (`ascii < 0x20`; returns `-1`
   immediately otherwise), OR's `0x40` to get the displayed uppercase
   letter, linearly scans all 33 `shortcut_mapping[]` slots for a
   `menu_shortcuts[i]` match, **skips `0` slots** (item doesn't exist in
   this build), and — critically — **if the matching item is currently
   `DISABLED`, treats it as no match** (`break`s out of the scan
   entirely, doesn't continue looking for another slot with the same
   shortcut — though the shortcut table is 1:1 by construction so this
   doesn't matter in practice). Returns the *title* (`OPTNMENU` if
   `item>=IICNITEM`, `VIEWMENU` if `item>=ICONITEM`, else `FILEMENU` —
   **the Desk menu has no shortcut-reachable items**: `ABOUITEM` isn't in
   `shortcut_mapping[]` at all) or `-1`.
7. If a title was found: `menu_tnormal(...,title,0)` (**title flashes
   selected first**, param `0`) then `hndl_menu(title,item)` — same
   dispatch a mouse click would take.
8. **Always**, at the very end (after the shortcut dispatch, but *not*
   for CR/ESC/arrow-key returns which exit early above): `men_update()`.

**Edge case:** because the CR/ESC/arrow-key paths `return` before reaching
step 8, only the function-key, Alt-letter, and Ctrl-shortcut paths (and the
implicit "no match at all" fallthrough) call `men_update()` at the bottom
— but Alt-letter *also* calls it explicitly mid-function (step 5) before
its own early return, so it runs `men_update` exactly once, same as
everything else; CR and ESC are the only two keys that skip it entirely.

---

## Message-pipe handling (`hndl_msg`, `:1250-1361`)

Dispatches on `G.g_rmsg[0]` (message type); `handle = G.g_rmsg[3]` is read
unconditionally before the switch (every window message's handle sits at
the same slot).

**Dead code found in source:** `ig_close` (`:127`, `static WORD`, zero-init,
comment `/* BugFix */`) is read at the top of `hndl_msg`
(`if ((G.g_rmsg[0]==WM_CLOSED) && ig_close) { ig_close=FALSE; return
done; }`, `:1261-1265`) but is **never set to a nonzero value anywhere in
the EmuTOS source tree** (verified: `grep -rn ig_close desk/` finds only
the declaration and these two lines). The guard is therefore permanently
false in the current codebase — `WM_CLOSED` always falls through to
`case WM_CLOSED: do_filemenu(CLOSITEM); break;` (`:1293-1295`). **This is
an EmuTOS oddity, not a NewTOS requirement**: the effective, observable
behavior is simply "`WM_CLOSED` always triggers `do_filemenu(CLOSITEM)`";
NewTOS need not replicate the dead flag, only the effective behavior.

Per-message-type handling:

- **`MN_SELECTED`** (`:1271-1273`): `desk_verify(G.g_wlastsel, FALSE)`
  (re-sync current-window state to whichever window last held a
  selection — **not** to the menu's own window, since menus aren't
  windows) then `hndl_menu(handle, G.g_rmsg[4])` — here `handle`
  (`G.g_rmsg[3]`) is the **menu title**, `G.g_rmsg[4]` the **item**
  (`MN_SELECTED`'s message shape differs from every `WM_*` message: slot 3
  is title not window handle, but the code reads it through the same
  `handle` local for both).
- **`WM_REDRAW`** (`:1275-1281`): sets `menu=TRUE` (suppresses the
  trailing `men_update()`See below); `if (handle) do_wredraw(handle,
  (GRECT*)&G.g_rmsg[4])` — **the exact rectangle from the message**, not
  the window's full extent. `handle==0` (the desktop itself, `DESKWH==0`)
  is explicitly **skipped** — the desktop surface is redrawn by the AES
  itself via the `WF_NEWDESK` object tree, not by `do_wredraw`.
- **`WM_TOPPED`** (`:1282-1292`): `desk_clear(G.g_cwin)` (deselect the
  *previously* current window — **before** switching), `wind_set(handle,
  WF_TOP,...)` (tell the AES), `win_top(pw)` (move the `WNODE` to the
  front of `G.g_wfirst`'s chain **and** reorder its objects in
  `G.g_screen` via `objc_order`), `desk_verify(pw->w_id, FALSE)`
  (`changed=FALSE` — topping doesn't rebuild the item grid), sets
  `change=TRUE`.
- **`WM_CLOSED`** (`:1293-1295`): `do_filemenu(CLOSITEM)` — routes through
  the **File-menu handler**, not a window-specific close function — i.e.
  clicking a window's close box is defined to behave *exactly* like
  selecting File → Close (`CLOSE_FOLDER` semantics — see `do_filemenu`
  above), operating on `win_ontop()` regardless of which window's close
  box was actually clicked (the AES only sends `WM_CLOSED` for the topped
  window's close box in the first place, since only the topped window has
  input focus, but the handler doesn't itself use `handle` at all here).
- **`WM_FULLED`** (`:1296-1305`): `win_top(pw)` (**topping happens before
  the resize** — fuller implicitly also brings the window to front),
  `do_wfull(handle)` (toggle full/previous extent), `desk_verify(handle,
  TRUE)` (`changed=TRUE` — a size change rebuilds the item grid), sets
  `change=TRUE`.
- **`WM_ARROWED`** (`:1306-1308`): `win_arrow(handle, G.g_rmsg[4])` — no
  `desk_verify`/`change` at all; a scroll doesn't move the window or
  change its content grid, only the visible window into it.
- **`WM_HSLID`** (`CONF_WITH_SIZE_TO_FIT`, `:1309-1313`)/**`WM_VSLID`**
  (`:1314-1316`): `win_slide(handle, horizontal?, G.g_rmsg[4])` — slider
  drag; likewise no `desk_verify`.
- **`WM_MOVED`/`WM_SIZED`** (`:1317-1339`, shared code path): both read
  `pw=win_find(handle)` (bail if not found), copy the message rect via
  `rc_copy`, `do_xyfix` the position, `wind_set_grect(handle, WF_CXYWH,
  &gr)` (tell the AES the confirmed new geometry). **`WM_SIZED`
  additionally**: remembers the *old* column count (`pw->w_pncol`)
  *before* the resize, reads the **previous** extent (`WF_PXYWH`) to
  detect a **shrink** (`shrunk = new.w<=prev.w && new.h<=prev.h` — note
  this compares against `G.g_rmsg[6]`/`[7]`, the message's **width/
  height**, not the just-computed `gr`), then `desk_verify(handle,TRUE)`
  (rebuild the grid — this is what may change `w_pncol`). **`WM_MOVED`**
  instead: reads back the confirmed rect (`WF_WXYWH`) and copies it into
  `G.g_screen[pw->w_root].ob_x/y/w/h` directly (`rc_copy(&gr,
  (GRECT*)&G.g_screen[pw->w_root].ob_x)`) — **a direct poke of the AES
  object array**, not through any AES call, keeping the desktop's own
  cached copy of the window's screen position in sync for later
  screen-relative math (e.g. `do_wopen`'s zoom-box source-object lookup).
  Either way, `change=TRUE`.
- **Post-switch shrink fixup** (`:1342-1350`, **outside** the switch,
  runs after *every* message type though only ever true after
  `WM_SIZED`): `if (shrunk && (pw->w_pncol != cols))` — if the window
  shrank **and** the column count the rebuild just computed differs from
  the pre-resize count, synthesizes an explicit
  `fun_msg(WM_REDRAW, handle, ...)` (queues a **new** message into the
  pipe) — comment: "we need to send a redraw message because the AES
  won't" (when a window shrinks, the AES's own frame-resize redraw
  doesn't know the *content* needs re-flowing into fewer columns, since
  that's desktop-private state).
- **Tail** (`:1352-1360`): `if (change) cnx_put()` (persist window
  geometry to the in-memory save area on every `WM_TOPPED`/`WM_FULLED`/
  `WM_MOVED`/`WM_SIZED`); `G.g_rmsg[0]=0` (mark consumed — **not**
  strictly necessary for a fresh `evnt_multi` result but zeroes it for
  safety/debugging); `if (!menu) men_update()` — **every message type
  except `WM_REDRAW`** re-runs the full menu-state recompute.

---

## Redraw wiring (`desk_all` + `do_wredraw`)

### `desk_all(flags)` (`:438-447`) — the View-menu → redraw pipeline

```
desk_busy_on();
if (flags & SORT_HAS_CHANGED)                              win_srtall();
if (flags & (VIEW_HAS_CHANGED|SORT_HAS_CHANGED|SIZETOFIT_HAS_CHANGED))
                                                             win_bdall();
win_shwall();
desk_busy_off();
```

Only called from `hndl_menu`'s `VIEWMENU` case (with the `do_viewmenu`
return bitmask) and once, unconditionally with `flags=0`, from
`do_optnmenu`'s `PREFITEM` (forces the `win_shwall()` tail without a
rebuild). **`BACKGROUND_HAS_CHANGED` alone triggers neither `win_srtall`
nor `win_bdall`** — only `win_shwall()` (a background-colour/pattern change
needs a repaint, not a grid rebuild) — this is why the bit exists at all
distinct from `SIZETOFIT_HAS_CHANGED` (which **does** trigger `win_bdall`).

`win_srtall()`/`win_bdall()`/`win_shwall()` (`deskwin.c:768-819`, cited
above) each walk **every allocated `WNODE`** (`G.g_wfirst`/`w_next` chain,
`w_id != 0` = actually open) — `win_srtall` re-sorts each window's FNODE
list in place (`pn_sort`); `win_bdall` calls `win_bldview(pw,
&clip)` per window with `clip = wind_get_grect(pw->w_id, WF_WXYWH)` (the
window's **current on-screen rect**, queried **fresh per window**, not
cached); `win_shwall` — for every **non-desktop** window (`wh != DESKWH`)
— queries the rect again and calls **`fun_msg(WM_REDRAW, wh, x,y,w,h)`**:
this **queues an AES message**, it does not call `do_wredraw` directly.
The actual repaint therefore happens **later**, back in the main loop's
message-drain sub-loop, when that synthesized `WM_REDRAW` is dequeued and
dispatched through `hndl_msg` → `do_wredraw` like any AES-originated
redraw. **The desktop pseudo-window (`DESKWH`) is never sent a
`WM_REDRAW` by `win_shwall`** — its surface is the AES's own `WF_NEWDESK`
object tree, redrawn by the AES's normal window-manager machinery, not by
this desktop-internal path.

### `do_wredraw(w_handle, gptr)` (`desksupp.c:202-232`) — the actual paint

```
clip_r = *gptr;                              /* the requested damage rect */
root = (w_handle==DESKWH) ? DROOT : win_find(w_handle)->w_root;
graf_mouse(M_OFF, NULL);
wind_get_grect(w_handle, WF_FIRSTXYWH, &t);
while (t.g_w && t.g_h) {
    if (rc_intersect(&clip_r, &t))
        objc_draw(G.g_screen, root, MAX_DEPTH, t.g_x, t.g_y, t.g_w, t.g_h);
    wind_get_grect(w_handle, WF_NEXTXYWH, &t);
}
graf_mouse(M_ON, NULL);
```

This is the mechanism the task calls out explicitly: **the redraw is
double-clipped** — once against the message's own damage rectangle
(`clip_r`, whatever the caller asked to be redrawn — a sub-region on a
partial expose, or the whole window on a full redraw request), and once,
**per iteration**, against the AES's own **visible-rectangle list** for
that window handle (`WF_FIRSTXYWH`/`WF_NEXTXYWH` — the set of on-screen
rectangles actually belonging to this window *after* every other,
higher-Z window has been subtracted out by the AES's own window manager).
`rc_intersect` computes the overlap of the two, and **only that overlap**
is painted, one `objc_draw` call per surviving rectangle. A window that is
95% covered by another window on top of it therefore repaints **at most**
the 5% still visible — never the covered part, **regardless of draw
order**, because the occlusion is baked into the rectangle list itself,
not into "draw the topmost window last". `graf_mouse(M_OFF/ON)` brackets
the whole redraw so the mouse cursor doesn't need per-rectangle handling.

Also note: `G.g_idt = Supexec((LONG)get_idt_cookie)` is refreshed at the
top of every `do_wredraw` call — the date/time format cookie is re-read
from supervisor mode on **every single redraw**, not cached (it can change
while the desktop runs, e.g. `_IDT` cookie changing via `Ssystem`/config
tools).

---

## Session save/restore (`cnx_put`/`cnx_get`)

Not core to the event loop, but the two functions that make the "session"
concept (window layout across a resolution-change/read-INF restart)
observable, and the only place `WM_TOPPED`/`WM_FULLED`/`WM_MOVED`/
`WM_SIZED` handling in `hndl_msg` has an externally-visible effect beyond
the screen (`cnx_put()` call at the tail of `hndl_msg`, `:1352-1353`).

- **`cnx_put()`** (`:1371-1427`): copies every current preference
  (`g_iview`/`g_isort`/copy-delete-overwrite-confirm flags/click
  speed/date-time format/blitter/cache/app-dir/full-path/size-to-fit) into
  `G.g_cnxsave`, **then** walks `G.g_wfirst`/`w_next` to count how many
  `WSAVE` slots are unused, zeroes those slots' `pth_save[0]` first (from
  the **end** of the `cs_wnode[]` array backward), then fills the
  remaining slots **from the start of the array forward** with each open
  window's geometry (`wind_get(WF_CXYWH)`, `do_xyfix`'d), scroll position
  (`w_cvcol`/`w_cvrow`), and path (`w_pnode.p_spec`) — **slot 0 = the
  bottom-most window** (comment, `:1367-1369`), i.e. windows are saved in
  `w_next`-chain order which, per `win_top`, is **most-recently-topped
  first** — so slot assignment is actually "last window in the chain
  (bottom) → slot 0" by construction of the two loops' pointer arithmetic
  (`pws` starts at `NUM_WNODES-1` for the *unused* count, then continues
  decrementing through the *used* loop — so the **first** `WNODE`
  encountered in chain order, i.e. the **topmost**, lands in the
  **highest** used slot index, and the **bottom-most** lands in slot 0).
- **`cnx_get()`** (`:1430-1501`): applies the saved view/sort via
  `do_viewmenu(ICONITEM+cs_view)`/`do_viewmenu(NAMEITEM+cs_sort)` — i.e.
  **reuses the menu dispatcher itself** to restore state (so the menu
  checkmarks end up correct too, for free) rather than setting `G.g_iview`
  directly. Restores every preference field directly. **Validates**
  each saved window's position/size against the *current* `G.g_desk`
  extent (clamped to `g_desk.g_w/2`/`g_desk.g_h/2` if out of range, or to
  the full extent if the saved w/h is `<=0` or larger than the desktop) —
  this is what makes a saved session from a **different resolution**
  degrade gracefully instead of placing windows off-screen. For each slot
  with a non-empty saved path, resolves it back to an object id
  (`obj_get_obid(pws->pth_save[0])` — **first character of the path only**,
  i.e. the drive letter) and calls `do_diropen` to reopen it — **silent
  failure, two distinct paths** (`:1489-1499`, precision fix — the prior
  wording implied a single `win_free` path for both failures): if
  `win_alloc(obid)` itself fails (`NUM_WNODES` slots already used), the
  `if ((pw = win_alloc(obid)))` block is skipped entirely — nothing was
  allocated, so no `win_free` call is needed or made; if `win_alloc`
  **succeeds** but the subsequent `do_diropen(...)` fails, **then**
  `win_free(pw)` releases the just-allocated `WNODE`. Either way the slot
  ends up simply not reopened, no error surfaced to the user.

---

## Edge cases discovered in source

- **`ig_close` is dead code** (see above) — a "BugFix" flag that never
  gets set; `WM_CLOSED` handling is unconditionally `do_filemenu
  (CLOSITEM)`. NewTOS should implement the *effective* behavior, not the
  vestigial flag.
- **CR and ESC bypass `men_update()`** entirely (early `return FALSE` in
  `hndl_kbd` before reaching the bottom); every other keyboard path
  reaches it (either explicitly, for Alt-letter, or via fallthrough).
- **`WM_REDRAW` never calls `desk_verify`** and is the only message type
  that suppresses the trailing `men_update()` (`menu=TRUE` short-circuits
  it) — a pure repaint changes no state at all, by design.
- **The desktop pseudo-window is invisible to `win_shwall`/`do_wredraw`**:
  `win_shwall` explicitly skips `wh==DESKWH`; `do_wredraw`'s caller in
  `hndl_msg` explicitly skips `handle==0` (`if (handle) do_wredraw(...)`,
  and `DESKWH==0`). The desktop surface's redraw is **entirely** the AES's
  own responsibility once `wind_set(DESKWH, WF_NEWDESK, ...)` hands it the
  object tree.
- **A shrink-with-column-change synthesizes its own `WM_REDRAW`**
  (`:1342-1350`) — the one place `deskmain.c` itself pushes a message into
  the pipe from inside `hndl_msg` rather than relying on the AES or
  `win_shwall`.
- **`men_update`'s hardware-feature checks are cached at startup**
  (`can_change_resolution`/`blitter_is_present`/`cache_is_present` from
  `detect_features()`), never re-probed — a resolution change (which
  restarts the whole desktop process) is the only way these can change.
- **Alt-letter detection has no dedicated Alt keytable** — it reads the
  **Shift** table for the raw scancode and assumes that coincides with
  what the physical Alt+key would look like on the layouts EmuTOS
  targets; this is inherently keyboard-layout-dependent and undocumented
  in Compendium.
- **`adjust_menu`'s French-desktop half-character shrink** (`:1546-1564`)
  is a real, shipped compatibility hack for one specific translation
  overflowing ST-Low screen width — evidence that menu-bar layout math is
  tuned to *specific known content*, not a fully general algorithm; a
  from-scratch NewTOS layout only needs to handle **NewTOS's own** menu
  text, so this specific hack is not required, but the general
  measure-then-shrink-then-truncate **strategy** (not just "assume it
  fits") is the contract-relevant behavior to preserve if menu text can
  ever be long enough to overflow at NewTOS's target resolution.

---

## Dependencies

### Requires (must be initialized / functional)

- The AES (`appl_init`, `evnt_multi`, `menu_bar`, `wind_*`, `objc_draw`,
  `form_do`/`form_alert`, `graf_mouse`) — Phase 13, already spec'd
  (`docs/api/aes/`).
- `desk_rs_init()`/resource loading (`deskrsrc.md`/`deskobj.md`) — must run
  before `desk_xlate_fix`.
- `app_start()` (`deskapp.md`) — must run before `install_shortcuts()`
  (may customize `menu_shortcuts[]`) and before `win_start()`/
  `app_blddesk()` (icon/window state must exist to build the tree).
- `win_start()` (`deskwin.md`) — must run before `desk_verify(DESKWH,
  FALSE)` and before any `win_*` call in the event loop.

### Used by

- `_deskstart` (`deskstart.S`) — calls `deskmain()` in a loop (restart on
  `FALSE` return, terminate on `TRUE`).
- Every other `desk/` module's window/menu-affecting function eventually
  routes back through `hndl_menu`/`hndl_msg`/`desk_all` to reach the
  screen — this file is the **integration point**, not a leaf.

---

## Related data structures

- `docs/api/structures/` — `OBJECT`/`TEDINFO`/`ICONBLK` (menu + dialog
  trees) — already spec'd for AES (Phase 13); the desktop reuses the same
  layouts for its own resource, no divergence.
- `GLOBES` (`deskbind.h:130-245`) — see the sibling `_state.md` (produced
  by the parallel extraction task) for the full field account; this entry
  only lists the subset `deskmain.c` itself touches.
- `CSAVE`/`WSAVE` (referenced by `cnx_put`/`cnx_get`; defined in
  `deskapp.h` — see `deskapp.md`).

---

## Configuration / version notes

Every `#if CONF_WITH_*`/`#if WITH_*` gate this file contains, its EmuTOS
default (`include/config.h`, machine-block-dependent — see
[[check-conf-gating-emutos-extraction]]), and the NewTOS decision **where
one is already on record** (`docs/api/desktop.md`,
`docs/HARDWARE-PROFILE.md`):

| Flag | Gates | NewTOS decision |
|---|---|---|
| `CONF_WITH_SEARCH` | `SRCHITEM` (File→Search) | **Open question** — no record found; STOP-flagged |
| `CONF_WITH_BOTTOMTOTOP` | `BTOPITEM` (File→Cycle/bottom-to-top) | **Open question** |
| `CONF_WITH_SELECTALL` | `SLCTITEM` (File→Select all), `ILL_NOWIN` entry | **Open question** |
| `CONF_WITH_FILEMASK` | `MASKITEM` (File→Set mask) | **Open question** |
| `CONF_WITH_FORMAT` | `FORMITEM` (File→Format) | **Open question** — no floppy hardware on RPi4; likely off, but not decided |
| `CONF_WITH_EJECT` | `EJCTITEM` | **Excluded** — Apple-Lisa-only auto-eject, no Atari/AArch64 equivalent (`docs/api/desktop.md` reconciliation) |
| `WITH_CLI` | `CLIITEM` (File→Execute EmuCON) | **On** — NewTOS ships a shell (`crate::shell::run_console`, `kernel/src/desktop.rs:826`) |
| `CONF_WITH_SHUTDOWN` | `QUITITEM` | **On (halt-only)** — `docs/HARDWARE-PROFILE.md:406` |
| `CONF_WITH_SIZE_TO_FIT` | `FITITEM`, `g_ifit`/`g_icols`, horizontal slider messages | **Open question** — current `kernel/src/desktop.rs` has a `size_to_fit()` (vertical only, `:1619-1637`); full EmuTOS semantics (horizontal too) not confirmed decided |
| `CONF_WITH_BACKGROUNDS` | `BACKGRND` item, `g_patcol` | **On** — `docs/api/desktop.md:71-72`; NewTOS's `desktop.rs` `G_PATCOL`/`set_background` already implements this |
| `CONF_WITH_DESKTOP_CONFIG` | `CONFITEM` | **On** — `docs/api/desktop.md:69` |
| `CONF_WITH_READ_INF` | `READITEM`, desktop-restart path | **Open question** |
| `CONF_WITH_BLITTER` | `BLITITEM` | **Excluded** — STE/TT/Falcon blitter chip, no AArch64 equivalent (`docs/api/desktop.md:73-78`) |
| `CONF_WITH_CACHE_CONTROL` | `CACHITEM` | **Excluded** — 68030+ CPU-cache control (`docs/api/desktop.md:73-78`) |
| `CONF_WITH_EASTER_EGG` | About-box Easter egg | **Open question** (low priority) |
| `CONF_WITH_PRINTER_ICON` | printer icon (`men_update`'s `ntrash` counting) | **Excluded** (no printer support recorded elsewhere) — not formally decided here |
| `CONF_WITH_3D_OBJECTS` | dialog 3D-gadget position tweaks | **On** — `docs/HARDWARE-PROFILE.md:401` |
| `CONF_WITH_ALT_DESKTOP_GRAPHICS` | title-alignment style | **Open question** |
| `CONF_WITH_VIDEL`/`CONF_WITH_ATARI_VIDEO` | which `shel_write(SHW_RESCHNG,...)` variant | **Off** per `docs/HARDWARE-PROFILE.md:405` (16-colour path only) — resolution-change messaging needs its own decision for a fixed-resolution RPi4 target |
| `CONF_WITH_PCGEM` | shell-default-dir-on-CLI-exit behavior | **Open question** |
| `CONF_WITH_DESKTOP_SHORTCUTS` | desktop-icon shortcut prompts (referenced in `men_update`'s `ndesk` counting) | **Open question** |

**STOP-flag:** the "open question" rows above are gates this file
exercises but for which no prior NewTOS ADR/decision was found in
`docs/`. Per CLAUDE.md Rule 3, these are **not** decided in this spec —
the team lead should resolve them (most are small, low-risk "keep or cut
this menu item" calls) before or during `kernel/src/desk/` implementation.

---

## NewTOS implementation notes

> **No window at boot without a saved one (2026-09-23).** NewTOS used to
> open the boot drive's root whenever the context restored no window — a
> crutch from the days when the mouse did not work and a window was the
> only way to see anything. With the context save in place (DESK09) it is
> gone: the desktop starts bare unless `NEWDESK.INF` names a window, which
> is what EmuTOS does (`desk/deskapp.c:171-180`, its built-in INF carries
> four `#W` lines with empty paths). `kernel/src/desk/mod.rs`, after
> `cnx_get`.


### The context save area — `CSAVE` / `cnx_put` / `cnx_get` (DESK09, 2026-09-23)

`G.g_cnxsave` is `DeskState::g_cnxsave` (`kernel/src/desk/global.rs`),
a `CSave { cs_view, cs_sort, cs_sizefit, cs_wnode: [WSave; NUM_WNODES] }`
with `WSave { x_save, y_save, w_save, h_save, hsl_save, vsl_save,
pth_save }` — the whole `WSAVE` field set (`deskapp.h:120-131`), and the
subset of `CSAVE`'s preference fields the desktop has live state for.
The rest of `CSAVE` mirrors `GLOBES` fields that exist only because
`inf_pref`/`inf_conf` can edit them, and both dialogs now exist (DESK02,
DESK05) — but the three *reasons* differ and are recorded here so the
next reader does not re-derive them:

* `cs_confdel`/`cs_dblclick`/`cs_timefmt`/`cs_datefmt` **are** carried
  (DESK02): each has a live consumer (`desk::dir`'s delete question, the
  AES `evnt_dclick` rate, `fmt_date`/`fmt_time`).
* `cs_confcpy`/`cs_confovwr` are not: NewTOS's copy engine asks no
  per-item and no overwrite question, so the switches are absent from
  `inf_pref` too (`deskinf.md`, DESK02).
* `cs_blitter`/`cs_cache` are m68k hardware; `cs_appdir`/`cs_fullpath`
  are `inf_conf`'s launch-directory policy, whose consumers
  (`set_default_path` in `do_aopen`, `build_selected_path` in
  `fun_win2win`) NewTOS does not have, so the two radios are absent from
  `inf_conf` too (`deskinf.md`, DESK05).

The bits of all of these survive a load/save round trip — `desk::inf`
keeps the parsed `#E` bytes of the session and re-emits every bit it
does not own — they are simply not routed through this structure.

| EmuTOS | NewTOS |
|---|---|
| `app_start`'s `#E`/`#W` arms (`deskapp.c:889-948`) | `inf::apply` — `#E` → `cs_view`/`cs_sort`/`cs_sizefit`, `#W` → `cs_wnode[]` (cells × `gl_wchar`/`gl_hchar` → pixels) |
| `cnx_put()` (`:1371-1427`) | `main::cnx_put()` |
| `cnx_get()` (`:1430-1501`) | `main::cnx_get() -> usize` (how many windows it opened) |
| `app_save(TRUE)` (`deskapp.c:1106-1290`) | `inf::save_desktop()` — `#E` and all `NUM_WNODES` `#W` lines out of `cs_wnode[]` |
| `win_alloc`'s default rect `cs_wnode[g_wcnt]` (`deskwin.c:165`) | `inf::window_rect(slot)` |

**Slot order is the source's, verbatim.** `cnx_put` counts the unused
slots, blanks their `pth_save` from the *end* of the array backwards,
then fills downwards from where that stopped with the window chain
front-to-back — so the topmost window lands in the highest used slot and
**slot 0 holds the bottom-most window** (`:1367-1369`). `cnx_get` walks
`0..NUM_WNODES`, i.e. bottom-most first, and the z-order comes back the
way it was saved.

**Call sites:**

* `cnx_get()` — in `desk::run()`, after `app_blddesk` and the
  `wind_set(DESKWH, WF_NEWDESK, …)`, bracketed by
  `wind_update(BEG_UPDATE)` / `END_UPDATE`, exactly at `:2053-2057`.
* `cnx_put()` — `Options → Save desktop` (`main::do_save_desktop`,
  before `app_save`, `:889-893`); the tail of `main::hndl_msg` when a
  message changed a window (`WM_TOPPED`/`WM_FULLED`/`WM_MOVED`/
  `WM_SIZED` set `change`, `:1352-1353`); and the desktop epilog
  (`main::desktop_relinquish`, `:2154`).
* `inf::set_background` also runs `cnx_put()` before its `save_desktop()`
  — NewTOS persists a background change immediately where EmuTOS leaves
  it to the next Save desktop, and the invariant "`app_save` is always
  preceded by `cnx_put`" (`:891`, `:2154`) is kept.

**Named deviations, four:**

1. **The desktop epilog's `app_save(FALSE)` has no counterpart.** EmuTOS
   parks the whole INF image in the AES shell buffer (`shel_get`/
   `shel_put`) so a restarted desktop finds it without touching the
   disk; NewTOS's `NEWDESK.INF` goes to and from the file directly.
   Consequence: a window layout survives a **cold boot** after Save
   desktop, but not the A106 AES restart on its own.
2. **`cnx_get()` is not run in the desktop *prolog*
   (`main::desktop_resume`).** That desktop resumes in place with its
   windows still registered (`wm_new_keep_desktop`), so re-opening them
   from the context would duplicate every one. Closing them in the
   epilog and rebuilding them in the prolog is the launch-path half of
   gaps G1/G2 and belongs to that rebuild.
3. **`obj_get_obid(pth_save[0])`** — EmuTOS resolves the saved path's
   drive letter back to a desktop object id and hands it to `win_alloc`
   as `w_obid`. NewTOS's window open path does not carry a source object
   (`win_alloc(handle, 0)` everywhere), and EmuTOS's own `win_free` on a
   failed `do_diropen` has no counterpart either, because the NewTOS
   open path has no failure signal: an unreachable saved path yields an
   empty window, exactly as any other unreachable path does.
4. **An unused slot supplies no default rectangle.** EmuTOS's
   `win_alloc` takes `cs_wnode[g_wcnt]` unconditionally, which works
   because its built-in `EMUDESK.INF` always carries four default `#W`
   lines with empty paths (`desk_inf_data1`, `deskapp.c:177-180`).
   NewTOS ships none, and `cnx_get`'s validation turns an all-zero slot
   into the **whole desktop** (`w_save <= 0` → `g_desk.g_w`,
   `:1476-1483`), so taking it would open every fresh window
   full-screen. `inf::window_rect` therefore answers only for a slot
   that holds a saved window; the staggered fallback stands otherwise.
   Adopting `desk_inf_data1`'s `#W` defaults would remove the deviation
   and change the boot picture (304×192 at (16,96) instead of 544×400 at
   (48,43)) — a decision of its own.

**Read .INF** (`Options → Read .INF file`, a NewTOS menu item) re-applies
in place where EmuTOS restarts the desktop (`READITEM` sets
`restart_desktop` and ends the loop, `:919-924`). It therefore takes only
the preference half of `cnx_get` (`main::cnx_get_prefs`,
`:1437-1461`): the context's windows are already on screen and must not
be opened a second time.

---

## NewTOS implementation notes — `do_deskmenu`'s About box (DESK01)

`kernel/src/desk/main.rs::do_deskmenu` had one case and no dialog: it put a
`form_alert` up with a NewTOS one-liner where EmuTOS shows the `ADDINFO`
resource tree. DESK01 replaces the alert with a real dialog,
`kernel/src/desk/inf.rs::do_aboutdesk`, built and run exactly like every other
`desk/` dialog (DESK08's `inf_show` chain — `form_center` →
`form_dial(FMD_START)` → `objc_draw` → `form_do` → `form_dial(FMD_FINISH)` →
message drain).

**Why the alert had to go.** A `form_alert` is not a dialog: it is the AES's own
`DIALERT` tree with its own `bb_save`/restore, no reserved dialog rectangle, and
no `WM_REDRAW` fan-out to the windows it covered. The About box is a dialog in
EmuTOS (`deskmain.c:620-646` calls `start_dialog`/`end_dialog` by hand), so it is
one here.

### The tree

`ADDINFO` (`desk/desk_rsc.c:772-873`) is an `OUTLINED` root box 40 characters
wide and 19 rows tall holding a `- EmuTOS -` title between two `G_IMAGE` logos, a
`Version` label + value pair, a copyright block, the project URL, the GPL notice
and one `DEOK` button. NewTOS keeps the **shape** — centred box, title line, a
label/value pair block, free text lines, one `DEFAULT|EXIT` OK button — and
replaces the **content**:

| # | Type | Content | Source |
|---|---|---|---|
| 0 | `G_BOX` | the box | sized to its text, see below |
| 1 | `G_STRING` | `- NewTOS -` | (title, centred) |
| 2/3 | `G_STRING` ×2 | `Version` + `NewTOS 0.1` | `initinfo::VERSION_STRING` |
| 4/5 | `G_STRING` ×2 | `Machine` + the board name | `initinfo::machine_name()` |
| 6/7 | `G_STRING` ×2 | the independence notice, split at its dash | `initinfo::INDEPENDENCE_NOTICE` |
| 8 | `G_STRING` | the authorship line | `initinfo::AUTHORSHIP_LINE` |
| 9 | `G_BUTTON` | ` OK ` (`SELECTABLE\|DEFAULT\|EXIT\|LASTOB`) | = `DEOK` |

The three strings are **`pub(crate)` constants in `kernel/src/initinfo.rs`**, read
by the boot screen and by this box. That is EmuTOS's own arrangement: its About
box carries no version of its own either — `desk_xlate_fix` points `DEVERSN`'s
`ob_spec` at the BIOS `version[]` string the boot screen also prints
(`deskmain.c:1881`).

### Named deviations

1. **`inf_show`, not the `form_do` loop.** EmuTOS loops
   (`deskmain.c:626-641`) only because `DEICON` is a `TOUCHEXIT` that plays the
   Easter-egg tune (`CONF_WITH_EASTER_EGG`) instead of closing. With no such
   object the loop runs exactly once, so the three calls are `inf_show`.
2. **The box is sized to its text, not to 40 fixed columns.** The machine line
   is board-dependent, so there is no build-time width to bake in. EmuTOS solves
   the same problem for its menus at run time (`adjust_menu`,
   `deskmain.c:1608-1635`).
3. **No build date.** `ADDINFO` has `DECOPYRT`, patched from `COPYRIGHT_YEAR`.
   NewTOS has **no build-date or build-year constant anywhere**: the OSHEADER's
   `os_date` is an explicit 1980-01-01 placeholder with the gap already recorded
   (`tos-api/src/bdos/process/loader.rs:806-809`), and the boot screen's
   `Boot time` line is the GEMDOS clock, not the build. Printing either would
   state something untrue about the build, so the line is **absent** until a real
   constant exists. Blocked on that constant, not on this dialog.
4. **No logos.** `ADDINFO`'s two `G_IMAGE` objects come from the resource file's
   `BITBLK`s; NewTOS has no desktop resource file.

### Measured

QEMU raspi4b, 640×480, `measurement-2026-09-24-desk01-desk02/`: the box lands at
**(128, 111, 384, 276)**, which is `form_center`'s value
(`(640-384)/2 = 128`, `19 + (480-19-276)/2 = 111`), and after `OK` the screen is
**byte-identical** to the one before it opened (same PNG md5, 0 differing
pixels).

---

## NewTOS implementation notes — the menu shortcut machinery (DESK05)

`deskmain.c` owns four of the six pieces the Desktop-configuration dialog
edits; the dialog itself is in [`deskinf.md`](deskinf.md#newtos-implementation-notes--inf_conf-desk05),
which also carries the measurement. All four now exist in
`kernel/src/desk/menu.rs`.

| EmuTOS | NewTOS |
|---|---|
| `menu_shortcuts[NUM_SHORTCUTS]` (`:119`) | `MENU.shortcuts` (33 bytes) + the `shortcut_key`/`set_shortcut_key`/`shortcuts`/`set_shortcuts` accessors |
| `shortcut_mapping[NUM_SHORTCUTS]` (`:169-260`) | `SHORTCUT_MAPPING: [Option<Item>; 33]` |
| `default_shortcuts[NUM_SHORTCUTS]` (`:269-307`) | `DEFAULT_SHORTCUTS` |
| `copy_menu_items()` (`:1755-1813`) | `MENU.labels`, filled by `build_menu` |
| `install_shortcuts()` (`:1825-1858`) | `install_shortcuts()`, called by `install_menu_bar` |
| `lookup_ascii_shortcut()` (`:1169-1198`) | `lookup_ascii_shortcut()`, array-driven |

Three points the source forces:

1. **The strings must be in RAM, and three characters longer.**
   `copy_menu_items` exists for exactly one reason: `install_shortcuts` writes
   into the item text. It sizes each menu at `widest + 1 + SHORTCUT_SIZE`
   (`:1781`) and pads every entry to `width - 1` characters (`:1799-1805`);
   `adjust_menu` then measures those padded strings, so the drop-down box is
   `widest + 4` cells and the last two characters are the `^X` slot
   (`:1845`). NewTOS's labels were `&'static` ROM literals with no reserve and
   the menu displayed **no shortcuts at all** before DESK05.
2. **A separator is not part of that measurement.** `copy_menu_items` skips it
   (`:1773`, `if (*q != '-')`), so its `ob_spec` is still the resource's
   one-character `"-"` when `adjust_menu` runs, and `adjust_menu` re-points it
   at `m` dashes (`:1623`). `build_menu` does the same, which is why the
   hand-written dash-run literals are gone.
3. **`shortcut_mapping`'s slot order is `NEWDESK.INF` contract.** The `#K` line
   is positional, so the three items NewTOS does not build (`FORMITEM`,
   `BLITITEM`, `CACHITEM`) keep their slots as `None` instead of being squeezed
   out — EmuTOS's own `0` convention (`:171-173`), and `install_shortcuts`
   clears such a slot in the array so a foreign `#K` cannot re-save a shortcut
   for an item that is not there (`:1833-1839`).

`lookup_ascii_shortcut` is the one behavioural rewrite: it used to match a
hard-coded `Item → letter` table, and now walks `menu_shortcuts[]` through
`shortcut_mapping[]`, skipping absent items (`:1181-1182`) and stopping on a
matching-but-`DISABLED` one (`:1185-1188`). Measured in
`measurement-2026-09-23-desk05/` (`Ctrl-S` still runs *Save desktop*).
