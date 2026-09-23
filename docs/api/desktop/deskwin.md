# `deskwin.c` — `win_view`, `win_ocalc`, `win_bldview`, and window management

> Phase-00 extraction, Track A. Source: EmuTOS @ `1a7484ab`,
> `desk/deskwin.c` (907 lines, read in full), `desk/deskwin.h`.

## EmuTOS source

**Primary locations:**
- `desk/deskwin.c:63-99` — `win_view()`
- `desk/deskwin.c:105-135` — `win_start()`
- `desk/deskwin.c:293-372` — `win_ocalc()` (static)
- `desk/deskwin.c:392-520` — `win_bldview()`
- `desk/deskwin.c:528-546` — `win_delta()` (static)
- `desk/deskwin.c:553-636` — `win_blt()` (static)
- `desk/deskwin.c:682-762` — `win_slide()`, `win_arrow()`
- `desk/deskwin.c:768-819` — `win_srtall()`, `win_bdall()`, `win_shwall()`
- `desk/deskwin.c:825-840` — `win_isel()`
- `desk/deskwin.c:846-906` — `win_sname()`, `win_sinfo()`
- `desk/deskwin.c:141-283` — `win_free`/`win_alloc`/`win_find`/`win_top`/
  `win_onbottom`/`win_ontop`/`win_count` (**correction**: previously cited
  as `141-274`, which excludes `win_count()` at `:280-283`; verified against
  EmuTOS source — `win_free` `:141-150`, `win_alloc` `:156-190`, `win_find`
  `:198-212`, `win_top` `:218-240`, `win_onbottom` `:247-256`
  (`CONF_WITH_BOTTOMTOTOP`-gated, `:243-257` incl. the `#if`), `win_ontop`
  `:265-274`, `win_count` `:280-283`)

**Helpers called (outside this file):**
- `desk/deskobj.c:113` `obj_init()`, `:156` `obj_walloc()`, `:187`
  `obj_wfree()`, `:220` `obj_ialloc()`
- `desk/deskfpd.c` — `pn_sort()` (via `win_srtall`), the `w_pnode.p_flist`
  that `win_ocalc`/`win_bldview` walk
- `desk/deskapp.c` — `app_afind_by_name()` (via `win_icalc`, static
  helper `deskwin.c:380-384`)
- `desk/desksupp.c:202` `do_wredraw()` — called from `win_blt`,
  `win_dispfile`
- AES: `wind_calc_grect`, `wind_create_grect`, `wind_get`/`wind_set`,
  `wind_get_grect`/`wind_set_grect`, `wind_delete`, `objc_add`,
  `objc_order`, `graf_growbox_grect`/`graf_shrinkbox_grect`, `gsx_sclip`,
  `bb_screen`, `rc_intersect`/`rc_equal` — all AES/VDI-level, already
  contract-documented under `docs/api/aes/` / VDI docs; not re-derived
  here except where their **return shape** matters to the algorithm
- `util/intmath.c` (not traced) — `mul_div()` (scaled-integer multiply-
  divide, used for slider size/position — standard AES idiom)

**Headers / type definitions used:**
- `desk/deskwin.h` — `WNODE` (see `_data-model.md`), `DROOT`=1,
  `WN_DESKTOP`/`WN_REBUILD`, `LEN_FNODE`=48, `USE_WIDE_FORMAT()` macro
  (unused by the functions in this entry)
- `desk/deskfpd.h` — `FNODE`/`PNODE` (see `_data-model.md`), `V_ICON`=0/
  `V_TEXT`=1
- `desk/deskbind.h:53-56` — `MIN_WINT`/`MIN_HINT`
- `desk/deskglob.h` — `extern GLOBES G`

**System variables / globals read:**
- `G.g_iview`, `G.g_wicon`/`G.g_hicon`, `G.g_desk`, `G.g_ifit`
  (`CONF_WITH_SIZE_TO_FIT`), `G.g_icols` (same), `G.g_screen`/
  `g_screeninfo`, `G.g_iblist`, `gl_wchar`/`gl_hchar` (normal font),
  `gl_rfull` (full-screen `GRECT`, `win_blt`)

**System variables / globals written:**
- `G.g_iwext`/`g_ihext`/`g_iwint`/`g_ihint`/`g_iwspc`/`g_ihspc`
  (`win_view`)
- `G.g_icols` (`win_view`, `CONF_WITH_SIZE_TO_FIT`)
- `G.g_wdesktop.w_flags`, `G.g_wfirst`/`g_wlist`/`g_wcnt` (`win_start`)
- Per-`WNODE`: `w_cvcol`/`w_cvrow`/`w_pncol`/`w_pnrow`/`w_vncol`/`w_vnrow`
  (`win_ocalc`), `w_next`/`w_id` (alloc/free/top)
- Per-`FNODE`: `f_obid` (`win_ocalc` resets, `win_bldview` sets)
- `G.g_screen[obid]`/`G.g_screeninfo[obid]` for every placed item
  (`win_bldview`)

**Build configuration affecting this module:**
- `CONF_WITH_SIZE_TO_FIT` (default 1) — see per-function notes below;
  every reference is guarded and has a non-size-to-fit fallback path
- `CONF_WITH_BOTTOMTOTOP` (default 1) — gates `win_onbottom()` only, not
  used by the rendering core
- `CONF_WITH_SEARCH` (default 1) — gates `win_dispfile()` only

---

## Source extraction summary

`deskwin.c` owns the **window list** (`WNODE` array + z-order chain) and
the **grid math for a window's icon/text view**: `win_view()` computes
the per-item pitch once per view-type change; `win_ocalc()` (a private
helper of `win_bldview`) computes the *virtual* grid size (how many rows/
columns would be needed to show every file) and the scroll-adjusted
starting file; `win_bldview()` walks the file list from that starting
point, allocates one `OBJECT` per visible item on the AES object tree,
and sets the window's scrollbar size/position. The remaining functions
in the file are: window-list bookkeeping (`win_start`/`win_alloc`/
`win_free`/`win_find`/`win_top`/`win_ontop`/`win_count`), scrolling
(`win_delta`/`win_blt`/`win_slide`/`win_arrow`), bulk operations
(`win_srtall`/`win_bdall`/`win_shwall`), and window-chrome text
(`win_sname`/`win_sinfo`) plus a selection-walk helper (`win_isel`).

---

## `win_view()` — `deskwin.c:63-99`

Recomputes the single-item extent/interval/pitch for the **current**
`G.g_iview` (there is one active view type for the whole desktop, not
per-window):

**`V_TEXT`** (`deskwin.c:67-80`):
```
g_iwext = LEN_FNODE * gl_wchar     /* LEN_FNODE = 48 (deskwin.h:36) */
g_ihext = gl_hchar
g_iwint = 2*gl_wchar - 1           /* the '-1' byte-aligns text for a fast blit path */
g_ihint = 2
```
`g_iwint` must be `>0` to leave room in front of each text line for
rubber-band multi-selection (comment, `deskwin.c:71-73`).

**`V_ICON`** (`deskwin.c:82-87`):
```
g_iwext = G.g_wicon      /* from app_start(), see deskapp.md */
g_ihext = G.g_hicon
g_iwint = MIN_WINT        /* deskbind.h:55, resolution-dependent 4 or 8 px */
g_ihint = MIN_HINT        /* deskbind.h:56, fixed 2 px */
```

**Both cases** then (`deskwin.c:89-90`):
```
g_iwspc = g_iwext + g_iwint     /* column pitch */
g_ihspc = g_ihext + g_ihint     /* row pitch */
```

**`CONF_WITH_SIZE_TO_FIT`** (`deskwin.c:91-98`): additionally computes
`G.g_icols`, the number of columns that would fit in a **maximum-size**
window at the current resolution:
```c
GRECT t;
wind_calc_grect(1, WINDOW_STYLE, &G.g_desk, &t);   /* 1 = WC_WORK: border -> work-area size */
G.g_icols = max(1, t.g_w / G.g_iwspc);
```
`WINDOW_STYLE` = `NAME|CLOSER|MOVER|FULLER|INFO|SIZER|UPARROW|DNARROW|
VSLIDE|LFARROW|RTARROW|HSLIDE` (`deskwin.c:48-49`) — the full window
gadget set every desktop window uses (there is no per-window gadget
customisation).

**Note:** the *window* pitch (`g_iwspc`/`g_ihspc`) is a **fixed** cell
extent plus a fixed gap — unlike the *desktop*'s `g_icw`/`g_ich`
(`deskapp.md`), it does **not** redistribute leftover window width/height
evenly; any remainder past a whole number of columns/rows is simply
unused (or, if `!CONF_WITH_SIZE_TO_FIT`, contributes to the invisible
virtual-grid overflow that scrolling reveals).

---

## `win_start()` — `deskwin.c:105-135`

One-time init at desktop boot:
1. `G.g_iview = START_VIEW` (=`V_ICON`), `G.g_isort = START_SORT`
   (=`S_NAME`); `CONF_WITH_SIZE_TO_FIT`: `G.g_ifit = TRUE`.
2. `win_view()` — **must run before** `obj_init()` (explicit comment,
   `deskwin.c:117-118`), because `obj_init()`/`sob_malloc()`
   (`deskobj.c:51-70`) sizes the `g_screen[]`/`g_screeninfo[]` allocation
   using `G.g_iwspc`/`g_ihspc` (how many items *could* be visible at once
   across all `NUM_WNODES+1` possible windows, at a maximum window size —
   an approximation using `gl_wchar`/`gl_hchar` as stand-ins for
   scrollbar/title-bar thickness, per the comment at `deskobj.c:56-69`).
3. Marks `G.g_wdesktop.w_flags = WN_DESKTOP`.
4. Allocates `G.g_wlist` (`NUM_WNODES`=7 `WNODE`s), threads `w_next`
   through them, sets every `w_id = 0` (free sentinel), `G.g_wfirst =
   G.g_wlist`, `G.g_wcnt = 0`. Returns `-1` if the allocation fails (no
   further error handling shown in this file — the caller, `deskmain.c`,
   is out of scope here).

## `win_alloc()`/`win_free()`/`win_find()`/`win_top()`/`win_ontop()`/`win_count()` — `deskwin.c:141-283`

Brief, since these are window-list bookkeeping rather than rendering
math:
- `win_alloc(obid)` (`:156-190`): refuses if `g_wcnt==NUM_WNODES`; reads
  the *saved* position/size for slot `g_wcnt` from
  `G.g_cnxsave->cs_wnode[]` (EMUDESK.INF window-save data, out of scope);
  calls `obj_walloc()` to reserve a `g_screen[]` root slot at that rect;
  initialises `w_cvcol=w_cvrow=w_vncol=0` and `w_pncol`/`w_pnrow` from the
  saved rect (`(w-gl_wchar)/g_iwspc`, `(h-gl_hchar)/g_ihspc` — a rough
  estimate, superseded by `win_ocalc()`'s recompute on first
  `win_bldview()`); calls `wind_create_grect()`; on AES failure, unwinds
  via `win_free()`.
- `win_free(thewin)` (`:141-150`): `wind_delete()` (skipped if
  `w_id==-1`, a sentinel meaning "AES window was never created"),
  decrements `g_wcnt`, resets `w_id=0`, `objc_order()` to move the root
  object to the back, `obj_wfree()` to release its item objects.
- `win_find(wh)` (`:198-212`): `wh==DESKWH` (0) returns
  `&G.g_wdesktop` directly; otherwise linear-scans `G.g_wfirst`'s
  `w_next` chain by `w_id`. Returns `NULL` if not found — **every**
  caller in this file (and `desksupp.c:do_wredraw`) checks for this and
  bails out silently, defending against "window closed between event
  post and processing" races.
- `win_top(thewin)` (`:218-240`): `objc_order()` moves the root object to
  the front of its AES sibling chain (topmost paint order), then
  re-splices `thewin` to the front of the `w_next` z-order chain.
- `win_ontop()` (`:265-274`): reads `G.g_screen[ROOT].ob_tail` (the AES
  sibling-order **last-drawn**, i.e. topmost, child) and returns its
  `WNODE*` **only if** that object has non-zero width/height (a
  zero-sized root means no window is actually open on top — e.g. only
  the desktop is showing).
- `win_count()` (`:280-283`): returns `G.g_wcnt`.

---

## `win_ocalc()` (static) — `deskwin.c:293-372`

```c
static void win_ocalc(WNODE *pwin, WORD wfit, WORD hfit, FNODE **ppstart)
```
Computes the window's virtual/physical grid dimensions and the `FNODE*`
to start displaying from, given `wfit`/`hfit` (how many whole items fit
horizontally/vertically in the *current* window size — computed by the
caller, `win_bldview`, as `window_width/g_iwspc`, `window_height/
g_ihspc`).

1. **Clamp** `wfit`/`hfit` to at least 1 (`:298-301`) — relevant in
   `V_TEXT` mode with a very narrow window.
2. **Count files** (`:307-312`): walk `pwin->w_pnode.p_flist`, reset
   every `f_obid = NIL` (marks "not currently displayed" before the
   rebuild decides which ones are), count `cnt`.
3. **Virtual grid** (`:320-337`, `CONF_WITH_SIZE_TO_FIT`):
   - if `G.g_ifit` (size-to-fit **on**): `w_vncol = wfit` (the virtual
     grid is exactly as wide as the current window — i.e. no horizontal
     scrolling in size-to-fit mode), `w_vnrow = ceil(cnt/wfit)`.
   - else (size-to-fit **off**, fixed max-width virtual grid):
     `w_vncol = min(cnt, G.g_icols)` (from `win_view()`'s max-window-width
     column count), `w_vnrow = ceil(cnt/G.g_icols)`.
   - both clamped to `>=1`.
   - (`!CONF_WITH_SIZE_TO_FIT` build: only `w_vnrow = ceil(cnt/wfit)` is
     computed; `w_vncol` doesn't exist as a field concern in that
     variant's semantics — not applicable to NewTOS, see `_overview.md`.)
4. **Physical grid** (`:343-344`): `w_pncol = wfit` (always — physical
   column count is simply what fits); `w_pnrow = min(hfit, w_vnrow)` (no
   point reserving more physical rows than there are virtual rows).
5. **Re-clamp current scroll position** if the window just grew
   (`:350-357`): for each axis, `while ((vn - cv) < space) cv--;` where
   `space = min(fit, vn)` — i.e. if the window is now large enough that
   the old scroll position `cv` would leave blank space at the
   bottom/right (fewer than `space` virtual rows/cols remaining below/
   right of `cv`), walk `cv` backwards one step at a time until the
   visible window is fully "backed" by virtual content. This is a
   **decrement-only** loop — it never *increases* `cv` (a shrinking
   window is handled purely by `w_pnrow`/`w_pncol` shrinking, not by
   moving `cv`).
6. **Compute the starting `FNODE`** (`:363-371`): the linear index of the
   top-left visible item is
   `start = cvrow*vncol + cvcol` (size-to-fit) or `start = cvrow*pncol`
   (fixed-width virtual grid) — then walks `p_flist` forward `start`
   times (**O(n) list walk**, not an indexed array access — a plain
   linked-list is being used as if it were 2-D addressable memory).

**Contract:** none of this (internal scroll-state bookkeeping). The
*visible effect* — which files appear at which screen position for a
given scroll state — is the kind of thing a user notices but not
something another *program* can observe through a documented API call.

---

## `win_bldview()` — `deskwin.c:392-520`

```c
void win_bldview(WNODE *pwin, GRECT *r)
```
The main "rebuild this window's displayed items" entry point. `r` is the
window's current work-area rectangle (already looked up by the caller via
`wind_get_grect(handle, WF_WXYWH, &r)` at every call site in this
codebase).

1. **Reset**: `r_get(r,&x,&y,&w,&h)`; `obj_wfree(pwin->w_root, x,y,w,h)`
   (`:405-408`) — frees all of this window's previously-built item
   objects back onto the global free chain (`deskobj.c:187-211`) **and**
   resizes the window's root object to the new work-area rect in the
   same call.
2. **`win_ocalc(pwin, w/g_iwspc, h/g_ihspc, &pstart)`** (`:411`) — derives
   `wfit`/`hfit` straight from the pixel work-area size divided by the
   current pitch (integer division — a partially-visible trailing
   row/column at the window edge is *not* counted as "fits").
3. **Effective fit counts for this build** (`:412-417`):
   ```c
   #if CONF_WITH_SIZE_TO_FIT
   o_wfit = min(pwin->w_pncol + 1, pwin->w_vncol - pwin->w_cvcol);
   #else
   o_wfit = pwin->w_pncol;
   #endif
   o_hfit = min(pwin->w_pnrow + 1, pwin->w_vnrow - pwin->w_cvrow);
   ```
   **Edge case discovered in source:** the `+1` is deliberate — one *more*
   row/column than what `win_ocalc` computed as "fits" is actually built,
   up to the remaining virtual extent. This lets a partially-clipped
   extra row/column of icons be object-tree-built (and therefore
   drawn, clipped by the AES window's own work-area clip) at the
   fringe, so scrolling by a fraction of an item still shows a sliver of
   the next one rather than an abrupt hard cut at a whole-item boundary.
4. **Placement loop** (`:418-496`): `r_cnt`/`c_cnt` start at 0; while
   `c_cnt<o_wfit && r_cnt<o_hfit && pstart`:
   - `yoff = r_cnt*g_ihspc`, `xoff = c_cnt*g_iwspc` (cell origin, pixel,
     relative to the window's work area).
   - `obid = obj_ialloc(pwin->w_root, xoff+g_iwint, yoff+g_ihint,
     g_iwext, g_ihext)` (`:431-432`) — **the item's own box is inset from
     the cell's top-left corner by the full gap (`g_iwint`/`g_ihint`)**;
     i.e. the gap is not split before/after — it is entirely a *leading*
     margin, applied uniformly including before the very first
     column/row (so there is also a `g_iwint`/`g_ihint` margin between
     the window's own work-area edge and the first item).
   - On allocation failure (`!obid`, free chain exhausted): `KDEBUG` and
     `break` — the remaining files are simply not displayed this build
     (no error surfaced to the user beyond an incomplete window).
   - `pstart->f_obid = obid`; `G.g_screeninfo[obid].fnptr = pstart`.
   - Build the object depending on `G.g_iview`:
     - **`V_TEXT`** (`:452-459`): `ob_type=G_USERDEF`, `ob_spec` points at
       the `SCREENINFO`'s embedded `USERBLK`; `ub_code = dr_code` (the
       text-line-draw callback, `desk/deskinf.c:331`, **not traced** —
       flagged needs-deeper-analysis if a later task covers text-view
       rendering); `ub_parm = (LONG)pstart` (the `FNODE*` to format);
       calls `win_icalc(pstart, pwin)` to resolve `f_pa`/`f_isap`.
     - **`V_ICON`** (`:460-481`): calls `win_icalc()` first; picks
       `i_index` = `anode->a_aicon` or `a_dicon` (depending on
       `f_isap`) if a matching `ANODE` was found, else a hard-coded
       default (`IG_FOLDER` for a directory, else `IG_APPL`/`IG_DOCU` by
       `f_isap` — **not** looked up in `g_iblist` by content type, just
       these three fallback indices; `KDEBUG`'d as "NULL anode, using
       defaults", `:468-469`); `memcpy(ib, &G.g_iblist[i_index],
       sizeof(ICONBLK))` (a **fresh per-instance copy**, not a shared
       pointer — see `_data-model.md` `SCREENINFO`); `ib_ptext =
       pstart->f_name`; if an `ANODE` was found, `ib_char |=
       anode->a_letter` (drive-letter overlay — for regular files this is
       almost always `a_letter==0`, a no-op OR). **`ib_xicon` is *not*
       recentred here** (contrast with `app_blddesk`'s desktop-icon path,
       see `deskapp.md`'s asymmetry note).
   - Advance: `pstart = pstart->f_next`; `c_cnt++`; if `c_cnt==o_wfit`
     (row full): `r_cnt++`, `c_cnt=0`, and (`CONF_WITH_SIZE_TO_FIT`) skip
     `pwin->w_vncol - o_wfit` further `FNODE`s (`:490-494`) — accounts for
     virtual columns that exist but are scrolled off to the right of the
     currently visible `o_wfit` columns, so the *next visible row*'s
     first file is correctly the one at virtual column `cvcol`, not
     `cvcol + o_wfit`.
5. **Scrollbar size/position** (`:498-519`):
   ```c
   #if CONF_WITH_SIZE_TO_FIT
   sl_size  = mul_div(pncol, 1000, vncol);
   wind_set(wh, WF_HSLSIZ, sl_size, 0,0,0);
   sl_value = (vncol>pncol) ? mul_div(cvcol, 1000, vncol-pncol) : 0;
   wind_set(wh, WF_HSLIDE, sl_value, 0,0,0);
   #else
   wind_set(wh, WF_HSLSIZ, 1000, 0,0,0);   /* full-width slider = no horizontal scroll */
   #endif
   sl_size  = mul_div(pnrow, 1000, vnrow);
   wind_set(wh, WF_VSLSIZ, sl_size, 0,0,0);
   sl_value = (vnrow>pnrow) ? mul_div(cvrow, 1000, vnrow-pnrow) : 0;
   wind_set(wh, WF_VSLIDE, sl_value, 0,0,0);
   ```
   Slider size/position are expressed in **thousandths** (the AES
   `WF_HSLSIZ`/`WF_VSLSIZ`/`WF_HSLIDE`/`WF_VSLIDE` contract, already
   documented in `docs/api/aes/window-graphics.md`) — `mul_div` is a
   scaled integer multiply-then-divide (no float). Position is `0` when
   there's no scroll range (`vn<=pn`).

**Contract:** `win_bldview()`'s *output* — the `OBJECT`/`ICONBLK` tree it
builds — uses contract-documented struct layouts (`object.md`,
`iconblk.md`) and the contract `WF_HSLSIZ` etc. `wind_set` calls. The
*algorithm* that decides which items go where (the grid math above) is
internal — but it is exactly the algorithm a faithful "looks like TOS"
rebuild needs to reproduce for a matching user experience, even though no
program can observe it through an API.

---

## `win_delta()`/`win_blt()` (static) — `deskwin.c:528-636`

Scrolling mechanics, briefly (not the focus of this task but read in
full per protocol):

- `win_delta(pw, horizontal, newcv)` (`:528-546`): clamps `newcv>=0`,
  then further clamps to `min(vn-pn, newcv)` (can't scroll past the last
  full page), returns the **delta** from the current `cv`.
- `win_blt(pw, horizontal, newcv)` (`:553-636`): computes `delcv` via
  `win_delta`; if zero, no-op. Updates `w_cvcol`/`w_cvrow` by `delcv`,
  re-runs `win_bldview()` for the new scroll position, then — as a
  performance optimisation — checks if the window is fully on-screen
  (`WF_FIRSTXYWH` intersected with `gl_rfull` equals the work rect
  exactly, i.e. no overlapping window covers any part of it) and if so,
  **blits (`bb_screen`)** the still-valid portion of the old image by
  `delcv` cells rather than redrawing it from scratch, shrinking the
  clip rect `c` to just the newly-exposed strip; finally calls
  `do_wredraw(pw->w_id, &c)` (see `desksupp-redraw.md`) to paint
  whatever wasn't (or couldn't be) blitted. If any other window overlaps,
  the blit optimisation is skipped entirely and the *whole* rect is
  redrawn via `do_wredraw`.
- `win_slide()`/`win_arrow()` (`:682-762`): translate an AES slider drag
  value (0-1000) or an arrow-gadget click (`WA_UPPAGE`/`WA_DNLINE`/…,
  `CONF_WITH_SIZE_TO_FIT` also handles the horizontal `WA_LF*`/`WA_RT*`
  variants) into a `newcv` and call `win_blt`.

---

## `win_srtall`/`win_bdall`/`win_shwall`/`win_isel`/`win_sname`/`win_sinfo` — `deskwin.c:768-906`

- `win_srtall()` (`:768-780`): for every open window, resets `w_cvrow=0`
  and re-sorts `w_pnode.p_flist` via `pn_sort()` (does **not** rebuild the
  view itself — a separate `win_bdall()`/redraw is expected afterwards).
- `win_bdall()` (`:786-799`): for every open window, re-fetches its
  work-area rect and calls `win_bldview()` — rebuild without redraw.
- `win_shwall()` (`:805-819`): for every open window **except the
  desktop** (`wh != DESKWH`), synthesizes a `WM_REDRAW` message via
  `fun_msg()` (out of scope, `deskfun.c`) for its full work-area rect —
  i.e. redraw-all is done by *injecting AES redraw messages*, not by
  calling `do_wredraw` directly.
- `win_isel(olist, root, curr)` (`:825-840`): starting from object
  `curr` (or the root's first child if `curr==0`), walks `ob_next`
  siblings looking for the next one with `SELECTED` set in `ob_state`;
  stops (`return 0`) once it walks past `root` itself (`curr <= root`
  test as the loop guard, since sibling index values are compared
  numerically against `root`).
- `win_sname(pw)` (`:846-859`): builds `w_name` as `" " + p_spec + " "`
  (leading/trailing space padding, no truncation check against the
  `LEN_ZPATH+2` buffer size visible in this function — relies on
  `p_spec` already being within `LEN_ZPATH`, enforced by `pn_open`).
- `win_sinfo(pwin, check_selected)` (`:867-906`): optionally counts
  selected `FNODE`s (`f_selected`) and sums their `f_size`; picks between
  two resource-string templates (`STINFST2` if anything is selected,
  else `STINFOST` with the *whole* directory's `p_count`/`p_size`);
  `sprintf`s into `pwin->w_info` (72-byte buffer, sized per the header
  comment for the longest known translated resource string) and
  `wind_set(..., WF_INFO, ...)`.
  **The two strings, verbatim** (`desk/desk_rsc.c:2468-2469`,
  `desk_rs_fstr[0..1]`): `STINFOST` = `" %ld bytes used in %d items."`,
  `STINFST2` = `" %ld bytes used in %d selected items."` — leading blank,
  **size first**, count second, trailing full stop, and the argument order
  of the `sprintf(pwin->w_info, alert, select_size, select_count)` at
  `:905`. **NewTOS (A109, 2026-09-22):** `kernel/src/desk/main.rs`
  `fmt_info` writes the `STINFOST` form; it had its own wording and the
  numbers the other way round (`" N items   M bytes used"`).
  **NewTOS (A110, 2026-09-23):** `fmt_info` takes the `selected` flag and
  writes the `STINFST2` form for it, and `kernel/src/desk/main.rs`
  `win_sinfo(handle, check_selected)` is the function above — count the
  selected `FNODE`s, fall back to the directory totals when none is selected
  (which is what makes a *deselect* put the byte count back), `wind_set(…,
  WF_INFO, …)` by address. The desktop handle is passed through as in EmuTOS:
  `win_find(DESKWH)` answers `&G.g_wdesktop` (`:202-203`), whose file list is
  empty, and the `wind_set` lands on a window that is `VF_INUSE` but never
  `VF_ISOPEN`, so it stores a pointer and draws nothing.

  **Call sites, all five of EmuTOS's.** `win_sinfo(pw, FALSE)` — the whole
  directory — stays where it was, in `set_window_strings`
  (`do_diropen`/`rebuild_window`, `desksupp.c:450`, `deskfun.c:141`).
  `win_sinfo(pw, TRUE)` now runs from:

  | EmuTOS | NewTOS |
  |---|---|
  | `deskmain.c:982-983` (single click, after `act_bsclick`/`act_bdown`) | `main.rs` `hndl_button` |
  | `desksupp.c:150-153` (`desk_clear`, and through it `desk_clear_all`) | `main.rs` `desk_clear`, the wrapper around the state half |
  | `deskfun.c:380-382` (`mark_matching_fnodes`) | `main.rs` `search_current_window` |
  | `deskfun.c:627-631` (`fun_selall`) | `dir.rs` `fun_selectall` |

---

## Edge cases discovered in source

- The window pitch (`g_iwspc`/`g_ihspc`) is a **fixed** gap-plus-extent,
  unlike the desktop's evenly-redistributed `g_icw`/`g_ich` — see
  `deskapp.md`.
- `win_ocalc`'s scroll-declamp loop only ever **decrements** `cv` — a
  shrinking window never needs to *increase* `cv` because `w_pnrow`/
  `w_pncol` (not `cv`) shrink to match.
- `win_bldview`'s `o_wfit`/`o_hfit` `+1` — builds one extra
  partially-visible row/column at the fringe (see above); easy to miss
  since `win_ocalc`'s own `w_pncol`/`w_pnrow` do *not* include this `+1`.
- The item-box inset (`xoff+g_iwint`, `yoff+g_ihint`) applies the full
  gap **before every item including the first** — not split
  symmetrically, and not omitted for the first column/row.
- `win_bldview`'s icon-selection fallback (`IG_FOLDER`/`IG_APPL`/
  `IG_DOCU`) when no matching `ANODE` is found bypasses the
  `a_aicon`/`a_dicon` lookup entirely — a plain file with no installed
  application match always gets the generic document icon, never a
  per-extension icon (extension-specific icons only exist via an
  installed `ANODE`, out of scope for this task).
- `ib_xicon` is **not** recentred for window icons (only for desktop
  icons, in `app_blddesk` — see `deskapp.md`).

---

## Dependencies

### Requires (must be initialized / functional)

- `app_start()` — must have set `G.g_wicon`/`g_hicon` before `win_view()`
  is called in `V_ICON` mode. **Verified boot order**
  (`desk/deskmain.c:2018,2028`): `app_start()` runs, then `win_start()`
  (whose first action is `win_view()`) — so `win_view()`'s `V_ICON` case
  always sees an already-computed `G.g_wicon`/`g_hicon`. The *in-file*
  comment at `deskwin.c:117-118` ("`obj_init()` must be called after
  `win_view()`, because it uses `G.g_iwspc`/`g_ihspc`") is about the
  ordering **within** `win_start()`, not between `app_start()` and
  `win_start()`.
- `pn_active()`/`pn_sort()` (`deskfpd.c`) — the window's `FNODE` list must
  already be built and sorted before `win_bldview()` is called
- `obj_init()` (`deskobj.c`) — `G.g_screen[]`/`g_screeninfo[]` must exist

### Used by

- `desksupp.c:desk_verify()` — calls `win_bldview()` after a window-size
  change
- `deskapp.c:app_blddesk()` — indirectly shares `G.g_wicon`/`g_hicon` with
  `win_view()`'s `V_ICON` case, but does not call into `deskwin.c` itself
  for its own (desktop) placement
- `deskact.c`, `deskfun.c` (out of scope) — call `win_bldview`/`win_blt`/
  `win_slide`/`win_arrow` in response to user actions

## Related data structures

- `docs/api/desktop/_data-model.md` — `WNODE`, `FNODE`, `PNODE`, `GLOBES`
  view-parameter fields
- `docs/api/desktop/deskapp.md` — `g_wicon`/`g_hicon`/`g_icw`/`g_ich`
  derivation
- `docs/api/aes/window-graphics.md` — `wind_calc_grect`, `WF_HSLSIZ`/
  `WF_VSLSIZ`/`WF_HSLIDE`/`WF_VSLIDE`, `WF_FIRSTXYWH`/`WF_NEXTXYWH`
- `docs/api/structures/object.md`, `docs/api/structures/iconblk.md`

## NewTOS implementation notes

(Empty during Phase 00 / this extraction task.)
