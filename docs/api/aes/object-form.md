# AES Object Manager (`objc_*` 40–48) + Form Manager (`form_*` 50–56)

> **PHASE-00b (AES Object/Form, full-contract refinement).**
> Source: EmuTOS @ `1a7484ab`. Re-validated 2026-07-10 (AES leg, Batch 3) —
> found and corrected D-089…D-093 (see `_discrepancies.md`), most notably
> an inverted `th`/border-thickness sign convention (D-089).
> Primary handlers: `aes/gemoblib.c` (`ob_*`, `just_draw`, `gr_crack` via
> `aes/gemgraf.c`), `aes/gemobed.c` (`ob_edit`, `ob_center`),
> `aes/gemfmlib.c` (`fm_do`/`fm_dial`/`fm_keybd`/`fm_button`/`fm_error`),
> `aes/gemfmalt.c` (`fm_alert`, alert tree build). Tree walk helpers:
> `aes/gemobjop.c` (`ob_sst`/`everyobj`/`get_par`). Dispatch arms:
> `aes/gemsuper.c:203-263`. Structs + constants: `include/obdefs.h`.
> Opcode + slot macros: `include/aesdefs.h:48-65,305-384`.
>
> Call convention + AESPB layout: [`_dispatcher.md`](./_dispatcher.md).
> Shared AES globals (`rlr`, the message pipe, the mouse owner):
> [`_state.md`](./_state.md). Structs:
> [`../structures/object.md`](../structures/object.md),
> [`tedinfo`](../structures/tedinfo.md),
> [`iconblk`](../structures/iconblk.md),
> [`bitblk`](../structures/bitblk.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** the opcode numbers (40–48, 50–56); each call's
> `int_in`/`addr_in`/`int_out` slot layout; the `OBJECT`/`TEDINFO`/`ICONBLK`/
> `BITBLK` byte layouts; the `G_*` type numbers, `ob_flags`/`ob_state` bit
> values and the colour-word bit split; the `objc_find` hit-test rule
> (deepest visible object, `HIDETREE`-skipping, or `-1`); `objc_offset`'s
> absolute-coordinate result; the `form_do` modal contract (which objects
> exit, `DEFAULT`/Enter, `TOUCHEXIT` vs `EXIT`, radio siblings); the
> `form_alert` `[icon][msg][buttons]` string grammar and 1-based button
> return; the `form_dial` `FMD_*` phases; the `objc_edit` `ED*` kinds and
> `te_pvalid` validation codes. **Reference (NewTOS reimplements the *how*
> freely — NewTOS draws with its own `RasterBackend`):** the exact VDI call
> sequence in `just_draw`, the `everyobj` non-recursive traversal, the
> `D.g_rawstr`/`g_tmpstr`/`g_fmtstr` scratch buffers, `gr_watchbox`/
> `ev_button` interaction internals, and the shadow geometry (drawn by
> NewTOS's own rasteriser). The 3D bevel is **not** free any more: since A67
> (2026-09-18) `add_3d_effect` follows `gemoblib.c:220-283` pixel for pixel
> — one-pixel lines on the grown rectangle, shrunk by `th` for an inside
> border, raised until SELECTED, the far corners left in the face colour —
> and the border is drawn whenever `th != 0` regardless of 3D
> (`gemoblib.c:663-667`), with the bevel laid over it (`:950-951`).

---

## 0. The object tree and the drawing primitives (shared reference)

A GEM UI is a **flat `OBJECT[]` array** indexed by sibling/child links
(`ROOT = 0`, `NIL = -1`). See [`../structures/object.md`](../structures/object.md)
for the byte layout, the `G_*` types, the `ob_flags`/`ob_state` bit values and
the `ob_spec` colour-word decode. Two shared internals recur below:

- **`ob_sst`** (`gemobjop.c:25-73`) reads an object's spec/state/type/flags,
  computes its **border thickness** `th`, applies the `INDIRECT` dereference,
  and masks `ob_type` to its low byte. Thickness rules: `G_TITLE` → 1;
  TED types → `te_thickness`; box types → byte 1 of `ob_spec`; `G_BUTTON` →
  `-1`, minus 1 more if `EXIT`, minus 1 more if `DEFAULT` (so a default exit
  button has a thicker outline). Values >128 are treated as negative
  (`th -= 256`) — **`th > 0` = inside border, `th < 0` = outside border**
  (see D-089 below — this is the opposite of an earlier version of this
  spec).
- **`gr_crack`** (`gemgraf.c:612-628`) decodes the colour word — the single
  most bug-prone table for the desktop renderer:

  | Colour-word bits | Output | Note |
  |------------------|--------|------|
  | 15–12 | border colour | `(c>>12)&0xf` |
  | 11–8  | text colour   | `(c>>8)&0xf` |
  | 7     | writing mode  | set → `MD_REPLACE`; clear → `MD_TRANS` |
  | 6–4   | inside pattern (3 bits) | `IP_HOLLOW`=0 … `IP_SOLID`=7 |
  | 3–0   | inside colour | `c&0xf` |

- **`everyobj`** (`gemobjop.c:76-137`) is the non-recursive tree walker
  `ob_draw` uses: it visits an object, descends into children while
  `depth <= maxdep` and the node is not `HIDETREE`, then walks siblings back
  up to the parent. `MAX_DEPTH = 8` (`obdefs.h:30`).

> **Correction (D-089): the `th` sign convention was inverted in an earlier
> version of this spec.** Traced through three independent call sites in
> `gemoblib.c`/`gemgraf.c`:
> 1. **`gr_box`** (`gemgraf.c:813-832`): for `th > 0` it iterates
>    `gr_inside(&n, th)` with a **positive** offset each pass — this
>    *shrinks* the nested outline rectangles, so all border lines land
>    **on or inside** the base rect. For `th < 0`, `gr_inside` receives a
>    **negative** offset — this *grows* the rect each pass, so the border
>    lines land **on or outside** the base rect.
> 2. **`just_draw`**'s fill inset (`gemoblib.c:553,663-671`):
>    `tmpth = (th < 0) ? 0 : th;` then `gr_inside(&t, tmpth)` before filling
>    the box interior. For `th < 0` (outside border) the fill is **not**
>    inset — the interior fill occupies the *entire* nominal rect, because
>    the border sits outside it and doesn't eat into the content area. For
>    `th > 0` (inside border) the fill *is* inset by `th`, leaving room for
>    the border inside the rect.
> 3. **`just_draw`**'s state-decoration inset (`gemoblib.c:889-892`):
>    `if (th > 0) gr_inside(&t, th); else th = -th;` — again, only a
>    positive (inside) border shrinks the content rect; a negative (outside)
>    border leaves it alone and just takes `|th|` for the shadow-size calc
>    that follows.
> 4. **`ob_actxywh`**'s 3D shadow-hit-test adjustment (`gemoblib.c:329-334`)
>    only grows the hit-test rect for `SHADOWED` **when `border < 0`** — a
>    drop shadow, which visually extends *outward* past the object, only
>    needs a bigger hit box when the border itself is an outward
>    (negative-`th`) one. This is consistent with (1)–(3), not with the
>    inverted reading.
>
> So the correct rule is **`th > 0` ⇒ border drawn inside the object's
> nominal rect (content area shrinks by `th`); `th < 0` ⇒ border drawn
> outside the nominal rect (content area is the full rect, border is an
> add-on)**. `G_BUTTON`'s `th` is always negative (`-1`, or more negative
> with `EXIT`/`DEFAULT`, `gemobjop.c:59-65`), i.e. **every button's border
> is drawn outside its nominal rect**, growing the button's apparent
> footprint — consistent with a `DEFAULT` exit button looking visually
> "bigger"/more emphasized. A renderer built on the inverted rule would draw
> every box/button border on the wrong side of the object's edge and get
> `SHADOWED` hit-test sizing backwards. See `_discrepancies.md` D-089.

---

## Object Manager (`objc_*`) — opcodes 40–48

Slot macros from `include/aesdefs.h:348-384`. `OB_TREE = addr_in[0]` for
every `objc_*` **except `objc_sysvar`**.

### `objc_add` (40)

**Source:** `gemsuper.c:204-205` → `ob_add` (`gemoblib.c:1121-1141`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_PARENT` | parent object index |
| `int_in[1]` `OB_CHILD` | child object index to link |
| `int_out[0]` | `1` — the arm leaves `ret` at its `TRUE` default (`ob_add` is `void`; `gemsuper.c:85`) |

Links `child` as the **last child** of `parent`: sets `child.ob_next = parent`
(closing the sibling chain), updates the parent's `ob_head` (if first child)
and `ob_tail`. No-op if `parent == NIL` or `child == NIL`. Does **not**
initialise the child's type/flags/state/spec/geometry — the caller must have
done that.

### `objc_delete` (41)

**Source:** `gemsuper.c:207-208` → `ob_delete` (`gemoblib.c:1147-1193`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_DELOB` | object to unlink |
| `int_out[0]` | 1 on success, **0** if `obj == ROOT` or not found |

Unlinks `obj` by fixing its parent's `ob_head`/`ob_tail` and the previous
sibling's `ob_next`. **Cannot delete `ROOT`** → returns 0
(`gemoblib.c:1154-1155`). If the object is not found in its parent's chain,
returns 0 with a `KINFO` note (`gemoblib.c:1180-1183`). It does not clear the
deleted node's own fields, only removes it from the link structure.

### `objc_draw` (42)

**Source:** `gemsuper.c:210-213` → `gsx_sclip(&OB_XCLIP); ob_draw(...)`.
`ob_draw` (`gemoblib.c:959-977`) → `everyobj(..., just_draw, ...)`.

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_DRAWOB` | object to start drawing from |
| `int_in[1]` `OB_DEPTH` | max levels of children to draw |
| `int_in[2..5]` `OB_XCLIP/YCLIP/WCLIP/HCLIP` | clip rectangle (x,y,w,h) |
| `int_out[0]` | `1` (default `ret`; `ob_draw` is `void`) |

The dispatcher first sets the VDI clip rect from `int_in[2..5]`
(`gsx_sclip`), then `ob_draw` walks the subtree from `OB_DRAWOB` down
`OB_DEPTH` levels, drawing each visible node with `just_draw`. The mouse is
hidden (`gsx_moff`) for the duration.

**`just_draw` per-type rendering** (`gemoblib.c:471-953`) — the behaviour a
NewTOS renderer must reproduce (drawn with the native `RasterBackend`, not
the EmuTOS VDI sequence):

1. **Trivial reject / skip.** Return immediately if `HIDETREE` set or
   `ob_spec == -1` (`gemoblib.c:491-492`). Clip-reject the object including
   its outline/shadow extent (`gemoblib.c:532-542`).
2. **Box family** (`G_BOX`/`G_IBOX`/`G_BOXCHAR`, and box-borders for
   `G_BUTTON`/`G_BOXTEXT`/`G_FBOXTEXT`): crack the colour word, draw the
   border with `th` thickness, then fill the inside with the cracked
   pattern/colour. `G_IBOX` draws **border only** (no fill,
   `gemoblib.c:669`). `G_BUTTON` forces `bcol = BLACK` and a hollow-white
   interior (non-3D) (`gemoblib.c:639-658`).
3. **Text family** (`G_TEXT`/`G_BOXTEXT`/`G_FTEXT`/`G_FBOXTEXT`): copy the
   `TEDINFO`, crack `te_color`. For the **formatted** types (`G_FTEXT`/
   `G_FBOXTEXT`) merge `te_ptext` into `te_ptmplt` via `ob_format`
   (`_` slots filled, literals kept; `gemoblib.c:716-757` calls
   `ob_format`, defined at `gemoblib.c:361-408` — see the D-091 correction
   below on what "justification" actually means here). `G_BOXCHAR` centres
   the single spec char in the IBM font.
> **Correction (D-091): `ob_format` implements only *two* fill directions,
> not three "`TE_LEFT`/`RIGHT`/`CNTR`" behaviours.** Reading `ob_format`
> in full (`gemoblib.c:361-408`): it has exactly one conditional,
> `if (just == TE_RIGHT)`. `TE_RIGHT` anchors the raw text to the *end* of
> the template and fills underscore slots **backward** from there; every
> other `just` value — including `TE_LEFT` **and `TE_CNTR`** — takes the
> same, unconditional forward branch and fills underscore slots from the
> *start* of the template. I.e. for the purposes of merging `te_ptext`
> into `te_ptmplt`, `TE_CNTR` behaves **identically to `TE_LEFT`**; there
> is no distinct "centred slot-fill" in `ob_format`. Whole-string `TE_CNTR`
> positioning (centring the *rendered* text within the object's box) is a
> separate, later step done by `gr_just`/`gr_gtext` (`gemgraf.c:567-605`),
> which *does* implement all three `TE_LEFT`/`TE_CNTR`/`TE_RIGHT` values —
> `gr_gtext` is called right after `ob_format` in the same `just_draw` type
> switch (`gemoblib.c:757`) and is what the earlier "TE_LEFT/RIGHT/CNTR
> justification" phrase was actually describing, misattributed to
> `ob_format`. See `_discrepancies.md` D-091.

4. **`G_STRING`/`G_TITLE`/`G_BUTTON`**: draw the plain text centred (button)
   or left-aligned, vertically centred (`gemoblib.c:801-862`).
5. **`G_IMAGE`**: blit the `BITBLK` transparently in `bi_color`
   ([`bitblk`](../structures/bitblk.md)).
6. **`G_ICON`/`G_CICON`**: offset the `ICONBLK` rects by the object's screen
   x/y and draw via `gr_gicon`; the icon owns its SELECTED look, so
   `just_draw` clears SELECTED afterwards ([`iconblk`](../structures/iconblk.md)).

> **`G_CICON` draw-arm detail (TASK/A41, 2026-09-19 — NewTOS: mono-only
> today).** `just_draw`'s `G_ICON`/`G_CICON` case (`gemoblib.c:776-791`)
> shares one code path: `cicon` starts `NULL`; only for `G_CICON`
> (`CONF_WITH_COLOUR_ICONS`, default 1) is it set to `((CICONBLK
> *)spec)->mainlist` — the `CICON` the **loader** already picked via
> `best_match` at `rsrc_load` time, not re-chosen per draw (see
> [`resource-fileselector.md`](./resource-fileselector.md) §5.6/§5.7 for the
> full load-time algorithm). The `ICONBLK` head (`*(ICONBLK *)spec`, valid
> for both types because a `CICONBLK` *begins* with one, `obdefs.h:213-217`)
> is copied and its `ib_xicon`/`ib_yicon`/`ib_xtext`/`ib_ytext` offset by the
> object's screen x/y — identically for `G_ICON` and `G_CICON` — before
> `gr_gicon(state, &ib, cicon)` (`gemoblib.c:784-789`).
>
> `gr_gicon` (`aes/gemgraf.c:710-807`):
> 1. Crack `ib_char` into `fgcol`/`bgcol`/`ch`, as for a mono icon.
> 2. **Data/mask selection** (`:726-753`): if `cicon != NULL`, use
>    `cicon->sel_data`/`sel_mask` when `state & SELECTED` **and** `sel_data`
>    is non-`NULL`, else `cicon->col_data`/`col_mask`; if `cicon == NULL`
>    (colour icons off, `best_match` found nothing suitable at load, or a
>    `NULL ob_spec`), fall back to the **mono** `ib_pdata`/`ib_pmask` with
>    fg/bg swapped when `SELECTED` — this is the path NewTOS implements
>    today (`tos-api/src/aes/object.rs:2675-2692`, comment `AUD-P3-006`,
>    → `draw_icon`).
> 3. **Background** (`:755-768`): unless `WHITEBAK` is set and the icon's
>    `bgcol` is white, blit the mask in `bgcol`/`fgcol` (`gr_gblt`); if the
>    label string is non-empty, fill the text rect with `fgcol` (a SELECTED
>    colour icon) or `bgcol` (otherwise).
> 4. **Image** (`:770-795`): for a colour icon, `gr_colourblit(pdata, pi,
>    cicon->num_planes)` — a multi-plane `vro_cpyfm`-style blit (mode
>    `S_OR_D`, or `S_AND_D` on the truecolor path, `gemgraf.c:642-671`,
>    cross-reference [`../vdi/raster/_raster.md`](../vdi/raster/_raster.md));
>    for `SELECTED` **without** a `sel_data` variant, darken by ANDing a
>    **dithered** copy of the mask (`dither_mask`: alternating `0xaaaa`/
>    `0x5555` per scanline, `gemgraf.c:680-700`) over the image in
>    black/white — a **second**, currently-unimplemented SELECTED mechanism
>    distinct from `sel_data`, used only when the resource has no dedicated
>    selected-state art. For a mono icon (`cicon == NULL`),
>    `gr_gblt(pdata, pi, fgcol, bgcol)` instead (already implemented,
>    `draw_icon`/`blit_plane`).
> 5. **Character + label** (`:797-806`): the overlay character in `fgcol`
>    (`SMALL` font) and the label text (`gr_gtext`, `TE_CNTR`, `SMALL`) —
>    identical to the mono path, already implemented.
>
> **NewTOS status (2026-09-19, TASK/A41): implemented.** The `G_ICON |
> G_CICON` arm (`object.rs` `just_draw`) reads the `ICONBLK` head for both
> types and, for a `G_CICON`, `CICONBLK.mainlist` (`ciconblk_mainlist`,
> @48 LP64 / @36 ILP32) → `Cicon::read_abi`; `draw_icon(backend, ib, x, y,
> state, cicon, clip)` is `gr_gicon`: steps 1–5 above for `Some(cicon)` —
> `sel_data`/`sel_mask` selection, the mask in `bgcol` and the label box in
> `fgcol`/`bgcol` unless `WHITEBAK` + white (`WHITEBAK` is honoured on the
> colour path only; the mono path never checked it, unchanged), the image
> through `vdi::blit_device_mem` (a Memory→Screen `vro_cpyfm` with
> `S_AND_D` — NewTOS's one device form is the truecolor RGB565 one, so the
> `num_planes > 8` mode choice of `gemgraf.c:664-667` is always the AND
> arm), the `dither_mask` darkening for a SELECTED variant without
> `sel_data`, then character and label. `None` keeps the mono path
> byte-for-byte. Pixel tests: `tos-api/tests/aes_cicon_draw.rs`. The
> loader side (`best_match` at load, device-form transform) and the open
> `ap_rscmem` contract question are in
> [`resource-fileselector.md`](./resource-fileselector.md) §5.9 (status
> callout) and §5.11 item 5.

7. **`G_USERDEF`**: call the application's `ub_code` draw routine with a
   `PARMBLK` (`ob_user`, `gemoblib.c:433-448`).
8. **State decorations** (after the type body, `gemoblib.c:867-943`):
   `OUTLINED` → a black box 3 px out + white box 2 px out (double frame);
   `SHADOWED` → solid drop-shadow to lower-right sized by `th`; `CHECKED`
   → check-mark char at top-left; `CROSSED` → an X; `DISABLED` → dither
   grey; `SELECTED` (non-3D) → XOR-invert the whole rect.

### `objc_find` (43)

**Source:** `gemsuper.c:214-215` → `ob_find` (`gemoblib.c:1034-1113`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_STARTOB` | object to start the search under |
| `int_in[1]` `OB_DEPTH` | max levels of children to descend |
| `int_in[2]` `OB_MX` | mouse x |
| `int_in[3]` `OB_MY` | mouse y |
| `int_out[0]` | index of the **deepest** object under (mx,my), or `-1` |

**The hit-test rule (contract).** Starting at `OB_STARTOB`, `ob_find`
descends the tree: an object is a candidate if `(mx,my)` is **inside its
rect** (absolute coordinates via `ob_actxywh`) **and** it is not `HIDETREE`
(`gemoblib.c:1080`). When a candidate has children and depth remains, the
search descends to the **last child** (`ob_tail`) and walks siblings backward
via `get_prev` — so it finds the **topmost (last-drawn) child** under the
point, matching draw order. The deepest such match is returned; `-1` if the
point is over nothing (`gemoblib.c:1112`).

> **Correction vs. the previous NewTOS doc.** `ob_find` does **not** skip
> `DISABLED` objects — it only checks `inside()` and `!HIDETREE`
> (`gemoblib.c:1080`). A disabled object is still *found* by `objc_find`; it
> is the higher-level form logic (`fm_button`) that ignores clicks on
> disabled objects (`gemfmlib.c:224`). The old spec's "deepest visible
> **non-disabled** object" was wrong — a `DISABLED` object under the cursor
> is returned by `objc_find`.

> **3D note.** With `CONF_WITH_3D_OBJECTS`, a `SHADOWED` object is hit-tested
> at its *relative* (un-expanded) rect, others at their 3D-expanded rect
> (`gemoblib.c:1062-1077`). Because 3D objects can grow visually, a click just
> inside an expanded child that lies outside an un-expanded parent i-box may
> miss — a known TOS 4 quirk EmuTOS reproduces (`gemoblib.c:1017-1032`).

### `objc_offset` (44)

**Source:** `gemsuper.c:217-218` → `ob_offset` (`gemoblib.c:285-296`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_OBJ` | object |
| `int_out[0]` | `1` (default `ret`; `ob_offset` is `void`) |
| `int_out[1]` `OB_XOFF` | absolute screen x |
| `int_out[2]` `OB_YOFF` | absolute screen y |

Sums the object's `ob_x`/`ob_y` and every ancestor's up to and including
`ROOT`, yielding the object's **absolute screen position**. Results go in
`int_out[1]`/`int_out[2]` (**not** `int_out[0]`, which is unused here).

### `objc_order` (45)

**Source:** `gemsuper.c:220-221` → `ob_order` (`gemoblib.c:1201-1246`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_OBJ` | object to move |
| `int_in[1]` `OB_NEWPOS` | new sibling position: `0` = head (front), `NIL(-1)` = tail (back), `n` = after the n-th sibling |
| `int_out[0]` | `TRUE(1)`; `FALSE(0)` if `obj == ROOT` |

Unlinks the object (via `ob_delete`) then re-inserts it among its siblings at
`new_pos`. `new_pos == 0` puts it at the head, `NIL` at the tail, otherwise
after position `new_pos`. Cannot reorder `ROOT` → `FALSE`.

### `objc_edit` (46)

**Source:** `gemsuper.c:223-227`. The arm sets `gsx_sclip(&gl_rfull)`, copies
`OB_ODX = OB_IDX`, then calls `ob_edit` (`gemobed.c:335-463`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_OBJ` | editable object (`G_FTEXT`/`G_FBOXTEXT` with `EDITABLE`) |
| `int_in[1]` `OB_CHAR` | the input character (for `EDCHAR`) |
| `int_in[2]` `OB_IDX` | current cursor index (copied to `OB_ODX` on entry) |
| `int_in[3]` `OB_KIND` | `EDSTART=0`, `EDINIT=1`, `EDCHAR=2`, `EDEND=3` |
| `int_out[0]` | `TRUE(1)` (return of `ob_edit`) |
| `int_out[1]` `OB_ODX` | updated cursor index |

**The four kinds** (`gemobed.c:361-457`):
- **`EDSTART`** — returns `TRUE` immediately (no-op; also returned early if
  `obj <= 0`, `gemobed.c:342-343`).
- **`EDINIT`** — set the cursor index to the end of the current text and draw
  the cursor.
- **`EDCHAR`** — process one keystroke: `BACKSPACE`/`DELETE` remove a char,
  `ESCAPE` clears the field, `ARROW_LEFT`/`RIGHT` move the cursor, otherwise
  validate the char against `te_pvalid[slot]` via `check()`, insert it, and
  advance. On a validation miss it may *skip ahead* to the next slot that
  matches a typed template character (`scan_to_end`). Redraws only the
  changed span (`curfld`).
- **`EDEND`** — no-op body; the caller uses it to finalise a field
  (`fm_do` calls it when focus leaves a field).

Cursor→template mapping is via `find_pos` (raw-string index → template
index, landing before an `_`). Text is capped at `te_txtlen-2` data chars.

### `objc_change` (47)

**Source:** `gemsuper.c:228-231`. The arm sets the clip rect from
`OB_XCLIP` (int_in[2..5]) then calls `ob_change` (`gemoblib.c:1253-1320`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `OB_TREE` | tree base |
| `int_in[0]` `OB_DRAWOB` | object to change |
| `int_in[2..5]` `OB_XCLIP..HCLIP` | clip rectangle |
| `int_in[6]` `OB_NEWSTATE` | new `ob_state` value |
| `int_in[7]` `OB_REDRAW` | redraw flag (non-zero = redraw) |
| `int_out[0]` | `1` (default `ret`; `ob_change` is `void`) |

> **Correction vs. the previous NewTOS doc.** The new state is at
> **`int_in[6]`** and the redraw flag at **`int_in[7]`** (`aesdefs.h:376-377`);
> the clip rect occupies `int_in[2..5]` (the same slots as `objc_draw`). The
> old table's `ob_change(tree, drawob, newstate, redraw)` argument order was
> right, but did not state the slot numbers — a caller filling `int_in[1]`
> with the new state (as for `objc_draw`'s depth) would be wrong.

Behaviour: no-op if the state is unchanged or `ob_spec == -1`
(`gemoblib.c:1263-1264`). Otherwise writes `ob_state = new_state`. If
`redraw` is 0, done. Else: `G_USERDEF` calls the user draw routine; `G_ICON`
always redraws (never XOR); other types toggle SELECTED via a fast XOR fill
when possible (avoiding a full redraw), and fall back to `just_draw`
otherwise. **The mouse is hidden (`gsx_moff`) for the whole redraw** —
`gemoblib.c:1275` opens the bracket right after `ob_offset` and `:1319`
closes it after the `just_draw`, so the `G_USERDEF` arm is inside it too.
This is the same requirement `objc_draw` above carries, and it is not
cosmetic: the cursor is a save-under sprite, so drawing into the sprite's
16×16 box while it is visible makes the *next* cursor step restore the
pre-draw pixels. See A65.

> **Addition (D-093a): deselecting a `G_IMAGE` XORs *and* redraws — the
> two are not mutually exclusive.** In the default (non-radio) branch, when
> the `SELECTED` bit toggles, `ob_change` does an XOR background-fill and
> then sets `redraw = !ok` where `ok = xor_is_ok(...)`
> (`gemoblib.c:1295-1304`). `xor_is_ok` returns `FALSE` for `G_IMAGE`
> unconditionally (`gemoblib.c:114`), so `ok` is `FALSE` and `redraw` stays
> `TRUE` — **both** the XOR fill **and** a subsequent `just_draw` run,
> whenever the object being deselected is a `G_IMAGE`. (For a
> selected→selected-XOR-toggle-only object, `ok == TRUE` and only the XOR
> fill runs, no `just_draw`.) The XOR pass undoes the background inversion
> `just_draw` applies to a *selected* `G_IMAGE`
> (`gemoblib.c:766-770`); the follow-up `just_draw` then re-blits the icon
> with its normal (non-complemented) colour. A NewTOS implementation must
> reproduce both steps for `G_IMAGE`, not treat the XOR/redraw paths as
> alternatives.

### `objc_sysvar` (48)

**Source:** `gemsuper.c:232-236` (guarded `#if CONF_WITH_3D_OBJECTS`) →
`ob_sysvar` (`gemoblib.c:1350-1423`). **Present in the default build**
(`CONF_WITH_3D_OBJECTS = 1`, `config.h:1259`). With 3D off, opcode 48 has no
arm and falls to the dispatcher `default` → `-1`.

| Slot | Meaning |
|------|---------|
| `int_in[0]` `OB_MODE` | 0 = get, non-0 = set |
| `int_in[1]` `OB_WHICH` | which variable (`LK3DIND=1`, `LK3DACT=2`, `INDBUTCOL=3`, `ACTBUTCOL=4`, `BACKGRCOL=5`, `AD3DVALUE=6`) |
| `int_in[2]` `OB_I1` | input value 1 (set mode) |
| `int_in[3]` `OB_I2` | input value 2 (set mode) |
| `int_out[0]` | `1` on success, `0` on bad `which`/out-of-range colour |
| `int_out[1]` `OB_O1` | output value 1 (get mode) |
| `int_out[2]` `OB_O2` | output value 2 (get mode) |

The 3D system variables (`gemoblib.c:43-49`, defaults from `init_3d`
`gemoblib.c:1327-1344`):

| `which` | var(s) | default | meaning |
|---------|--------|---------|---------|
| `LK3DIND` (1) | `indtxtmove`, `indcolchange` | 0, 1 | indicators: don't move text, do change colour |
| `LK3DACT` (2) | `acttxtmove`, `actcolchange` | 1, 0 | activators: move text, don't change colour |
| `INDBUTCOL` (3) | `indbutcol` | `LWHITE`/`WHITE`† | indicator button colour |
| `ACTBUTCOL` (4) | `actbutcol` | `LWHITE`/`WHITE`† | activator button colour |
| `BACKGRCOL` (5) | `backgrcol` | `LWHITE`/`WHITE`† | 3D background colour (also used by `form_alert`) |
| `AD3DVALUE` (6) | — | `ADJ3DSTD` (2), `ADJ3DSTD` (2) | get-only: the 3D size adjustment in pixels |

† `LWHITE` (8) when the palette has more than 8 colours, else `WHITE` (0)
(`gemoblib.c:1336-1343`). Setting a button/background colour `>= ws_ncolors`
returns 0 without changing anything. `AD3DVALUE` is get-only; setting it
returns 0 (`default` case). In EmuTOS these are file-local statics in
`gemoblib.c:43-49` (`backgrcol` is also read by `fm_alert`); they are the AES
3D/colour system state a NewTOS build must hold somewhere equivalent (a
sibling to the shared AES state in [`_state.md`](./_state.md)).

---

## Form Manager (`form_*`) — opcodes 50–56

Slot macros from `include/aesdefs.h:305-336`.

### `form_do` (50)

**Source:** `gemsuper.c:239-240` → `fm_do` (`gemfmlib.c:278-353`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `FM_FORM` | dialog tree |
| `int_in[0]` `FM_START` | starting editable field (0 = first editable) |
| `int_out[0]` | index of the `EXIT`/`TOUCHEXIT` object that ended the dialog |

**The modal loop (contract).** `fm_do` grabs screen+mouse ownership
(`fm_own`), flushes the keyboard, positions the cursor on the starting
editable field (`fm_inifld` → `find_obj(FORWARD)`), then loops on
`ev_multi(MU_KEYBD | MU_BUTTON)`:

- **Keyboard** → `fm_keybd` (navigation) then, if a printable char remains,
  `ob_edit(EDCHAR)` into the focused field. `Return`/`Enter` selects the
  `DEFAULT` object and (if found) ends the loop.
- **Button** → `ob_find(ROOT, MAX_DEPTH, x, y)` to locate the clicked object.
  `NIL` (clicked nothing) rings the bell (`Bconout(2, 0x07)`) and continues.
  Otherwise `fm_button` processes selection.
- On leaving a field (focus change or exit) `ob_edit(EDEND)` finalises it.

Returns the index of the object that terminated the loop.

**`fm_button`** (`gemfmlib.c:201-270`) is the selection engine, and encodes
these contract behaviours:
- **`TOUCHEXIT`** exits on the down-click (`cont = FALSE`); a *double* click
  ORs `0x8000` into the returned object index (`gemfmlib.c:216-221`).
- A **disabled** or non-`SELECTABLE` object is not toggled
  (`gemfmlib.c:224`).
- **`RBUTTON`** (radio): among the parent's children, the previously
  selected radio sibling is deselected and the clicked one selected — all
  redrawn (`gemfmlib.c:227-247`).
- A non-radio selectable toggles SELECTED after a `gr_watchbox` press-track;
  it then waits for button-up unless `TOUCHEXIT` (`gemfmlib.c:249-256`).
- **`EXIT`**: if the object ends up SELECTED and has `EXIT`, the loop ends
  (`gemfmlib.c:260-261`).

**`fm_keybd`** (`gemfmlib.c:163-198`): `Return`/`Enter` → find `DEFAULT`
field and select it (ends dialog); `Up` → previous editable; `Tab`/`Down`
→ next editable; any other key is passed back for `ob_edit`.

### `form_dial` (51)

**Source:** `gemsuper.c:242-243` → `fm_dial` (`gemfmlib.c:360-383`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` `FM_TYPE` | phase: `FMD_START=0`, `FMD_GROW=1`, `FMD_SHRINK=2`, `FMD_FINISH=3` |
| `int_in[1..4]` `FM_IX/IY/IW/IH` | small (inner) rect `pi` |
| `int_in[5..8]` `FM_X/Y/W/H` | large (outer) rect `pt` |
| `int_out[0]` | `TRUE(1)` |

Reserves/releases screen space and animates the dialog box:
- **`FMD_START`** — reserve the screen area (no drawing here in EmuTOS).
- **`FMD_GROW`** — draw an expanding box from `pi` to `pt` (`gr_growbox`).
- **`FMD_SHRINK`** — draw a shrinking box from `pt` to `pi` (`gr_shrinkbox`).
- **`FMD_FINISH`** — redraw the desktop/window under `pt` (`w_drawdesk` +
  `w_update`).

> **Slot correction vs. the previous NewTOS doc.** The two rects occupy
> `int_in[1..4]` (`pi`) and `int_in[5..8]` (`pt`) — four WORDs each, not a
> single slot per rect (`aesdefs.h:315-322`).

> **Correction (D-092): `form_dial` does *not* save or restore the screen —
> it never calls `bb_save`/`bb_restore`.** Reading all of `fm_dial`
> (`gemfmlib.c:360-383`) line by line: `FMD_START` is a bare `switch` case
> with only a comment (`/* grab screen sync or some other mutual exclusion
> method */`) and no body; `FMD_GROW`/`FMD_SHRINK` draw an animated
> *outline* box via `gr_growbox`/`gr_shrinkbox` (vector lines, no pixel
> save/restore); `FMD_FINISH` asks the **window manager** to redraw what's
> under `pt` (`w_drawdesk`/`w_update`) rather than restoring a saved
> bitmap. **The VDI Memory-MFDB gap noted under `form_alert` below does
> not block `form_dial`** — `form_dial` has no `vro_cpyfm`/Memory-MFDB
> dependency at all. (An earlier task briefing for this validation round
> conflated `form_dial`'s screen reservation with `form_alert`'s actual
> `bb_save`/`bb_restore` background copy; corrected here — see
> `_discrepancies.md` D-092.)

### `form_alert` (52)

**Source:** `gemsuper.c:245-246` → `fm_alert` (`gemfmalt.c:319-396`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` `FM_DEFBUT` | default button number (1-based; 0 = none) |
| `addr_in[0]` `FM_ASTRING` | the alert string `char *` |
| `int_out[0]` | the button the user pressed (**1-based**) |

**The alert-string grammar** `[icon][msg][buttons]`, parsed by `fm_parse` →
`fm_strbrk` (`gemfmalt.c:78-174`):
- **`icon`** = the digit after the first `[`: `0` = none, `1` = NOTE, `2` =
  QUEST, `3` (or other) = STOP (`gemfmalt.c:163,258-270`). The icons are
  prebuilt `BITBLK`s `rs_bitblk[NOTEBB/QUESTBB/STOPBB]`, forced to a 32×32
  `G_IMAGE` (`gemfmalt.c:349-352`).
- **`msg`** = up to `MAX_LINENUM = 5` lines split on `|`, each up to
  `MAX_LINELEN = 40` chars (TOS-standard cap 32, over-length only warns).
- **`buttons`** = up to `MAX_BUTNUM = 3` buttons split on `|`, each up to
  `MAX_BUTLEN = 20` (TOS-standard cap 10).
- **Escaping:** `||` or `]]` yields a literal `|`/`]` (undocumented TOS/PC-GEM
  behaviour EmuTOS matches, `gemfmalt.c:64-76,94-98`).

**`fm_build`** (`gemfmalt.c:188-316`) sizes and positions the DIALERT tree
(`rs_trees[DIALERT]`, 10 objects `NUM_ALOBJS`), adds the icon/message/button
objects as children of `ROOT` (`ob_add`, `gemfmalt.c:274,282,292`), and
converts the character coordinates to pixels (`rs_obfix`,
`gemfmalt.c:299-300`). **`fm_alert`** itself (`gemfmalt.c:319-396`) then, in
this order: sets the `DEFAULT` flag on button `defbut` (`gemfmalt.c:343-347`,
*after* `fm_build` returns), fixes up the icon object to `G_IMAGE`/32×32
(`:349-352`), centres the tree (`ob_center`, `:355`), saves the screen
underneath (`bb_save`, `:363`), draws it (`ob_draw`, `:367`), runs `fm_do`
(`:383`), then restores the saved screen (`bb_restore`, `:390`) — with no
cursor bracket of its own: `bb_set` hides the mouse for both copies
(`gemgsxif.c:367,378`; see `docs/api/vdi/raster/_raster.md` "AES consumer"),
so the sprite is never saved as background. The return
is `i - BUTOFF + 1` — the pressed button's **1-based** number
(`gemfmalt.c:395`).

> **Correction (D-090).** An earlier version of this spec attributed the
> `DEFAULT`-flag set, centring, screen save, draw, and `fm_do` call to
> `fm_build` itself. All of these are done by `fm_alert` *after* `fm_build`
> returns; `fm_build` only builds/sizes/positions the tree and converts
> character→pixel coordinates. **The order matters for a NewTOS
> reimplementation**: `fm_build` **unconditionally assigns** (not ORs)
> `obj->ob_flags = BUTTON_FLAGS` for every button object
> (`gemfmalt.c:288`), so setting a button's `DEFAULT` bit *before* or
> *during* the tree-build step would be silently wiped out — `DEFAULT` must
> be applied strictly after `fm_build` returns, as EmuTOS does. See
> `_discrepancies.md` D-090.

> **DIALERT tree object map** (from `fm_build`/`erd.c`): object 0 = ROOT,
> object 1 = the icon (`G_IMAGE`), objects `MSGOFF..MSGOFF+4` = message
> lines (`G_STRING`, up to 5), objects `BUTOFF..BUTOFF+2` = buttons
> (`G_BUTTON` with `SELECTABLE|EXIT`, plus `FL3DACT` when
> `CONF_WITH_3D_OBJECTS`, `gemfmalt.c:52-56`). `MSGOFF`/`BUTOFF` are fixed by
> the resource (`gem.rsc`); they are **reference** internals — the observable
> contract is the string grammar and the 1-based return. Unlike the menu
> bar's drop-down boxes (D-085 in `menu.md`), these **are** genuine
> `ROOT`-children via `ob_add` — no hidden-container indirection here.

> **`form_alert`'s screen save/restore depends on VDI Memory-MFDB support
> (blocking note).** `bb_save`/`bb_restore` (`gemgsxif.c:320-393`, called
> from `fm_alert` at `gemfmalt.c:363,390`) blit through `vro_cpyfm(S_ONLY,
> …)` between the physical screen and an off-screen memory buffer `gl_tmp`
> (`fd_addr` set by `gsx_malloc`/`dos_alloc_anyram`, `gemgsxif.c:108-113`) —
> i.e. one side of the copy is always a **non-screen (Memory) MFDB**. Per
> `docs/STATUS.md` (2026-07-10 VDI-audit entry, item **F3**), NewTOS's
> current `vro_cpyfm`/`vrt_cpyfm` implementation is a **no-op when either
> side is a non-screen MFDB** (only screen↔screen copies are wired up). As
> written today, **`form_alert` (52) cannot save the background under the
> alert box before drawing it, nor restore it afterward** — this is a VDI
> raster-layer gap, not an AES spec ambiguity, and it is a prerequisite for
> a faithful `form_alert`/menu-drop-down implementation (the Menu Manager's
> own drop-down save/restore has the identical dependency, see `menu.md`
> "Used by"/Dependencies). See `_discrepancies.md` D-092.

### `form_error` (53)

**Source:** `gemsuper.c:248-249` → `fm_error` (`gemfmlib.c:436-472`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` `FM_ERRNUM` | GEMDOS/BIOS error number (a **positive** DOS error code, 0–63) |
| `int_out[0]` | `TRUE(1)` if the user did **not** pick button 1, else `FALSE(0)` |

Maps a DOS error number to a canned alert string via `rs_str` and shows it
with `fm_show` at alert level 1. Known mappings (`gemfmlib.c:443-467`):
2/18/3 → "file not found"; 4 → too many open files; 5 → access denied;
15 → invalid drive; 8/10/11 → memory/exec errors; anything else → the
generic `ALXXERR` with the number substituted. `n > 63` returns `FALSE`
without showing an alert (`gemfmlib.c:440-441`). The result is
`(ret != 1)` — TRUE unless the first button (usually Cancel) was picked.

> **Note on the error-number sign.** `form_error` takes a *positive* DOS
> error number (the absolute value of the negative TOS `E*` code); e.g. TOS
> error `EFILNF = -33` is not what `form_error` expects — the DOS critical
> handler passes small positive codes (`gemfmlib.c:436` comment
> "n = dos error number"). The exact code→string table is the contract.

### `form_center` (54)

**Source:** `gemsuper.c:251-252` → `ob_center` (`gemobed.c:67-106`).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `FM_FORM` | tree to centre |
| `int_out[0]` | `1` (default `ret`; `ob_center` is `void`) |
| `int_out[1..4]` `FM_XC/YC/WC/HC` | the resulting bounding rect (x,y,w,h) |

Centres the ROOT object on the screen: `x = (gl_width - w)/2`,
`y = gl_hbox + (gl_height - gl_hbox - h)/2` (below the menu bar), writing the
new `ob_x`/`ob_y` into the tree. The returned rect **includes the object's
`OUTLINED` (±3 px each side) and `SHADOWED` (2·|th|) extents**
(`gemobed.c:82-104`), so a caller can use it as the redraw/reserve rectangle.
Results go in `int_out[1..4]` (not `int_out[0]`).

### `form_keybd` (55)

**Source:** `gemsuper.c:254-259` → `fm_keybd` (`gemfmlib.c:163-198`). The arm
copies `FM_OCHAR = FM_ICHAR` and `FM_ONXTOB = FM_INXTOB` before the call and
sets the full clip rect.

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `FM_FORM` | dialog tree |
| `int_in[0]` `FM_OBJ` | current object |
| `int_in[1]` `FM_ICHAR` | input char |
| `int_in[2]` `FM_INXTOB` | input next-object |
| `int_out[0]` | continue flag: `TRUE(1)` keep editing, `FALSE(0)` dialog should end |
| `int_out[1]` `FM_ONXTOB` | next object to focus |
| `int_out[2]` `FM_OCHAR` | output char (0 if consumed as navigation) |

Lets an application drive dialog key handling itself: maps `Return`/`Enter`
to the `DEFAULT` field (returns `FALSE` to signal exit), `Up` to the previous
editable, `Tab`/`Down` to the next editable, and passes other keys back
unchanged in `FM_OCHAR`.

### `form_button` (56)

**Source:** `gemsuper.c:260-262` → `fm_button` (`gemfmlib.c:201-270`). Sets
the full clip rect first.

| Slot | Meaning |
|------|---------|
| `addr_in[0]` `FM_FORM` | dialog tree |
| `int_in[0]` `FM_OBJ` | clicked object |
| `int_in[1]` `FM_CLKS` | click count (1 or 2) |
| `int_out[0]` | continue flag: `TRUE(1)` keep editing, `FALSE(0)` dialog should end |
| `int_out[1]` `FM_ONXTOB` | next object (high bit 0x8000 set on a `TOUCHEXIT` double-click) |

Lets an application drive dialog click handling itself, with the same
`TOUCHEXIT`/`RBUTTON`/`EXIT`/disabled semantics as the internal `fm_do` loop
(see `form_do` above).

> **Addition (D-093b): `FM_ONXTOB` is zeroed unless the dialog is exiting or
> the clicked object is `EDITABLE`.** At the end of `fm_button`
> (`gemfmlib.c:264-267`): `if (cont && !(flags & EDITABLE)) new_obj = 0;`
> — i.e. clicking a plain (non-editable) button/box that does **not** end
> the dialog reports `FM_ONXTOB = 0`, *not* the clicked object's index. Only
> when the click is on an `EDITABLE` field (so `fm_do`/an application should
> move text-edit focus there) or the dialog is ending (`cont == FALSE`,
> in which case the real object index — possibly OR'd with `0x8000` for a
> `TOUCHEXIT` double-click — is preserved) does `FM_ONXTOB` carry the
> clicked object's index. This is why clicking an ordinary button in
> `fm_do`'s loop does not change the active edit field: `fm_button`'s
> `new_obj` output is intentionally suppressed for that case.

---

## Trap interface (parameter-block form)

All calls enter via the GEM trap (`svc #2`) with **selector 200 in `x0`** and
an **AESPB pointer in `x1`** — see [`_dispatcher.md`](./_dispatcher.md). Per
call the dispatcher reads `control[0]` = opcode and returns via `int_out[0]`.

| Op | Function | `int_in` | `addr_in` | `int_out` |
|---:|----------|----------|-----------|-----------|
| 40 | `objc_add`    | `[0]`=parent, `[1]`=child | `[0]`=tree | `[0]`=1 (void) |
| 41 | `objc_delete` | `[0]`=obj | `[0]`=tree | `[0]`=1/0 |
| 42 | `objc_draw`   | `[0]`=obj, `[1]`=depth, `[2..5]`=clip | `[0]`=tree | `[0]`=1 (void) |
| 43 | `objc_find`   | `[0]`=startob, `[1]`=depth, `[2]`=mx, `[3]`=my | `[0]`=tree | `[0]`=obj/-1 |
| 44 | `objc_offset` | `[0]`=obj | `[0]`=tree | `[0]`=1 (void), `[1]`=xoff, `[2]`=yoff |
| 45 | `objc_order`  | `[0]`=obj, `[1]`=newpos | `[0]`=tree | `[0]`=1/0 |
| 46 | `objc_edit`   | `[0]`=obj, `[1]`=char, `[2]`=idx, `[3]`=kind | `[0]`=tree | `[0]`=1, `[1]`=new idx |
| 47 | `objc_change` | `[0]`=obj, `[2..5]`=clip, `[6]`=newstate, `[7]`=redraw | `[0]`=tree | `[0]`=1 (void) |
| 48 | `objc_sysvar` | `[0]`=mode, `[1]`=which, `[2]`=i1, `[3]`=i2 | — | `[0]`=1/0, `[1]`=o1, `[2]`=o2 |
| 50 | `form_do`    | `[0]`=start | `[0]`=form | `[0]`=exit obj |
| 51 | `form_dial`  | `[0]`=type, `[1..4]`=small rect, `[5..8]`=large rect | — | `[0]`=1 |
| 52 | `form_alert` | `[0]`=defbut | `[0]`=string | `[0]`=button (1-based) |
| 53 | `form_error` | `[0]`=errnum | — | `[0]`=1/0 |
| 54 | `form_center`| — | `[0]`=form | `[0]`=1 (void), `[1..4]`=centred rect |
| 55 | `form_keybd` | `[0]`=obj, `[1]`=ichar, `[2]`=inxtob | `[0]`=form | `[0]`=cont, `[1]`=onxtob, `[2]`=ochar |
| 56 | `form_button`| `[0]`=obj, `[1]`=clks | `[0]`=form | `[0]`=cont, `[1]`=onxtob |

> Opcode 49 is unused (no arm). `objc_sysvar` (48) is present only with
> `CONF_WITH_3D_OBJECTS` (default on); without it, opcode 48 → dispatcher
> `default` → `-1`.

---

## Inputs / Outputs summary

All inputs arrive in the AESPB arrays; the object tree is always
`addr_in[0]` (except `objc_sysvar`, which takes no tree). Outputs are
`int_out[0]` (return) plus the extra `int_out[1..4]` slots noted per call for
`objc_offset`/`objc_edit`/`objc_sysvar`/`form_center`/`form_keybd`/
`form_button`. The **tree is mutated in place** (`ob_state`, `ob_edit` text,
`ob_add`/`ob_delete`/`ob_order` links, `ob_center` position), so writes must
be flushed back to the caller's memory.

---

## Error conditions

The object/form managers use small boolean/index returns, not TOS `E*`
codes:

| Condition | Source | Return |
|-----------|--------|--------|
| `objc_delete(ROOT)` or object not found | `gemoblib.c:1154,1180` | `0` |
| `objc_order(ROOT)` | `gemoblib.c:1208-1209` | `FALSE(0)` |
| `objc_find` over nothing | `gemoblib.c:1112` | `-1` |
| `objc_edit(EDSTART)` or `obj <= 0` | `gemobed.c:342-343` | `TRUE(1)` (no-op) |
| `objc_sysvar` bad `which` / colour `>= ws_ncolors` | `gemoblib.c:1371-1386` | `0` |
| `objc_sysvar` when `CONF_WITH_3D_OBJECTS == 0` | `gemsuper.c` default arm | `-1` |
| `form_error(n > 63)` | `gemfmlib.c:440-441` | `FALSE(0)` |

---

## Verification criteria

- **`objc_find` child hit:** with a tree whose ROOT contains one child box at
  (10,10,20,20), `objc_find(ROOT, depth≥1, 15, 15)` returns the **child's**
  index; `objc_find(ROOT, ≥1, 200, 200)` returns `-1` (`gemoblib.c:1080-1112`).
- **`objc_find` does not skip DISABLED:** a `DISABLED` object under the point
  is still returned (only `HIDETREE` and out-of-rect are skipped)
  (`gemoblib.c:1080`).
- **colour-word decode:** a `G_BOX` with colour word `0x1170` cracks to
  border=1(BLACK), text=1, mode=`MD_TRANS` (bit7 clear), pattern=7(`IP_SOLID`),
  inside=0(WHITE) (`gemgraf.c:612-628`).
- **`objc_delete(ROOT)`** returns `0` and leaves the tree unchanged
  (`gemoblib.c:1154-1155`).
- **`objc_offset`** of a child at `ob_x=5` inside a parent at `ob_x=10`
  (both under ROOT at 0) returns `xoff = 15` in `int_out[1]`
  (`gemoblib.c:285-296`).
- **`objc_order(obj, 0)`** makes `obj` the first child (head) of its parent
  (`gemoblib.c:1217-1221`).
- **`objc_change` slot:** setting `int_in[6] = SELECTED`, `int_in[7] = 1`
  selects and redraws the object; `int_in[7] = 0` selects without redraw
  (`gemoblib.c:1267-1271`).
- **`objc_sysvar` get `AD3DVALUE`:** `objc_sysvar(mode=0, which=6)` returns
  `int_out[1]=int_out[2]=2` (`ADJ3DSTD`) and `int_out[0]=1`
  (`gemoblib.c:1413-1416`).
- **`form_alert` return:** `form_alert("[1][Message][ OK ]", defbut=1)`
  returns `1` when the single button is pressed; a
  `"[2][Quit?][Yes|No]"` returns `1` for Yes, `2` for No
  (`gemfmalt.c:395`).
- **`form_alert` escaping:** `"[0][a||b][ OK ]"` renders the message line
  `a|b` (`gemfmalt.c:94-98`).
- **`form_error` range:** `form_error(64)` returns `FALSE(0)` without showing
  an alert (`gemfmlib.c:440-441`).
- **`form_dial` return:** every phase returns `TRUE(1)` (`gemfmlib.c:382`).
- **`objc_edit(EDINIT)`** sets `int_out[1]` (`OB_ODX`) to the length of the
  field's current text (`gemobed.c:363-364`).

---

## Edge cases discovered in source

1. **No `DRAW3D` (0x80) `ob_state` bit.** The header defines state bits only
   up to `WHITEBAK = 0x40` (`obdefs.h:100-107`). 3D drawing is driven by the
   **`ob_flags` FL3D\* bits**, not an `ob_state` bit. A NewTOS renderer must
   not invent one.
2. **Colour-word writing-mode is bit 7 of the WORD**, pattern is a 3-bit field
   (bits 4–6), and the border thickness for box types is **byte 1 of the LONG
   `ob_spec`** (`gemobjop.c:57`, `gemgraf.c:612-628`) — three independent
   fields programs frequently mis-shift.
3. **`objc_find` returns DISABLED objects.** Disabled handling is at the
   *form* layer, not the hit-test (`gemoblib.c:1080`, `gemfmlib.c:224`).
4. **`OUTLINED` draws a *double* frame** 3 px outside: black box (+6, th 1)
   then white box (+4, th 2) (`gemoblib.c:882-885`). `form_center` reserves
   ±3 px on each side for it (`gemobed.c:82-93`).
5. **`SHADOWED` shadow geometry** is sized by `|th|`, drawn to the lower-right
   as two solid fills (`gemoblib.c:894-901`); `ob_actxywh` and `form_center`
   grow the rect by `2·|th|` for it.
6. **`G_BUTTON` thickness depends on flags:** `-1`, minus 1 more for `EXIT`,
   minus 1 more for `DEFAULT` (`gemobjop.c:59-64`) — so a default-exit button
   is drawn with a thicker outline. This is how a dialog's default button
   gets its heavy border.
7. **`G_BOXCHAR`** takes its single character from **byte 0 of `ob_spec`**
   (`ob_sst` returns `*(char*)pspec`, `gemobjop.c:72`); the rest of `ob_spec`
   is the colour word.
8. **`INDIRECT` (0x100)** makes `ob_spec` a *pointer to* the spec; `ob_sst`
   and `ob_getsp` dereference one level (`gemobjop.c:36-37`,
   `gemobed.c:60-63`).
9. **Icons own their SELECTED look** — `just_draw` clears SELECTED after
   drawing a `G_ICON`, and `ob_change` never XORs an icon
   (`gemoblib.c:790,1286-1287`).
10. **`fm_button` double-click on `TOUCHEXIT`** ORs `0x8000` into the returned
    object index (`gemfmlib.c:216-220`) — the caller must mask it off.
11. **`RBUTTON` scope is siblings under one parent** — `fm_button` walks the
    parent's child chain to deselect the old radio (`gemfmlib.c:227-247`).
12. **`form_alert` icon `>= 3`** all map to STOP (`default` case,
    `gemfmalt.c:266-269`); icon digit `0` = no icon.
13. **`form_alert`/`form_error` share `alert_str[256]`** and the DIALERT tree
    — they are single-instance (not re-entrant); a NewTOS reimplementation can
    make them re-entrant since that buffer sharing is a reference workaround.
14. **`ob_edit` validation-miss skip-ahead:** typing a template literal char
    jumps the cursor to the next matching underscore slot (`scan_to_end`,
    `gemobed.c:141-149,431-438`) — a subtle data-entry behaviour.
15. **`te_pvalid` shorter than the slots** → the last validation char is
    repeated to fill (`gemobed.c:353-356`).
16. **`ob_format` `@` sentinel:** a `te_ptext` starting with `'@'` is treated
    as empty (`gemoblib.c:367-368`).
17. **`th` sign convention (D-089):** `th > 0` draws the border *inside* the
    object's nominal rect (content shrinks by `th`); `th < 0` draws it
    *outside* (content stays full-size, border is an add-on). Confirmed via
    `gr_box` (`gemgraf.c:813-832`), `just_draw`'s fill/decoration insets
    (`gemoblib.c:553,663-671,889-892`), and `ob_actxywh`'s
    `SHADOWED && border < 0` hit-test gate (`gemoblib.c:329-334`). An
    earlier version of this spec had the sign backwards.
18. **`fm_build` vs. `fm_alert` (D-090):** the `DEFAULT`-flag set, centring,
    screen save, `ob_draw`, and `fm_do` call are all done by `fm_alert`
    itself, *after* `fm_build` returns — not by `fm_build`. `fm_build`
    unconditionally **assigns** (not ORs) each button's `ob_flags` to
    `BUTTON_FLAGS` (`gemfmalt.c:288`), so any `DEFAULT`-flag set must happen
    strictly after `fm_build`, or it is silently lost.
19. **`ob_format` has only two fill directions, not three (D-091):**
    `TE_RIGHT` anchors/fills backward from the template's end; `TE_LEFT`
    *and* `TE_CNTR` both take the same forward-fill branch — there is no
    distinct "centred" underscore-fill in `ob_format` itself
    (`gemoblib.c:361-408`). Whole-string `TE_CNTR` box-centring happens
    later, in `gr_just`/`gr_gtext` (`gemgraf.c:567-605`).
20. **`form_dial` never saves/restores the screen; `form_alert` does, but
    NewTOS's raster layer currently can't (D-092):** `fm_dial`
    (`gemfmlib.c:360-383`) has no `bb_save`/`bb_restore`/`vro_cpyfm` call at
    all — it only draws animated outline boxes and asks the window manager
    to redraw on finish. `fm_alert`'s `bb_save`/`bb_restore`
    (`gemfmalt.c:363,390`) *do* copy Screen↔Memory via `vro_cpyfm`, which
    per `docs/STATUS.md` (2026-07-10, item F3) is currently a no-op in
    NewTOS for any non-screen MFDB — a VDI-layer blocker for a faithful
    `form_alert`.
21. **Deselecting a `G_IMAGE` both XORs and redraws (D-093a):**
    `xor_is_ok` always returns `FALSE` for `G_IMAGE` (`gemoblib.c:114`), so
    `ob_change`'s SELECTED-toggle path does the XOR background-fill *and*
    falls through to a full `just_draw` (`gemoblib.c:1295-1304`) — the two
    are not exclusive alternatives for this one type.
22. **`fm_button`'s `FM_ONXTOB` is zeroed for non-editable, non-exiting
    clicks (D-093b):** `if (cont && !(flags & EDITABLE)) new_obj = 0;`
    (`gemfmlib.c:264-267`) — clicking an ordinary button that doesn't end
    the dialog reports next-object `0`, not the clicked index.

---

## Dependencies

### Requires (must be initialized / functional)

- The **VDI / rasteriser** (NewTOS `RasterBackend`) for all of `objc_draw`,
  `objc_change`, `form_alert`, `form_dial`: box/line/fill, text (`gr_gtext`),
  icon/image blits, and clipping (`gsx_sclip` → `vs_clip`). **`form_alert`
  additionally needs Screen↔Memory `vro_cpyfm`** for `bb_save`/`bb_restore`
  (`gemfmalt.c:363,390`) — currently a no-op gap in NewTOS per
  `docs/STATUS.md` (2026-07-10, item F3); see D-092. `form_dial` has **no**
  screen save/restore dependency (D-092) — it only needs outline-box drawing
  (`gr_growbox`/`gr_shrinkbox`) and the window manager's desktop redraw.
- The **Event Manager** ([`event.md`](./event.md)) for `form_do`'s modal loop
  (`ev_multi`, `ev_button`) and the mouse-owner handshake
  ([`_state.md`](./_state.md) `gl_mowner`).
- The **resource** `gem.rsc` (DIALERT tree + NOTE/QUEST/STOP `BITBLK`s) for
  `form_alert`/`form_error`.
- `ob_sst`/`everyobj`/`get_par` tree helpers and the colour-word cracker.
- The **keyboard decode** for `objc_edit`/`form_keybd`
  (cf. [[keyboard-via-atari-ikbd]]).

### Used by

- **Every GEM dialog / desktop element.** The NewTOS desktop's rendering,
  hit-testing, dialogs and icons all go through `objc_draw`/`objc_find`/
  `form_do`/`form_alert`.
- The **Menu Manager** ([`menu.md`](./menu.md)) draws menu trees with the same
  `objc_*` object model.

---

## Related data structures

- [`../structures/object.md`](../structures/object.md) — `OBJECT` node,
  `G_*` types, `ob_flags`/`ob_state` bits, the colour-word decode.
- [`../structures/tedinfo.md`](../structures/tedinfo.md) — text/editable
  field payload, `te_pvalid` validation codes.
- [`../structures/iconblk.md`](../structures/iconblk.md) — `G_ICON`/`G_CICON`
  payload, mask/data/char/text + SELECTED swap.
- [`../structures/bitblk.md`](../structures/bitblk.md) — `G_IMAGE` payload.
- The **DIALERT** alert tree (`gem.rsc`) — reference layout used by
  `form_alert`.

---

## Cross-references

### Atari Compendium / Profibuch

- **Not present in `references/`** (no PDFs). Cross-check of the `objc_*`/
  `form_*` slot layouts, the alert-string grammar, the colour-word bit split,
  the `ob_flags`/`ob_state` values, the `objc_find` rule and the `te_pvalid`
  codes against Compendium/Profibuch is **deferred** until the PDFs are
  available. EmuTOS `1a7484ab` is authoritative per CLAUDE.md Rule 1.

### AES dispatcher

- [`_dispatcher.md`](./_dispatcher.md) — the selector-200 call model,
  `control[]` counts, `int_out[0]` return, and the `default` → `-1` path.

---

## Configuration / version notes

- **`CONF_WITH_3D_OBJECTS`** (default **1**, `config.h:1259`): enables the
  FL3D\* bevel/activator/indicator rendering, `objc_sysvar` (opcode 48), the
  3D size adjustments (`ADJ3D*`), and the coloured-background handling in
  `just_draw`/`fm_alert`. With it off, opcode 48 → `-1`, FL3D flags are inert,
  and objects draw flat.
- **`CONF_WITH_COLOUR_ICONS`** (default **1**, `config.h:1267`): enables
  `G_CICON` colour-icon drawing; off → only the mono `ICONBLK` head is drawn.
  Full draw-arm extraction: the callout after item 6 above; full load-path
  extraction: [`resource-fileselector.md`](./resource-fileselector.md) §5
  (TASK/A41).
- **`CONF_WITH_EXTENDED_OBJECTS`** (default **1**): the `G_STRING` underline
  path (`(state & WHITEBAK) && (state&0xFF00)==0xFF00`, `gemoblib.c:851`).
- **`CONF_WITH_NICELINES`** (default **1**): a `DISABLED` all-dashes
  `G_STRING` (a menu separator) is drawn as a solid line instead of dashes
  (`gemoblib.c:838-841`).
- **`CONF_WITH_MENU_EXTENSION`** changes the `ev_multi` argument shape in
  `fm_do` (`gemfmlib.c:311-317`) — a reference detail, not a slot contract.

---

## NewTOS implementation notes

- **EC1 milestone** = the first visible GEM UI, and it hinges on this file:
  `objc_draw` + `form_alert`/`form_do`. Natural build order: rasteriser →
  `objc_draw` → `form_alert` → `form_do`.
- **The object tree is a user-supplied array** (`addr_in[0]`), mutated in
  place — apply the [`_dispatcher.md`](./_dispatcher.md) user/kernel boundary
  handling; writes to `ob_state`/`te_ptext`/link fields go back to user
  memory. The m68k-vs-native `ob_spec` width follows the caller's ABI
  (COMPATIBILITY "Native vs m68k struct variants").
- **`ob_draw`/`ob_find` recursion is bounded by the tree** and `MAX_DEPTH=8`;
  no dynamic allocation — fits `#![no_std]` without `alloc`.
- **`form_do` owns an event loop** → depends on the Event Manager and the
  mouse-owner model being up.
- **The `RasterBackend` is where NewTOS deviates by design**: the *pixel
  drawing* is reference (NewTOS may draw more cleanly), but the struct
  layouts, bit values, colour-word format, `objc_find` rule, and the
  `objc_*`/`form_*` ABIs above are hard contract.
- **`form_alert`/`form_error` re-entrancy:** EmuTOS's shared `alert_str` and
  single DIALERT tree are a scarcity workaround; NewTOS may build the alert
  tree per-call to allow nested/DA alerts, provided the string grammar and
  1-based return are preserved.
- **`G_CICON` colour drawing is implemented** (TASK/A41, 2026-09-19): the
  arm follows `CICONBLK.mainlist` to the variant the loader's `fix_cicons`
  chose and draws it per `gr_gicon` (`draw_icon` with `Some(cicon)`);
  without one it takes the mono `ICONBLK` path as before (`AUD-P3-006`).
  Full spec: the callout after item 6 in the `just_draw` walkthrough above,
  plus [`resource-fileselector.md`](./resource-fileselector.md) §5.
