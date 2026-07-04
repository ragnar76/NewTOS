# `deskapp.c` — icon/grid metrics (`app_start`, `app_blddesk` consumption)

> Phase-00 extraction, Track A. Source: EmuTOS @ `1a7484ab`,
> `desk/deskapp.c`, `desk/deskapp.h`. This entry covers **only** the
> icon-geometry / grid-metric portion of `app_start()` and how
> `app_blddesk()` consumes it. The rest of `deskapp.c` (ANODE list
> management, EMUDESK.INF parse/build, icon loading `app_rdicon`, install
> dialogs) is narrated in `docs/api/desktop.md` and not re-derived here
> except where it feeds the metrics below.

## EmuTOS source

**Primary locations:**
- `desk/deskapp.c:781-1014` — `app_start()` (icon metrics at
  `:810-817`, per-`ANODE` pixel placement at `:979-1004`)
- `desk/deskapp.c:1306-1331` — `set_background()`
- `desk/deskapp.c:1337-1404` — `app_blddesk()` (consumes `g_wicon`/
  `g_hicon` at `:1362-1363`, recentres `ib_xicon` at `:1391`)
- `desk/deskapp.c:454-532` — `setup_iconblks()`, the helper (called from
  `app_rdicon()`, `:662-674`, either directly for the built-in set at
  `:673` or indirectly via `load_icon_file()`/`setup_mono_icons()` for a
  user-supplied resource) that allocates `G.g_iblist[]`/transforms the
  bitmaps and contains the "fix up the ICONBLKs" loop (`:509-521`), which
  sets `ib_wtext`/`ib_htext`/`ib_ytext` for **every** loaded icon block,
  but leaves `ib_xtext` untouched

**Helpers called:**
- `desk/deskobj.c:220` — `obj_ialloc()` — allocates + places one item
  `OBJECT` (used by `app_blddesk` for desktop icons, by `win_bldview` for
  window items)
- `desk/deskapp.c` (not traced in this task) — `snap_icon()` — collision
  avoidance when placing a new desktop icon; **needs deeper analysis** if
  a later task covers desktop-icon installation
- `aes/gemwrect.c` (not traced) — `r_set`/`r_get` GRECT helpers

**Headers / type definitions used:**
- `include/obdefs.h` — `ICONBLK` (see `docs/api/structures/iconblk.md`)
- `desk/deskbind.h:47-56` — `SMALL_HSPC`/`LARGE_HSPC`/`MIN_WINT`/`MIN_HINT`
- `desk/deskapp.c:74` — `MAX_ICONTEXT_WIDTH` (12)

**System variables / globals read:**
- `G.g_desk` (`desk/deskbind.h:164`) — desktop work-area `GRECT`
- `G.g_iblist[0]` — the hard-disk icon's `ICONBLK` (`ib_xtext`, `ib_hicon`,
  `ib_wicon`)
- `gl_wschar` (`include/gsxdefs.h:85`) — width of the **small font**
  character cell (used for icon labels; distinct from `gl_wchar`, the
  normal/window-text font)

**System variables / globals written:**
- `G.g_wicon`/`G.g_hicon` (`desk/deskapp.c:810-811`)
- `G.g_icw`/`G.g_ich` (`desk/deskapp.c:814,817`)
- `G.g_xyicon[]`/`G.g_xytext[]` (`desk/deskapp.c:990-1011`)
- `pa->a_xspot`/`pa->a_yspot` for every `AF_ISDESK` `ANODE`
  (`desk/deskapp.c:983-985`)

**Build configuration affecting this function:**
- `CONF_WITH_BACKGROUNDS` (default 1) — `set_background()`'s `ob_spec`
  value; `INITIAL_ICON_STATE` in `deskwin.c` (see `deskwin.md`)
- `CONF_WITH_SIZE_TO_FIT` (default 1) — does **not** affect the metrics
  in this file directly (it affects `win_view`/`win_ocalc`, see
  `deskwin.md`)
- Neither is TARGET_192-disabled behaviour relevant to NewTOS (RPi4 build
  follows the full/general variant — see `_overview.md`)

---

## Source extraction summary

`app_start()` runs once at desktop boot (after the icon set is loaded by
`app_rdicon()`) and derives two related but **distinct** pairs of
numbers:

1. **`g_wicon`×`g_hicon`** — the size of **one icon cell** (icon bitmap +
   its label, as a single bounding box), derived from the hard-disk icon
   (`G.g_iblist[0]`) and the current small-font metrics. This value is
   used both for **desktop** icons (`app_blddesk`) and for **window**
   icon-view items (`win_view`, see `deskwin.md`) — there is only one
   icon-cell size system-wide, not one per icon type or per window.

2. **`g_icw`×`g_ich`** — the **desktop's** grid pitch (column/row pixel
   step), computed by fitting as many `g_wicon`+`MIN_WINT`-wide columns
   (resp. `g_hicon`+`MIN_HINT`-tall rows) as possible into `g_desk`, then
   **redistributing the desktop's exact pixel width/height evenly**
   across that many columns/rows — so the last column never has leftover
   slack, and spacing looks symmetric at every resolution. This is a
   *different* pitch computation than the one `win_view()` uses inside a
   window (see `deskwin.md`): windows do **not** evenly redistribute —
   they pack items left-to-top with a fixed `MIN_WINT`/`MIN_HINT` gap and
   let any remainder be unused/scrollable space at the window's right/
   bottom edge.

`app_blddesk()` then places each desktop `ANODE` at its (already
pixel-converted, see `_data-model.md`) `a_xspot`/`a_yspot`, sized
`g_wicon`×`g_hicon`, and **recentres the icon bitmap inside that cell
every time** using the *live* `g_wicon` value — but does not touch the
label's centering offset (`ib_xtext`), which was set once, resolution-
independently, when the icon was loaded.

---

## The exact grid-metric formulas

### 1. Icon-cell size (`g_wicon`/`g_hicon`) — `deskapp.c:810-811`

```
G.g_wicon = (MAX_ICONTEXT_WIDTH * gl_wschar) + (2 * G.g_iblist[0].ib_xtext);
G.g_hicon = G.g_iblist[0].ib_hicon + gl_hschar + 2;
```

- `MAX_ICONTEXT_WIDTH` = 12 (`deskapp.c:74`) — the maximum label length in
  characters (excluding the trailing NUL) that the desktop ever displays
  under an icon; longer names are truncated elsewhere (out of scope here).
- `gl_wschar`/`gl_hschar` — small-font character cell width/height
  (`gsxdefs.h:85-86`), i.e. **not** the same font metric as window title
  text (`gl_wchar`/`gl_hchar`).
- `G.g_iblist[0]` is **always** the hard-disk icon (`IG_HARD`=0,
  `deskapp.h:54`) — `app_start()` uses **only this one icon's** dimensions
  for the universal cell size, relying on the documented assumption
  (`deskapp.c:462-464` comment) that "all icons have the same width &
  height as the first." A user- or theme-supplied icon set with a
  differently-sized icon #0 would silently resize *every* icon's cell.
- **`ib_xtext` is not recomputed here** — it is whatever value the icon
  resource (`icon.rsc`, compiled at build time from `icon.def` by an
  external `erd` tool; **not present as generated C** in this checkout —
  see below) carried for icon #0. `setup_iconblks()`'s "fix up" loop
  (`deskapp.c:509-521`, called from `app_rdicon()`, `:662-674`) explicitly
  sets `ib_ytext`/`ib_wtext`/`ib_htext` for every icon but **does not
  touch `ib_xtext`**.
- **Geometric reading:** if a label box of width `wtext` is meant to be
  horizontally centred within a cell of width `W`, then
  `xtext = (W - wtext) / 2`, i.e. `W = wtext + 2*xtext`. Substituting
  `wtext = MAX_ICONTEXT_WIDTH*gl_wschar` (this is exactly what
  `setup_iconblks()` sets `ib_wtext` to, `deskapp.c:519`) gives precisely the
  `g_wicon` formula above. **This only produces a correctly-centred label
  at the font size the resource's `ib_xtext` was authored for** — since
  `wtext` scales with the live `gl_wschar` but `xtext` is a fixed baked-in
  resource constant, a resolution/font change that alters `gl_wschar`
  shifts the label's centring, not just its width. This is an EmuTOS
  quirk inherent to the resource format, not something a rebuild needs to
  reproduce byte-for-bit (visual centring is not contract) — but it is
  the authoritative reasoning behind why the cell width is
  `wtext + 2*xtext` and not simply `wtext`.
- **STOP / needs deeper analysis:** the *numeric* value of
  `G.g_iblist[0].ib_xtext` (i.e. what the built-in hard-disk icon's
  `icon.rsc` actually encodes) could not be extracted from this checkout.
  `desk/icon.rsc`/`icon.def` are present, but the compiled `icons.c`
  (whose `icon_rs_iconblk[]` array `app_rdicon()` passes to
  `setup_iconblks()` as `ibstart`, `deskapp.c:673`) is generated at
  EmuTOS build time by an external resource
  decompiler (`erd`) that is not part of this reference tree, and
  decoding the binary `.rsc` `ICONBLK` table directly would require
  reverse-engineering the GEM resource file's extension-block layout — out
  of scope for a documentation task per the STOP rule. **If a later task
  needs the exact baked pixel value, it must build EmuTOS (or acquire
  `erd`) and inspect the generated `icons.c`.**

### 2. Desktop grid pitch (`g_icw`/`g_ich`) — `deskapp.c:813-817`

```
xcnt = G.g_desk.g_w / (G.g_wicon + MIN_WINT);   /* max columns that fit */
G.g_icw = G.g_desk.g_w / xcnt;                  /* pitch: redistribute width evenly */

ycnt = G.g_desk.g_h / (G.g_hicon + MIN_HINT);   /* max rows that fit */
G.g_ich = G.g_desk.g_h / ycnt;                  /* pitch: redistribute height evenly */
```

- `MIN_WINT`/`MIN_HINT` (`deskbind.h:53-56`):
  ```c
  #define SMALL_HSPC  4   /* horizontal, ST low/medium, Falcon double-line */
  #define LARGE_HSPC  8   /* horizontal, ST high, Falcon not-double-line */
  #define MIN_WINT    ((gl_height<=300) ? SMALL_HSPC : LARGE_HSPC)
  #define MIN_HINT    2   /* vertical, fixed */
  ```
  i.e. the *minimum* required horizontal gap is itself resolution-
  dependent (4px in low/medium-height modes, 8px in high-res modes),
  chosen because "because the desktop (between resolution changes) is a
  fixed size, the actual spacing between desktop icons is adjusted for
  symmetrical spacing" (`deskbind.h:47-52` comment).
- Both divisions are **integer** (`WORD` arithmetic) — `xcnt`/`ycnt` are
  floored column/row counts, and `g_icw`/`g_ich` is then the *exact*
  integer pixel pitch that makes `xcnt`/`ycnt` columns/rows tile
  `g_desk.g_w`/`g_h` **exactly** (any remainder from the floor division
  is absorbed into a *slightly wider* pitch for every column/row, not
  left as unused space at one edge) — this is the "adjusted for
  symmetrical spacing" behaviour. Concretely: `g_icw >= g_wicon+MIN_WINT`
  always (by construction, since `xcnt` was chosen as the floor), and the
  excess `g_icw - (g_wicon+MIN_WINT)` is spread across every column
  equally by virtue of being a per-column pitch rather than a fixed gap.
- **This is the desktop-only pitch.** A directory *window*'s icon-view
  pitch (`g_iwspc`/`g_ihspc`, see `deskwin.md`) is computed differently —
  `g_wicon+MIN_WINT` directly, **without** the redistribution step — so
  windows can have slightly tighter or looser spacing than the desktop at
  the same resolution, and (unlike the desktop) any leftover window width
  simply becomes unused/scrollable space.

### 3. Per-`ANODE` desktop placement (pixel conversion) — `deskapp.c:979-987`

```c
for (pa = G.g_ahead; pa; pa = pa->a_next) {
    if (pa->a_flags & AF_ISDESK) {
        x = pa->a_xspot * G.g_icw;
        y = pa->a_yspot * G.g_ich + G.g_desk.g_y;
        snap_icon(x, y, &pa->a_xspot, &pa->a_yspot, 0, 0);
    }
}
```

Converts the EMUDESK.INF-supplied **grid-unit** spot (`a_xspot`/`a_yspot`
as parsed — column/row indices) into pixel coordinates, offsetting `y` by
the desktop's top (`g_desk.g_y`, below the menu bar), then hands both to
`snap_icon()` (not traced — collision avoidance / nudging to an unused
spot) which writes the **final pixel** coordinates back into the same
`a_xspot`/`a_yspot` fields. See `_data-model.md` for the full unit-
lifecycle note (grid units in the INF file, pixels in memory afterwards).

### 4. `app_blddesk()`'s consumption (`deskapp.c:1358-1399`)

```c
obid = obj_ialloc(DROOT, pa->a_xspot, pa->a_yspot, G.g_wicon, G.g_hicon);
...
pic->ib_xicon = ((G.g_wicon - pic->ib_wicon) / 2);   /* deskapp.c:1391 */
```

Every desktop icon `OBJECT` is placed at its (pixel) spot, sized exactly
`g_wicon`×`g_hicon` — i.e. **the object box equals the icon cell**, not
just the icon bitmap. The icon **bitmap**'s horizontal offset within that
box (`ib_xicon`) is explicitly **recentred using the live `g_wicon`**
(overriding whatever `ib_xicon` the source `ICONBLK` carried) — but the
**label**'s offset (`ib_xtext`, copied verbatim via the preceding
`memcpy(pic, &G.g_iblist[icon], sizeof(ICONBLK))`, `deskapp.c:1390`) is
**not** recentred here.

**Edge case discovered in source — icon-bitmap vs. label recentring
asymmetry:** `app_blddesk()` (desktop icons) recomputes `ib_xicon` live
against `g_wicon` (so the bitmap is always correctly centred in the cell,
regardless of resolution/font), but `win_bldview()` (window icons, see
`deskwin.md`) does **not** perform this recentre — window icons rely on
whatever `ib_xicon` came from `G.g_iblist[i_index]` unchanged. Both paths
leave `ib_xtext` untouched in all cases. Net effect: the **icon bitmap**
is guaranteed centred on the desktop but only "as-authored" in windows;
the **label** is only "as-authored" (both places) — a discrepancy that
only reading both call sites side-by-side reveals.

---

## `set_background()` (`deskapp.c:1306-1331`)

Not a grid-metric function, but the other half of what `app_blddesk()`
does before placing icons — included here for completeness since
`docs/api/desktop.md` already covers its narrative (default `g_patcol`
values, `#Q` INF line). Reproduced structurally: chooses a plane-count
bucket `n` (0 = 1-plane, 1 = 2-plane, 2 = other), sets every window
object's `ob_spec = G.g_patcol[n].window | BORDER_TEXT_COLOURS` and
`G.g_screen[DROOT].ob_spec = G.g_patcol[n].desktop | BORDER_TEXT_COLOURS`
(`CONF_WITH_BACKGROUNDS`); otherwise (`#else`) `G.g_screen[DROOT].ob_spec
= AP_PRIVATE` (a fixed AES-private fill, no user pattern/colour choice).
`BORDER_TEXT_COLOURS` = `(BLACK<<12)|(BLACK<<8)` (`deskapp.c:80`) — border
and text colour nibbles fixed to black regardless of the pattern/colour
choice.

---

## Edge cases discovered in source

- `g_wicon`/`g_hicon` derive from icon **index 0 only** — see formula
  discussion above. A theme/user icon set is implicitly required to keep
  icon #0's dimensions consistent with the rest, or all cells resize.
- `ib_xtext`'s resolution-dependence (label centring only correct at the
  font size the resource was authored for) — see above; visual-only, not
  contract.
- The icon-bitmap-vs-label recentring asymmetry between `app_blddesk`
  (recentres bitmap) and `win_bldview` (doesn't) — see above.
- `g_icw`/`g_ich` symmetric-redistribution vs. `g_iwspc`/`g_ihspc`'s
  plain fixed-gap pitch (desktop vs. window) — two different pitch
  algorithms for what looks like "the same" grid; conflating them would
  make desktop and window icon spacing subtly different from EmuTOS.
- `a_xspot`/`a_yspot` unit change (grid units → pixels) happens exactly
  once, in `app_start()`'s loop, via in-place field overwrite — see
  `_data-model.md`.

---

## Dependencies

### Requires (must be initialized / functional)

- `app_rdicon()` — must have populated `G.g_iblist[]` (specifically index
  0) before `app_start()` computes `g_wicon`/`g_hicon`
- `G.g_desk` — must already reflect the current resolution's desktop
  work-area rectangle
- `gl_wschar`/`gl_hschar` — AES font-metric globals, set during AES/VDI
  startup (`aes/gemgraf.c`, out of scope here)

### Used by

- `app_blddesk()` — desktop icon sizing/placement, `ib_xicon` recentre
- `win_view()` (`deskwin.c:83-86`, see `deskwin.md`) — `V_ICON` case sets
  `G.g_iwext = G.g_wicon`, `G.g_ihext = G.g_hicon`, so the same universal
  cell size governs window icon-view spacing too
- `deskrez.c` (out of scope, resolution change) — presumably re-triggers
  `app_start()`'s metric recompute on a resolution switch; not traced in
  this task

## Related data structures

- `docs/api/desktop/_data-model.md` — `GLOBES` field table (`g_wicon`,
  `g_hicon`, `g_icw`, `g_ich`, `g_iblist`, `ANODE.a_xspot`/`a_yspot`)
- `docs/api/structures/iconblk.md` — `ICONBLK` field layout (`ib_xicon`,
  `ib_xtext`, `ib_wtext`, …)

## NewTOS implementation notes

(Empty during Phase 00 / this extraction task. NewTOS's current
`kernel/src/desktop.rs` (`ICON_LABEL_W`/`ICON_GAP_W`/`ICON_STEP_W`
constants, commit `af426a0`) already independently arrived at a
`12*CELL_W`-wide fixed cell with a per-label dynamic centring offset —
consistent in *spirit* with the `wtext + 2*xtext` formula above, though
NewTOS recomputes the centring offset per-label rather than baking a
single resource-authored `ib_xtext` for icon #0. Whether to also adopt
the desktop-vs-window pitch distinction (symmetric redistribution vs.
fixed gap) is a decision for the implementation phase, not this
extraction.
