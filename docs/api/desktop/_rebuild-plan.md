# Desktop rebuild — implementation plan (`kernel/src/desk/`)

> Team-lead synthesis (Task #4) from the extracted specs (`_overview.md`,
> `_data-model.md`, `deskapp.md`, `deskwin.md`, `desksupp-redraw.md`,
> `deskmain.md`, `_current-defects.md`) + the dir/file/INF extraction (Task #3,
> in progress). Governs the Developer tasks (#5–#9) and the Auditor gate (#10).
> Charter: **ADR-016** — EmuTOS-faithful reimplementation of the desktop
> *application* on top of the already-faithful AES/VDI/GEMDOS layer in `tos-api`.

## 1. Principle

Reimplement EmuTOS `desk/` **behaviour** (window/file model, grid metrics, view
building, damage-rect redraw, event loop, menu, file ops, INF) cleanly in
`#![no_std]` Rust. Do **not** port the C line-by-line. The measuring stick is
the **user-observable** desktop behaviour, not byte-identical internals
(`WNODE`/`FNODE`/`G` are all *internal* per `_data-model.md` — free to redesign).
What must match: the `OBJECT`/`ICONBLK` trees handed to `objc_draw`, and the AES
call semantics — both already contract-documented.

## 2. Target module layout (`kernel/src/desk/`)

| Module | EmuTOS origin | Spec | Content |
|---|---|---|---|
| `mod.rs` | `deskstart.S` + `deskmain()` head | `deskmain.md` | `desk::run()` boot entry; one-time setup (icons, metrics, window/obj init, menu install, `app_blddesk`); hands off to the event loop |
| `global.rs` | `deskglob.c` + `GLOBES` (`deskbind.h`) | `_data-model.md` | `DeskState` (≙ `G`): view/sort, grid metrics (`g_iwext/ihext/iwint/ihint/iwspc/ihspc`, `g_wicon/hicon`, `g_icw/ich`), `g_desk` rect, window list head, the `g_screen[]`/`g_screeninfo[]` object arena + free chain |
| `model.rs` | `deskfpd.c` (`FNODE`/`PNODE`) + `SCREENINFO` | `_data-model.md` | `FNode`/`PNode`/`ScreenInfo`; directory scan (`pn_open`/`pn_active`/`pn_sort`) via GEMDOS `Fsfirst`/`Fsnext`. **No fixed 64-cap** (fix D9) — dynamic list |
| `app.rs` | `deskapp.c` (rendering parts) | `deskapp.md` | `app_start` metrics (`g_wicon`/`g_hicon`/`g_icw`/`g_ich`), `app_blddesk` (desktop pseudo-window tree), `set_background` |
| `win.rs` | `deskwin.c` | `deskwin.md` | `WNode` + window-list mgmt (`win_start`/`win_alloc`/`win_free`/`win_find`/`win_top`), **`win_view`/`win_ocalc`/`win_bldview`**, scrolling (`win_slide`/`win_arrow`/`win_blt`), bulk (`win_srtall`/`win_bdall`/`win_shwall`) |
| `redraw.rs` | `desksupp.c` `do_wredraw` | `desksupp-redraw.md` | `do_wredraw(handle, clip)` over the AES `WF_FIRSTXYWH`/`WF_NEXTXYWH` rect list |
| `main.rs` | `deskmain.c` event loop | `deskmain.md` | `evnt_multi` loop + message drain subloop; `hndl_button` (via `wind_find` — fix D3), `hndl_kbd` (fix D4), `hndl_menu` dispatch; `desk_all` view/sort pipeline |
| `menu.rs` | `deskmain.c` menu parts | `deskmain.md` | menu-tree build, `adjust_menu` (measure/shrink), **`men_update`** (enable/disable/check — fix D6) |
| `dir.rs` | `deskdir.c` + `deskfun.c` | *(Task #3)* | file ops: copy/move/delete/rename/new-folder dispatch + recursive helpers |
| `act.rs` | `deskact.c` | *(Task #3)* | selection / drag / rubber-band state machine; single geometry source (fix D7) |
| `ins.rs` | `deskins.c` | *(Task #3)* | install icon / install application (ANODE) |
| `inf.rs` | `deskinf.c` | *(Task #3)* | `NEWDESK.INF` parse/save (`#W`/`#M`/`#T`/`#O`/`#E`/`#Q`/…) |
| `rez.rs` | `deskrez.c` | *(Task #3)* | resolution change → recompute metrics |
| `icons.rs` | `icon.rsc` (built-in bitmaps) | — | NewTOS icon bitmaps (port/replace old `desktop_icons.rs`); NewTOS owns its `ICONBLK` params incl. `ib_xtext` (§5) |

## 3. Data model (NewTOS)

Per `_data-model.md`, all of `G`/`WNODE`/`PNODE`/`FNODE`/`SCREENINFO` are
desktop-internal (never cross an ABI boundary) → redesign cleanly:
- Prefer safe Rust ownership over EmuTOS's `malloc`'d free-chains where it costs
  nothing observable. `g_screen[]` stays an `OBJECT` array (it **is** the AES
  tree handed to `objc_draw`) but its indexing (`DROOT`/`WOBS_START`/free chain)
  is our choice.
- Drop the `FNODE`/`DTA` 23-byte `memcpy` overlap trick (`deskfpd.c:271`) — copy
  DTA fields explicitly; our `f_name` need not be 13 bytes.
- Scroll position is a **grid-cell** offset (`w_cvcol`/`w_cvrow`), converted to
  pixels only at draw time via `g_iwspc`/`g_ihspc` — **one** coherent unit for
  both views (fixes D5, the icon/text scroll-unit mismatch).

## 4. AES/VDI/GEMDOS call surface (the NewTOS "aesbind")

The desktop is a client of `tos-api`; it must go **through** these calls (like
EmuTOS `desk/` over `aesbind`), not reach into kernel internals (ADR-016
review charge). Confirmed available in `tos-api` (per `_current-defects.md` D2
and the AES specs):
- **Window mgr:** `wind_create/open/close/set/get/find/calc`; crucially
  `wind_get(WF_FIRSTXYWH/WF_NEXTXYWH)` — the occlusion-aware rect list already
  implemented in `tos-api/src/aes/window.rs:264-310` (unused today).
- **Objects:** `objc_draw`/`objc_find`/`objc_add`/`objc_offset`.
- **Menu:** `menu_bar`/`menu_ienable`/`menu_icheck`/`menu_tnormal`.
- **Events:** `evnt_multi` (`MU_BUTTON|MU_MESAG|MU_KEYBD|MU_TIMER`).
- **Graphics:** `graf_mouse` (M_OFF/M_ON, HGLASS — enables D11 busy cursor).
- **GEMDOS:** `Fsfirst`/`Fsnext` (`tos_api::bdos::fs::find`), file ops for `dir.rs`.

Developer: confirm exact Rust names/signatures against `tos-api/src/aes/*` before
each task; if a needed AES call is missing or wrong → **STOP**, report to lead
(do not reach around it into the kernel).

## 5. Grid-metric contract (root of the overlap bug — D1)

From `deskapp.md` (`app_start`, `deskapp.c:810-817`), the exact formulas NewTOS
must reproduce:

```
icon cell width   g_wicon = MAX_ICONTEXT_WIDTH(12) * char_w + 2*ib_xtext
icon cell height  g_hicon = ib_hicon(icon#0) + char_h + 2
window pitch      g_iwspc = g_wicon + MIN_WINT      (MIN_WINT = 4px if rows<=300 else 8px)
                  g_ihspc = g_hicon + MIN_HINT      (MIN_HINT = 2px)
desktop pitch     xcnt = g_desk.w / (g_wicon+MIN_WINT);  g_icw = g_desk.w / xcnt   (remainder spread evenly)
                  ycnt = g_desk.h / (g_hicon+MIN_HINT);  g_ich = g_desk.h / ycnt
```

Key correctness points (fixing D1):
- The label box is a **fixed** `MAX_ICONTEXT_WIDTH`-char width — **never**
  shrink-to-fit. The cell = label width **+ `2*ib_xtext`** (icon-0's centering
  offset), not label width alone.
- `char_w`/`char_h`: NewTOS uses its **fixed 8×16 font** (Phase-12 decision), so
  `char_w = 8` is our metric; that is legitimate as long as it is used
  consistently everywhere (renderer *and* metric) — Developer must confirm the
  VDI glyph renderer's real per-glyph width equals the metric before wiring D1.
- **`ib_xtext` is a NewTOS design choice** (EmuTOS's value lives in a build-time
  `icon.rsc` not in the checkout — R1 STOP-flag; not a blocker). Pick a small
  fixed centering offset for the NewTOS icon `ICONBLK` in `icons.rs` and use the
  **formula** everywhere. The formula is the contract; the value is ours.
- `draw_text` must clip glyphs to the object text box (the `tos-api` AES fix from
  the superseded mossy plan Phase 0) — verify it is in place / apply once.

## 6. Redraw contract (D2/D8)

From `desksupp-redraw.md` (`do_wredraw`, `desksupp.c:202-232`):
```
root = (handle==DESK) ? DROOT : win_find(handle).w_root   // else silent return
graf_mouse(M_OFF)
t = wind_get(handle, WF_FIRSTXYWH)
while t.w && t.h {                        // sentinel: w==0||h==0 ends
    if rc_intersect(clip, t) { objc_draw(g_screen, root, MAX_DEPTH, t) }
    t = wind_get(handle, WF_NEXTXYWH)     // always advance, even after a hit
}
graf_mouse(M_ON)
```
Every repaint (WM_REDRAW with **its own `msg[4..8]` rect** — fix D8; scroll blt;
selection change; view/sort rebuild) routes through this. Draw order becomes
irrelevant to correctness. `win_shwall` sends one WM_REDRAW **per window** with
that window's rect.

## 7. Task sequencing

| Task | Deliverable | Depends | Verify |
|---|---|---|---|
| **#5 Scaffold** | `mod.rs`/`global.rs`/`model.rs`/`win.rs` skeleton + `DeskState` + `FNode`/`WNode`/`PNode` + directory scan; wire `desk::run()` at the boot call site (replace `desktop::run()`); deprecate `desktop.rs`/`desktop_icons.rs` (guarded, kept until parity) | #4 | build+clippy both boards; **still boots** to a (possibly empty) desktop screen |
| **#6 View builder** | `app_start` metrics + `win_view`/`win_ocalc`/`win_bldview`; `find_entry_at` reuses the builder | #5 | QMP screendump on qemu raspi4b, **representative 12-char 8.3 SD image**: 4 clean icon columns, no overlap |
| **#7 Redraw** | `do_wredraw` over the rect list; route open/close/move/size/top/view/sort | #6 | screendump: overlap-on-top + view-switch residue gone (Auditor drives the residue repro) |
| **#8 Event/menu** | `evnt_multi` loop + `hndl_button`(`wind_find`) + `hndl_kbd` + `hndl_menu`; `men_update`; `adjust_menu` | #5 | keyboard + click-any-window work; menu items gray correctly |
| **#9 Dir/file/INF/rez** | `dir.rs`/`act.rs`/`ins.rs`/`inf.rs`/`rez.rs` | #5, #3 | file ops + INF round-trip; resolution change |
| **#10 Audit (gate)** | per-deliverable adversarial verification | each of #5–#9 | representative data; CONFIRMED/PLAUSIBLE findings; nothing "done" without sign-off |

## 8. Open scope decision — `CONF_WITH_*` feature gates

`deskmain.md` catalogued ~12 gates with no recorded NewTOS decision. They affect
which menu items/behaviours `menu.rs`/`dir.rs` include (they do **not** affect
#5–#7 core). **Recommended default:** the EmuTOS general-Atari variant (all
`CONF_WITH_*` **on**), *except* hardware-absent/legacy ones:
- **In:** SelectAll, FileMask (Set mask), Search, SizeToFit (full), Read/Save
  NEWDESK.INF preferences, PrinterIcon (we already draw one), DesktopShortcuts,
  BottomToTop (window cycling), PCGEM defaults.
- **Out:** Format (no floppy/removable-format on Pi4 SD — consistent with the
  FAT12/floppy out-of-scope decision), AltDesktopGraphics (cosmetic), EasterEgg.

→ **DECIDED 2026-07-04 (user):** the **full EmuTOS-default set** above — In as
listed (SelectAll, Set-mask/FileMask, Search, Size-to-fit, Read+Save NEWDESK.INF
preferences, Printer icon, Desktop shortcuts, BottomToTop, PCGEM); **Out:**
Format, Alt-Desktop-Graphics, Easter-Egg. `menu.rs`/`dir.rs` build against this
set.

## 9. Carried-forward risks / STOP-flags

- **View-switch residue** (R2 STOP-flag): unconfirmed mechanism; best hypothesis
  = instance of D2, possibly compounded by D5. The rebuild fixes both
  structurally; **Auditor must drive the specific repro** (single window, two
  overlapping, 3-window overlap, plus a menu/dialog overlap) to confirm.
- **`format_fnode`/`dr_code`** (text-view line format) — **resolved** by Task #3
  in `deskinf.md`. **`snap_icon`** — **resolved** by Task #19 in `deskins.md`
  (section "`snap_icon`/`ins_posdisk`/`grid_free` — desktop grid placement",
  cross-linked from `deskapp.md` and `_data-model.md`). Correction to the
  framing above: `snap_icon` itself is **not** collision avoidance — it only
  rounds a pixel position to the nearest desktop grid cell and clamps it to
  the desktop bounds. Collision avoidance against already-placed icons is a
  *separate* pair of helpers, `grid_free`/`ins_posdisk`, used only by the
  three new-icon-install call sites (`install_drive`, `install_desktop_icon`,
  and `ins_shortcut`'s 2nd+ icon in a multi-drop) — `ins_shortcut`'s *first*
  icon, `app_start()`'s INF-position-apply loop, and `move_drvicon()`'s
  drag-drop path all call `snap_icon` alone, with **no** collision check, so
  EmuTOS itself can (and does, given colliding input) silently overlap desktop
  icons. `ins.rs` (this plan's Task #9 "Dir/file/INF/rez"; tracker #20
  "Install-Icon-Platzierung") must decide explicitly whether NewTOS
  reproduces this gap or adds a stronger free-cell guarantee everywhere (a
  deliberate improvement, not contract — see `docs/COMPATIBILITY.md`).
  NEWDESK.INF
  grammar lives in `deskinf.md`'s appendix (sourced from `deskapp.c`), **not**
  `deskinf.c` — `inf.rs` builds against that.
- **AES gaps:** if #6/#7 hit a missing/incorrect `wind_*`/`objc_*` in `tos-api`,
  that is a `tos-api` fix (separate Developer task), **not** a desk-side
  workaround.

## 10. Verification protocol

Every Developer deliverable: `cargo build` + `cargo clippy -D warnings` clean for
**both** boards, system still boots, and — for anything visual — a QMP
screendump on **`qemu raspi4b` with a realistic 8.3-long-name SD image** (the
blind spot that hid the original bug). Then the **Auditor** gate (#10) with
adversarial/representative data. Nothing is "done" before Auditor sign-off.
