# AES Window Manager (`wind_*` 100–109) + Graphics (`graf_*` 70–79)

> **PHASE-00b D8, Batch 4.** Source: EmuTOS @ `1a7484ab` —
> `aes/gemwmlib.c` (`wm_*`), `aes/gemwrect.c` (rectangle list),
> `aes/gemgrlib.c` (`gr_*`), `aes/gemgraf.c` (primitives), constants
> `include/aesdefs.h:519-552`, dispatch `aes/gemsuper.c`.
>
> Call convention: [`_dispatcher.md`](./_dispatcher.md).

## Window border components (the `kind` bitmask, `aesdefs.h:519-531`)

`wind_create`'s `kind` ORs the widgets the window border shows:

| Bit | hex | Widget | Bit | hex | Widget |
|-----|----:|--------|-----|----:|--------|
| NAME | 0x001 | title bar | UPARROW | 0x040 | scroll up |
| CLOSER | 0x002 | close box | DNARROW | 0x080 | scroll down |
| FULLER | 0x004 | full box | VSLIDE | 0x100 | vert. slider |
| MOVER | 0x008 | drag bar | LFARROW | 0x200 | scroll left |
| INFO | 0x010 | info line | RTARROW | 0x400 | scroll right |
| SIZER | 0x020 | size box | HSLIDE | 0x800 | horiz. slider |

A clicked widget generates a `WM_*` message (Batch 2) via `evnt_mesag`.

## Window fields (`WF_*`, used by `wind_get`/`wind_set`, `aesdefs.h:534-552`)

| | | | |
|--|--|--|--|
| 1 `WF_KIND` | 2 `WF_NAME` | 3 `WF_INFO` | 4 `WF_WXYWH` work area |
| 5 `WF_CXYWH` current (border) | 6 `WF_PXYWH` previous | 7 `WF_FXYWH` full size | 8 `WF_HSLIDE` |
| 9 `WF_VSLIDE` | 10 `WF_TOP` | 11 `WF_FIRSTXYWH` | 12 `WF_NEXTXYWH` |
| 14 `WF_NEWDESK` | 15 `WF_HSLSIZ` | 16 `WF_VSLSIZ` | 17 `WF_SCREEN` |
| 18 `WF_COLOR` | 19 `WF_DCOLOR` | 20 `WF_OWNER` | |

`WF_FIRSTXYWH`/`WF_NEXTXYWH` iterate the **rectangle list** (`gemwrect.c`):
the visible (unobscured) pieces of a window's work area — what a
`WM_REDRAW` handler must redraw, walked until a returned rect is empty.

---

## Window Manager (`wm_*`)

`WM_*` intin macros; rects are `GRECT{x,y,w,h}`.

| Op | Name | Dispatch | Args / effect |
|---:|------|----------|---------------|
| 100 | `wind_create` | `wm_create(kind, &wx)` | allocate a window of border `kind`, max extent `wx`; returns a **handle** (≥0) or negative error |
| 101 | `wind_open`   | `wm_open(handle, &wx)` | display window at rect `wx`; generates first `WM_REDRAW` |
| 102 | `wind_close`  | `wm_close(handle)` | hide window (handle stays valid) |
| 103 | `wind_delete` | `wm_delete(handle)` | free the handle |
| 104 | `wind_get`    | `wm_get(handle, field, &out, &in)` | read a `WF_*` field → intout 1–4 |
| 105 | `wind_set`    | `wm_set(handle, field, &in)` | set a `WF_*` field (name, slider pos, size, …) |
| 106 | `wind_find`   | `wm_find(mx, my)` | handle of the window under (mx,my); 0 = desktop |
| 107 | `wind_update` | `wm_update(beg)` | begin/end screen lock & mouse ownership. `beg` = END_UPDATE 0 / BEG_UPDATE 1 / END_MCTRL 2 / BEG_MCTRL 3 |
| 108 | `wind_calc`   | `wm_calc(type, kind, ix,iy,iw,ih, &ox,&oy,&ow,&oh)` | convert work↔border rect. `type` = WC_BORDER 0 / WC_WORK 1 |
| 109 | `wind_new`    | `wm_new()` | close+delete **all** windows, reset the manager (cleanup) |

Handle **0** is always the **desktop** (root) window. The AES owns the
screen background (the desktop tree) and clips each window to the parts
not covered by windows above it (the rectangle list).

---

## Graphics Manager (`graf_*`) — interactive feedback

Rubber-band / drag / animation helpers; most own the mouse for the
duration and block until the button is released.

| Op | Name | Dispatch | Effect |
|---:|------|----------|--------|
| 70 | `graf_rubberbox` | `gr_rubbox(x,y,minw,minh, &lastw,&lasth)` | rubber-band a box from a fixed corner; returns final w/h |
| 71 | `graf_dragbox`   | `gr_dragbox(w,h,sx,sy, &bound, &lastx,&lasty)` | drag a fixed-size box within a bounding rect; returns final x/y |
| 72 | `graf_movebox`   | `gr_movebox(w,h, sx,sy, dx,dy)` | animate an outline box from (sx,sy) to (dx,dy) |
| 73 | `graf_growbox`   | `gr_growbox(&small, &big)` | zoom-rect animation small→big (window open) |
| 74 | `graf_shrinkbox` | `gr_shrinkbox(&small, &big)` | zoom-rect big→small (window close) |
| 75 | `graf_watchbox`  | `gr_watchbox(tree, obj, instate, outstate)` | track button in/out of an object, toggling state; returns 1 if released inside |
| 76 | `graf_slidebox`  | `gr_slidebox(tree, parent, obj, isvert)` | slide a child within parent (scrollbar thumb); returns 0–1000 position |
| 77 | `graf_handle`    | returns `gl_handle` + fills `gl_wchar/hchar/wbox/hbox` | **the VDI workstation handle** the AES draws through + system font cell size |
| 78 | `graf_mouse`     | `gr_mouse(number, addr)` | set the mouse cursor shape (`number` = ARROW/HAND/… or `M_OFF/M_ON`; `addr` = `MFORM*` for user shape) |
| 79 | `graf_mkstate`   | `gr_mkstate(&mx,&my,&mstate,&kstate)` | instantaneous mouse x/y + button + shift state (poll, no wait) |

`graf_handle` (77) is the bridge to the VDI: an application calls it once
to learn the AES's VDI workstation handle, then issues VDI calls on it.

---

## NewTOS implementation notes

- **Window clipping** = the rectangle list (`gemwrect.c`): the algorithm
  that splits a window's work area into the visible pieces given the
  windows stacked above it. This is the core invariant of the WM; a
  redraw handler walks `WF_FIRSTXYWH`/`WF_NEXTXYWH` and clips each VDI
  draw to that rect.
- `graf_handle` ties the AES to **VDI Phase 12**: the AES must open its
  own VDI workstation at init and hand its handle out here. `gl_wchar`/
  `gl_hchar` come from the VDI system font metrics.
- The mouse cursor (`graf_mouse`) and rubber-band feedback (`gr_*`) need a
  **mouse sprite / XOR draw** primitive over the framebuffer — pairs with
  the IKBD mouse packets ([[keyboard-via-atari-ikbd]]) feeding position.
- `wind_update` locking presumes preemptible AES processes; single-app
  bring-up can treat it as a no-op-ish screen lock.
- Bring-up order: VDI workstation → `graf_handle` → `wind_create/open` +
  desktop (handle 0) → rectangle list → `WM_REDRAW` loop.
