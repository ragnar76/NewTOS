# `desksupp.c` — `do_wopen` and `do_diropen`, the shared tail of every window open

> Phase-00 extraction, Track A. Source: EmuTOS @ `1a7484ab`,
> `desk/desksupp.c:275-322` (`do_wopen`) and `:415-471` (`do_diropen`).
> Written for DESK50 (2026-10-01), which builds both functions; before it,
> this file's material existed only as a line reference in `deskpro.md`,
> `deskmain.md` §DESK34 and `deskinf.md` §`opn_appl`.

## EmuTOS source

**Primary locations:**
- `desk/desksupp.c:294-322` — `void do_wopen(WORD new_win, WORD wh, WORD curr, GRECT *pt)`
  (header comment `:275-293`)
- `desk/desksupp.c:422-471` — `WORD do_diropen(WNODE *pw, WORD new_win, WORD curr_icon, char *pathname, GRECT *pt, WORD redraw)`
  (header comment `:415-421`)

**Installed handler / vector:** none — both are called directly.

**Callers (complete, `grep -n "do_wopen\|do_diropen" desk/*.c`):**

| Function | Caller | Arguments |
|---|---|---|
| `do_wopen` | `deskpro.c:61` `pro_run` | `do_wopen(FALSE, wh, curr, &G.g_desk)`, only `if (wh != -1)` |
| `do_wopen` | `desksupp.c:454` `do_diropen` | `do_wopen(new_win, pw->w_id, curr_icon, pt)` |
| `do_diropen` | `deskfun.c:413` `search_display` (`CONF_WITH_SEARCH`) | `(search_window, newwin, curr, pathname, &G.g_screen[w_root].ob_x, FALSE)` |
| `do_diropen` | `deskmain.c:1496` `cnx_get` | `(pw, TRUE, obid, pws->pth_save, (GRECT *)pws, TRUE)` |
| `do_diropen` | `desksupp.c:1067` `do_dopen` | `(pw, TRUE, curr, path, &G.g_screen[pw->w_root].ob_x, TRUE)` |
| `do_diropen` | `desksupp.c:1156` `do_fopen` | `(pw, new_win, curr, app_path, &t, TRUE)` |

`do_fopen` itself is reached from `do_open` (`desksupp.c:1259`, a folder),
`w_setpath` (`deskfun.c:824`, Close-folder / `WM_CLOSED` "up one level",
`curr = 0`) and `refresh_window` (`desksupp.c:1692`, `curr = 0`).

**Helpers called:**
- `desksupp.c:262-272` `do_xyfix(&t.g_x, &t.g_y)` — x to the nearest
  16-pixel boundary, y below the menu bar (plus the `CONF_WITH_3D_OBJECTS`
  right-edge nudge, `:265-269`)
- `desksupp.c:238-245` `get_xywh(olist, obj, …)` — the raw `ob_x/ob_y/
  ob_width/ob_height` of one object (no parent offset)
- AES `graf_growbox_grect(&d, &t)` — `gembind.c:589-600`,
  `AES_CTRL_CODE(GRAF_GROWBOX, 8, 1, 0)`: opcode 73, eight `int_in`
  (from x/y/w/h, to x/y/w/h), one `int_out`
- `deskact.c:419-466` `act_chg(wh, root, obj, pc, set, dodraw)`
- AES `wind_open_grect(wh, &t)`, `wind_set(pw->w_id, WF_NAME, …)`
- `desksupp.c:1840-1852` `desk_busy_on`/`desk_busy_off`
- `deskfpd.c:69-90` `pn_open`, `deskfpd.c:227-298` `pn_active`,
  `deskfpd.c:59-63` `pn_close`
- `deskwin.c:846-858` `win_sname`, `deskwin.c:867-906` `win_sinfo`,
  `deskwin.c:218-240` `win_top`
- `desksupp.c:175-199` `desk_verify(wh, TRUE)`
- `deskfun.c:101-113` `fun_msg(WM_REDRAW, …)` — fills `G.g_rmsg` and runs
  `hndl_msg` synchronously, whose `WM_REDRAW` arm is `do_wredraw`

**Globals read:** `G.g_cwin`, `G.g_croot`, `G.g_screen`, `G.g_desk`,
`gl_rfull`. **Written:** `G.g_wlastsel` (`do_wopen`), and through
`desk_verify` `G.g_cwin`/`G.g_croot`/`G.g_wlastsel`.

**Build configuration:** neither function is gated. `search_display` is
`CONF_WITH_SEARCH`; the `do_xyfix` nudge is `CONF_WITH_3D_OBJECTS`.

---

## `do_wopen` (`desksupp.c:294-322`)

```c
void do_wopen(WORD new_win, WORD wh, WORD curr, GRECT *pt)
{
    GRECT t;
    GRECT c, d;

    t = *pt;
    do_xyfix(&t.g_x, &t.g_y);

    if (curr > 0)
    {
        get_xywh(G.g_screen, G.g_croot, &c.g_x, &c.g_y, &c.g_w, &c.g_h);
        get_xywh(G.g_screen, curr, &d.g_x, &d.g_y, &d.g_w, &d.g_h);
        d.g_x += c.g_x;     /* convert window to screen coordinates */
        d.g_y += c.g_y;

        graf_growbox_grect(&d, &t);
        act_chg(G.g_cwin, G.g_croot, curr, &gl_rfull, FALSE, new_win?TRUE:FALSE);
    }

    if (new_win)
        wind_open_grect(wh, &t);

    G.g_wlastsel = wh;
}
```

Behaviour, step by step:

1. The target rectangle is the caller's `pt`, **copied** and then
   `do_xyfix`ed. The caller's rectangle is not modified.
2. **`curr > 0`** gates both the zoom and the deselect. `curr <= 0` means
   "no source object": `w_setpath`/`refresh_window` pass 0, a function-key
   launch passes -1 through `pro_run`, `do_dopen` passes
   `obj_get_obid(drv)`, which is 0 for a drive with no icon.
3. The zoom's **source** is the object `curr`'s own rectangle offset by
   the origin of **`G.g_croot`** — the *current* root, not `wh`'s and not
   the one `curr` necessarily lives under. For every interactive caller the
   two coincide (the icon was clicked in the current window). For
   `cnx_get` they do not after the first restored window: `do_diropen`'s
   `desk_verify` has made that window current, so the next drive icon's
   rectangle is offset by the previous window's work-area origin. That is
   the source's behaviour and is reproduced, not corrected.
4. The zoom's **target** is the fixed-up `t`: the new window's frame
   rectangle for `do_diropen`, the whole desktop (`&G.g_desk`) for
   `pro_run`.
5. `act_chg(G.g_cwin, G.g_croot, curr, &gl_rfull, FALSE, dodraw)` clears
   `curr`'s `SELECTED` bit (and its `FNODE`'s `f_selected`).
   `dodraw = new_win`: the object is repainted only when a **new** window
   opens. In the same-window case and in `pro_run` the state changes
   without a redraw — the header comment (`:282-292`) explains why: the
   window's `FNODE`s are already gone (`pn_close`) and a redraw would read
   freed memory; for `pro_run` the desktop is about to exit.
6. `new_win` → `wind_open_grect(wh, &t)`. Otherwise no AES geometry call.
7. `G.g_wlastsel = wh`, unconditionally.

Return: `void`.

## `do_diropen` (`desksupp.c:422-471`)

```c
WORD do_diropen(WNODE *pw, WORD new_win, WORD curr_icon,
                char *pathname, GRECT *pt, WORD redraw)
{
    WORD ret;

    desk_busy_on();

    if (!pn_open(pathname, pw))     /* pathname is too long */
    {
        desk_busy_off();
        return FALSE;
    }

    ret = pn_active(&pw->w_pnode, TRUE);
    if (ret < 0)    /* error reading directory */
    {
        pn_close(&pw->w_pnode);
        desk_busy_off();
        return FALSE;
    }

    win_sname(pw);
    win_sinfo(pw, FALSE);
    wind_set(pw->w_id, WF_NAME, pw->w_name, 0, 0);

    do_wopen(new_win, pw->w_id, curr_icon, pt);
    if (new_win)
        win_top(pw);

    desk_verify(pw->w_id, TRUE);

    if (redraw && !new_win)
        fun_msg(WM_REDRAW, pw->w_id, pt->g_x, pt->g_y, pt->g_w, pt->g_h);

    desk_busy_off();
    return TRUE;
}
```

Behaviour:

1. Busy pointer on for the whole body; every exit turns it off again
   (the `search_display` caller relies on that, `deskfun.c:422-426`).
2. **Two failure exits, both `FALSE`, both before any AES window call:**
   - `pn_open` refuses a `pathname` of `LEN_ZPATH` (114) characters or
     more (`deskfpd.c:73-74`);
   - `pn_active` returns `< 0`. It returns the final `Fsfirst`/`Fsnext`
     code with `ENMFIL` and `EFILNF` folded to 0
     (`deskfpd.c:297`), so an empty directory succeeds and a missing path
     (`EPTHNF`), a missing drive or a read error fails. The file list is
     released (`pn_close`).
   The caller owns the cleanup of a failed open: `do_dopen`,
   `do_fopen` (new window) and `cnx_get` `win_free` the freshly allocated
   `WNODE`; `do_dopen` additionally deselects the drive icon with a redraw
   (`act_chg(DESKWH, DROOT, curr, &gl_rfull, FALSE, TRUE)`, `:1070`).
3. Title (`win_sname`), info line (`win_sinfo(pw, FALSE)` — the whole
   directory's totals) and `wind_set(WF_NAME)`.
4. `do_wopen(new_win, …)` — zoom + deselect + (new window only)
   `wind_open`.
5. `win_top(pw)` for a new window.
6. `desk_verify(pw->w_id, TRUE)` — rebuild the view for the actual work
   area, make this window current.
7. **Redraw only for an existing window** (`redraw && !new_win`): a
   synchronous `WM_REDRAW` of `pt` through `fun_msg`. A new window is
   repainted by the AES's own `WM_REDRAW` after `wind_open`.
   `search_display` passes `redraw = FALSE`.

Return: `TRUE` on success.

## The callers' halves

- **`do_dopen(curr)`** (`desksupp.c:1041-1069`): `curr >= 0` is an
  object, its drive letter is `ib_char & 0xff`; `curr < 0` is `-drive`
  (from `check_alt_letter_key`, `deskmain.c:1157`) and the object is
  `obj_get_obid(drv)` (`deskobj.c:249-261`: the first `G_ICON` child of
  `DROOT` whose `ib_char` low byte is the letter, else 0). `valid_drive`
  (with its own alert) gates everything. `win_alloc(curr)`; no node →
  `fun_alert(1, STNOWIND)`. The path is `X:\*.*`, the rectangle the new
  node's root (`win_alloc`'s `cs_wnode[g_wcnt]` default). Always returns
  `FALSE`.
- **`do_fopen(pw, curr, pathname, allow_new_win)`** (`:1089-1160`):
  `t = WF_WXYWH` of `pw` first; a new window replaces `t` with the new
  node's root rectangle (`rc_copy`, src → dst); the same-window arm
  `pn_close`s first. A failed new-window open `win_free`s.
- **`cnx_get`** (`deskmain.c:1489-1499`): `obid = obj_get_obid(path[0])`,
  `win_alloc(obid)`, saved scroll position, `do_xyfix` on the saved
  rectangle, `do_diropen(…, TRUE, obid, …, TRUE)`, `win_free` on failure.
- **`search_display`** (`deskfun.c:391-433`): a scratch window per
  matching folder, `redraw = FALSE`.
- **`pro_run`** (`deskpro.c:61`): `do_wopen(FALSE, wh, curr, &G.g_desk)`
  with `wh = G.g_cwin` and `curr` = the launched icon (`do_aopen`'s
  `curr`, `desksupp.c:951,986,1001`), -1 from a function key
  (`deskmain.c:1102`) and `wh = -1` from the console launch
  (`deskmain.c:753`), which skips the call. Called whatever `pro_exec`
  returned. A *Cancel* in `opn_appl` never reaches `pro_run`
  (`desksupp.c:986`), so the icon stays selected then.

---

## NewTOS implementation (DESK50, 2026-10-01)

`kernel/src/desk/main.rs`:

| EmuTOS | NewTOS |
|---|---|
| `do_wopen` | `main::do_wopen(new_win, wh, curr, pt)` |
| `do_diropen` | `main::do_diropen(handle, new_win, curr_icon, pathname, title, pt, redraw)` |
| `do_dopen` | `main::do_dopen(curr)` — replaces `open_root_window` |
| `do_fopen` | `main::do_fopen` — both arms now end in `do_diropen` |
| `cnx_get`'s window loop | `main::cnx_get` |
| `w_setpath` | `main::do_close_folder` → `do_fopen(handle, 0, …, false)` |
| `win_alloc(obid)` (AES half + list half) | `main::win_alloc_window(obid)` = `wind_create` + `DeskState::win_alloc` |
| `win_free(pw)` | `main::win_free_window(handle)` = `wind_delete` + `DeskState::win_free` |
| `obj_get_obid` | `main::obj_get_obid` |
| `graf_growbox_grect` | `desk::graf_growbox` (`mod.rs`, opcode 73, 8/1/0) |
| `pro_run(…, wh, curr)` | `pro::pro_run(isgraf, cmd, tail, wh, curr)` |
| `pn_active`'s `WORD` | `PNode::pn_active` returns the folded GEMDOS code |
| `pn_open`'s `NULL` | `PNode::pn_open` returns `false` for `>= LEN_ZPATH` |

Named differences, each with its reason:

1. **`do_diropen` takes the title as a parameter.** EmuTOS's `win_sname`
   shows the whole `p_spec` (`" C:\SUB\*.* "`); NewTOS keeps the mask in
   `p_mask` and titles a window with its directory (plus a real mask) —
   a pre-existing presentation choice of `set_window_mask`/`spec_and_title`
   that DESK50 neither introduces nor changes.
2. **`search_display` has no NewTOS caller.** File → Search is built for
   the current window only (`main::do_search`, `mark_matching_fnodes`);
   the recursive descent that opens a scratch window per folder is not.
   When it is built, it opens its window through `do_diropen` with
   `redraw = false`.
3. `pn_sort` is a separate call in NewTOS (EmuTOS sorts inside
   `pn_active`); `do_diropen` runs it right after a successful scan.
4. The default rectangle of a new node is `main::win_default_rect(slot)`
   — the saved `#W` rectangle for the slot or the staggered fallback, as
   before (`deskmain.md` §cnx_get deviation 4); EmuTOS's `obj_walloc`
   copy of `cs_wnode[g_wcnt]` into the root object is not reproduced.
