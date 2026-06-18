# AES Object Manager (`objc_*` 40–48) + Form Manager (`form_*` 50–56)

> **PHASE-00b D8, Batch 3.** Source: EmuTOS @ `1a7484ab` —
> `aes/gemoblib.c` (`ob_*`), `aes/gemobed.c` (`ob_edit`),
> `aes/gemfmlib.c` (`fm_*`), `aes/gemfmalt.c` (alerts), structs in
> `include/obdefs.h`, dispatch `aes/gemsuper.c:170-230`.
>
> Call convention: [`_dispatcher.md`](./_dispatcher.md). This is the
> **heart of GEM UI**: everything visible (dialogs, buttons, menus,
> the desktop) is an **object tree**.

## The `OBJECT` tree (`obdefs.h:141-154`)

A GEM UI is a flat array of `OBJECT` nodes; tree structure is by index
(`-1` = `NIL` = none). `objc_*` / `form_*` take the array base + an index.

```c
typedef struct {
/* +0x00 */ WORD  ob_next;    /* next sibling index (-1 = none)     */
/* +0x02 */ WORD  ob_head;    /* first child index                  */
/* +0x04 */ WORD  ob_tail;    /* last child index                   */
/* +0x06 */ UWORD ob_type;    /* G_* graphic type                   */
/* +0x08 */ UWORD ob_flags;   /* SELECTABLE/DEFAULT/EXIT/...         */
/* +0x0A */ UWORD ob_state;   /* SELECTED/CHECKED/DISABLED/...       */
/* +0x0C */ LONG  ob_spec;    /* type-specific payload (see below)  */
/* +0x10 */ WORD  ob_x, ob_y; /* position, relative to parent       */
/* +0x14 */ WORD  ob_width, ob_height;
} OBJECT;   /* 24 bytes */
```

`ob_spec` payload depends on `ob_type`: a colour-word for `G_BOX`/`G_IBOX`;
a `char*` for `G_STRING`/`G_TITLE`/`G_BUTTON`; a `TEDINFO*` for
`G_TEXT`/`G_BOXTEXT`/`G_FTEXT`/`G_FBOXTEXT`; an `ICONBLK*` for `G_ICON`;
a `CICONBLK*` for `G_CICON`; a `BITBLK*` for `G_IMAGE`; a `USERBLK*`
(`ub_code`/`ub_parm`) for `G_USERDEF`.

### Constants (`obdefs.h:67-107`)

| `ob_type` (G_) | | `ob_flags` | hex | `ob_state` | hex |
|---|---|---|---|---|---|
| BOX 20, TEXT 21, BOXTEXT 22 | | NONE | 0x000 | NORMAL | 0x00 |
| IMAGE 23, USERDEF 24, IBOX 25 | | SELECTABLE | 0x001 | SELECTED | 0x01 |
| BUTTON 26, STRING 28 | | DEFAULT | 0x002 | CROSSED | 0x02 |
| FTEXT 29, FBOXTEXT 30 | | EXIT | 0x004 | CHECKED | 0x04 |
| ICON 31, TITLE 32, CICON 33 | | EDITABLE | 0x008 | DISABLED | 0x08 |
| | | RBUTTON (radio) | 0x010 | OUTLINED | 0x10 |
| | | LASTOB (tree end) | 0x020 | SHADOWED | 0x20 |
| | | TOUCHEXIT | 0x040 | WHITEBAK | 0x40 |
| | | HIDETREE | 0x080 | | |
| | | INDIRECT | 0x100 | | |

`TEDINFO` (editable text, `obdefs.h:170-183`): `te_ptext`/`te_ptmplt`/
`te_pvalid` (text/template/validation strings), `te_font`, `te_just`,
`te_color`, `te_thickness`, `te_txtlen`/`te_tmplen`.
`ICONBLK` (`:185-201`): mask/data bitmaps + char + icon/text rects.
`BITBLK` (`:219-227`): raw raster `bi_pdata`/`bi_wb`/`bi_hl`/`bi_color`.

---

## Object Manager (`objc_*`)

`intin` slots named per the dispatch macros (`OB_*`); `addrin[0]` = tree.

| Op | Name | Dispatch (`gemsuper.c`) | Args / effect |
|---:|------|------------------------|---------------|
| 40 | `objc_add`    | `ob_add(tree, parent, child)` | link `child` as last child of `parent` |
| 41 | `objc_delete` | `ob_delete(tree, delob)` | unlink object from tree |
| 42 | `objc_draw`   | `gsx_sclip(clip); ob_draw(tree, drawob, depth)` | draw subtree from `drawob` down `depth` levels, clipped to rect `OB_XCLIP{x,y,w,h}` (intin 3–6) |
| 43 | `objc_find`   | `ob_find(tree, startob, depth, mx, my)` | hit-test: index of deepest object under (mx,my), or -1 |
| 44 | `objc_offset` | `ob_offset(tree, obj, &xoff, &yoff)` | object's **absolute** screen x/y (sums parents) → intout 1,2 |
| 45 | `objc_order`  | `ob_order(tree, obj, newpos)` | move object to sibling position `newpos` (0=front, -1=back) |
| 46 | `objc_edit`   | `ob_edit(tree, obj, char, &idx, kind)` | edit an `EDITABLE` `TEDINFO` field; `kind` = EDSTART/EDINIT/EDCHAR/EDEND (0–3); returns new cursor `idx` |
| 47 | `objc_change` | `gsx_sclip(clip); ob_change(tree, drawob, newstate, redraw)` | set `ob_state = newstate`, redraw if `redraw`≠0, clipped |
| 48 | `objc_sysvar` | `ob_sysvar(mode, which, i1, i2, &o1, &o2)` | get/set 3-D look (gated `CONF_WITH_3D_OBJECTS`) |

`ob_draw` is recursive over the tree, drawing each node by `ob_type`;
`ob_find` walks the same tree top-down honoring `HIDETREE`/`DISABLED`.

---

## Form Manager (`form_*`)

Higher level: drives a complete dialog interaction on top of `objc_*`.

| Op | Name | Dispatch | Args / effect |
|---:|------|----------|---------------|
| 50 | `form_do`   | `fm_do(form, start)` | **run a modal dialog**: own the event loop, handle keyboard/mouse, edit fields, track buttons; returns the index of the `EXIT`/`TOUCHEXIT` object that ended it |
| 51 | `form_dial` | `fm_dial(type, &ix, &x)` | reserve/free screen + draw grow/shrink box. `type` = FMD_START 0 / FMD_GROW 1 / FMD_SHRINK 2 / FMD_FINISH 3; `ix`/`x` = small/large rects |
| 52 | `form_alert` | `fm_alert(defbut, astring)` | **alert box** `"[icon][msg][buttons]"`; `defbut` = default button #; returns button pressed (1-based). `astring` = `char*` (addrin) |
| 53 | `form_error` | `fm_error(errnum)` | canned DOS-error alert for GEMDOS error `errnum`; returns button |
| 54 | `form_center` | `ob_center(form, &x)` | centre tree on screen; fills result rect (intout 1–4) |
| 55 | `form_keybd` | `fm_keybd(form, obj, &ochar, &onxtob)` | dialog key handling: map a key to a field/navigation; returns continue-flag |
| 56 | `form_button` | `fm_button(form, obj, clks, &onxtob)` | dialog click handling: process a click on `obj` |

`form_do` is the single call an application makes to run a dialog; it
internally loops `evnt_multi` + `objc_edit`/`objc_change` until an `EXIT`
button is hit. `form_alert`/`form_error` build a tiny tree internally.

---

## NewTOS implementation notes

- **EC1 milestone** depends on `objc_draw` + `form_alert`/`form_do`: the
  first visible GEM UI. These need the **VDI** (Phase 12) for all drawing
  (text via `v_gtext`, boxes via `v_bar`/`vr_recfl`, icons via raster
  blits) and clipping (`gsx_sclip` → VDI `vs_clip`).
- The object tree is a **user-supplied array** (addrin pointer) — apply
  the `_dispatcher.md` §4 user/kernel boundary handling. The tree is
  *mutated in place* (`ob_state`, `ob_edit` cursor), so writes go back to
  user memory.
- `ob_draw` recursion depth is bounded by the tree; no dynamic allocation
  needed — fits `#![no_std]` without `alloc`.
- `form_do` owns an **event loop** → depends on Event Manager (Batch 2)
  being functional first. Natural order: VDI → objc_draw → form_alert →
  form_do.
- `objc_edit` text editing reuses the same key decoding as the IKBD path
  ([[keyboard-via-atari-ikbd]]).
