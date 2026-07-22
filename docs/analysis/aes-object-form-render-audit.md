# AES OBJECT/FORM Render Audit — Fix-B Campaign Reference

**Purpose.** This is the authoritative reference for the serial **Fix B** campaign
(developer → tester → commit, one divergence-group at a time) to make the AES
OBJECT/FORM render path (`tos-api/src/aes/object.rs`, function `just_draw` and its
helpers) faithful to EmuTOS. QED — an unmodified Atari GEM editor — is the first
real app to drive this path and its OBJECT/FORM rendering is badly wrong. Every
finding below is grounded in **exact file:line on both sides** (EmuTOS is the
specification; the extracted spec `docs/api/aes/object-form.md` is cross-checked
against the source). The campaign target is **EmuTOS's real behaviour, which means
the 3D look where 3D flags are present (`add_3d_effect` bevels, `FL3D*` colours)
and the classic 2D look where they are not** — every proposed fix must land the
correct 3D result, never strip to flat 2D.

> **Ground-truth note on 3D reachability.** Neither EmuTOS `rs_obfix`
> (`gemrslib.c:89-99`) nor NewTOS's resource loader adds `FL3D*` flags to a loaded
> resource. So **QED's own dialogs render through the NON-3D branches** (its RSC
> predates 3D objects), while **the AES's own kernel-built alerts DO carry 3D
> flags**: `form_alert` builds the root as `G_BOX | FL3DBAK | OUTLINED`
> (`tos-api/src/aes/form.rs:214-217`) and every button as
> `G_BUTTON | SELECTABLE | EXIT | FL3DACT` (`form.rs:230-236`). Therefore the 2D
> findings (D1, D3/D4/D5) hit QED dialogs directly, and the 3D findings (D7, the
> 3D branch of D4) hit every `form_alert`/`form_error` box. Both paths are live.

**Spec status headline.** `docs/api/aes/object-form.md` is **correct** on every
audited point (left-aligned strings, negative button `th`, `INDIRECT` deref,
`OUTLINED` double-frame, no `DEFAULT` frame, `G_USERDEF` = `ub_code` callback). The
bugs are all in the **code**, which diverges from an already-correct spec — so **no
`docs/api` correction is required before coding** (Rule 5 is satisfied). The single
spec *gap* (not error) is that the pixel-level 3D grow/bevel predicate is
explicitly deferred (`object-form.md:37`); tightening it is optional (see D7).

---

## Ranked divergence table (worst → least visual impact for the Fix-B campaign)

| Rank | ID | Severity | One-line symptom | Reached by |
|------|----|----------|------------------|-----------|
| 1 | **D1** | blocker | Every `G_STRING` label/alert line drawn **16 px too far right** (2-cell code indent); check mark mislocated | QED dialogs, every `form_alert` message line |
| 2 | **D4** | blocker | `G_BUTTON` border hardcoded `th=+1` (1 px **inside**) instead of `-1/-2/-3` (**outside**); `EXIT`/`DEFAULT` ignored; 3D button loses its outline (`th` zeroed) | Every QED button + every alert button |
| 3 | **D3** | major | **Invented** `DEFAULT` double-frame 3–4 px out — EmuTOS draws no such thing; the default look must come from D4's thicker negative `th` | QED/dialog default buttons |
| 4 | **D7** | major | `FL3DBAK` panels **wrongly grown 2 px + beveled**; must be flat (bg-colour fill + `draw_3d_outline` when `OUTLINED`) | Every `form_alert` background panel |
| 5 | **D5** | major | Negative box/button border **1 px too thin, not edge-inclusive**: draws `|th|` px, EmuTOS draws `|th|+1` px (rect edge included) | Hollow-interior boxes + buttons (couples with D4) |
| 6 | **D2** | major (structural) | No `SMALL` (6×6) font; `te_font` ignored — icon char/label and `SMALL` text objects render at 8×16 | Desktop icon labels; QED only if a field uses `SMALL` |
| 7 | **D9** | minor | `G_TITLE` indented 1 cell (8 px) + spurious white fill; EmuTOS draws title text at `ob_x`, no fill | Menu-bar titles (shares D1's code) |
| 8 | **D6** | major (large scope) | `G_USERDEF` (24) and `G_CICON` (33) **draw nothing** (`_ => {}`) | Any USERDEF/CICON object → invisible |
| 9 | **D8** | minor | `INDIRECT` (0x0100) flag ignored — `ob_spec` never dereferenced one level | Rare; apps that use `INDIRECT` |
| — | **N1** | minor | Post-body decorations (`CROSSED`/`DISABLED`/`CHECKED`/`SELECTED`) use the full rect; EmuTOS uses the `th`-inset rect for `th>0` boxes | Positive-`th` boxes with those states |

**Refuted / adjusted hypotheses:** none of D1–D9 were wrong in substance, but two
need reframing — **D3 and D4 are one physical bug** (button thickness), and **D5
only manifests on hollow interiors** (solid fills overpaint the extra ring, so
solid boxes already match). See each section. One new minor divergence (**N1**) was
found; the "verified-correct" set held up (see bottom).

---

## D1 — `G_STRING` / label text drawn 16 px too far right  **[CONFIRMED · BUG]**

**Current behavior** — `tos-api/src/aes/object.rs:1714-1733` (G_STRING arm):

```rust
let cw = crate::vdi::font::CELL_W;          // = 8
if ob_state & CHECKED != 0 {
    backend.draw_glyph(x, y + (h - CELL_H)/2, glyph(0x08), ink, Transparent, clip); // check at ob_x
}
let s = read_cstr(ob_spec);
// indent past the check column (2 cells)
draw_text(backend, x + 2 * cw, y, w, h, s, TE_LEFT, ink, clip);   // ← text at ob_x + 16px
```

**EmuTOS-correct behavior** — `references/emutos/aes/gemoblib.c:801-846` draws the
string at **`t.g_x`** (the object's own x, no code indent):

```c
tmpx = t.g_x;                                  // gemoblib.c:817
tmpy = t.g_y + ((t.g_h-gl_hchar)/2);           // vertical centre
if (obtype == G_BUTTON) tmpx += ((t.g_w-(len*gl_wchar))/2);   // centre ONLY for buttons
...
gsx_tblt(IBM, tmpx, tmpy, len);                // gemoblib.c:845 — IBM (8x16) font
```

The CHECKED mark is a **separate** later step, drawn at **`t.g_x+2` (2 pixels,
not 2 cells)** over the *start* of the string (`gemoblib.c:903-908`):

```c
if (state & CHECKED) { gsx_attr(TRUE, MD_TRANS, BLACK);
    intin[0] = 0x08; gsx_tblt(IBM, t.g_x+2, t.g_y, 1); }
```

So in EmuTOS the menu indent comes from **leading spaces embedded in the menu-item
string** (GEM convention), and the check mark overwrites that leading blank at
`+2px`. There is no `2*cell` code indent anywhere.

**Why it's wrong for QED.** QED dialog labels ("Search:", "Replace:", …) have no
leading spaces, so NewTOS shifts every label **16 px right** of its object rect,
overflowing past the intended text box (`draw_text`'s `box_clip` moves with the
shifted `x`, so it overflows rather than truncates) — labels collide with adjacent
fields / the box border. `form_alert` message lines (`form.rs:222-226`, `ob_width =
msg_width` exactly) are shifted 16 px right of the icon/box alignment. Highest-count
object type on screen → biggest visible payoff.

**3D relevance.** None — `G_STRING` is always non-3D and always IBM font. The fix
does not touch the 3D path.

**Proposed fix sketch.** Draw the string at `x` (drop `x + 2*cw` → `x`); draw the
CHECKED glyph at `x + 2` px (not `x`), matching `t.g_x+2`. **Coupling risk:** if
NewTOS's desktop menu builder emits item strings *without* EmuTOS's leading blanks,
removing the indent will jam menu items against the check column — the menu-string
builder must carry the 2 leading spaces EmuTOS menu resources use. Verify with a
menu screendump after the change (out-of-scope for object.rs, flag to menu owner).

**Spec impact.** None — `object-form.md:210-211` already says "left-aligned,
vertically centred"; the code diverges from the spec too.

---

## D4 — `G_BUTTON` border thickness hardcoded, `EXIT`/`DEFAULT` ignored, 3D outline dropped  **[CONFIRMED · BUG]**

**Current behavior** — `object.rs:1657-1663`:

```rust
let th = if d3 != 0 {
    0                          // 3D → no border at all (WRONG: EmuTOS keeps the negative border)
} else if ob_type == G_BUTTON {
    1                          // every 2D button → 1px INSIDE border (WRONG sign & direction)
} else {
    ((ob_spec >> 16) as u8 as i8) as i16    // box types: byte 1 — correct
};
```

A button's `th` never depends on `EXIT`/`DEFAULT`, is `+1` (inside) instead of
negative (outside), and is forcibly **zeroed** for 3D buttons.

**EmuTOS-correct behavior** — `references/emutos/aes/gemobjop.c:59-65` (`ob_sst`):

```c
case G_BUTTON:
    th--;                                 // base = -1
    if (objptr->ob_flags & EXIT)    th--; // EXIT     → -2
    if (objptr->ob_flags & DEFAULT) th--; // DEFAULT  → -2 (both → -3)
    break;
```

and the border is drawn whenever `th != 0`, **regardless of 3D**
(`gemoblib.c:663-667`):

```c
if (th != 0) { gsx_attr(FALSE, MD_REPLACE, bcol); gr_box(t.g_x, t.g_y, t.g_w, t.g_h, th); }
```

with `add_3d_effect` layered **on top** afterwards (`gemoblib.c:950-951`). So a 3D
`FL3DACT|EXIT` alert button (`th=-2`) gets a **thick black outline AND a bevel**;
NewTOS drops the outline.

**Why it's wrong for QED.** Every QED button gets a thin 1-px border on the *inside*
of its rect instead of EmuTOS's `|th|+1`-px black frame straddling the *outside*
edge; the button's footprint and weight are wrong. A DEFAULT button gets no heavier
outline (all buttons look identical), so the user can't see which button Enter
triggers. Alert buttons (`form.rs:234`, `th` should be `-2`) lose their outline
entirely, leaving only a bevel.

**3D relevance.** Central. The fix must **keep** `th` negative under 3D (do NOT zero
it), draw the negative border first, then `add_3d_effect` — so the 3D button keeps
its black outline *and* its bevel exactly like EmuTOS. Do not strip either.

**Proposed fix sketch.** Replace the button branch with the `ob_sst` derivation:
`let mut th = -1; if EXIT { th -= 1; } if DEFAULT { th -= 1; }` and use it for
**both** 2D and 3D buttons (remove the `if d3 != 0 { 0 }` special-case for buttons
— box types may still legitimately use their spec byte). Pair with D5 (edge-inclusive
negative border) and D3 (delete the invented DEFAULT frame).

**Spec impact.** None — `object-form.md:53-56, 106-111, 774-777` already document the
`-1/-2/-3` derivation and the sign convention. Code diverges from the spec.

---

## D3 — invented `DEFAULT` double-frame  **[CONFIRMED · BUG]** (merge into D4)

**Current behavior** — `object.rs:1857-1860`:

```rust
if ob_flags & DEFAULT != 0 {
    draw_border(backend, x - 3, y - 3, w + 6, h + 6, 1, BLACK, clip);
    draw_border(backend, x - 4, y - 4, w + 8, h + 8, 1, BLACK, clip);
}
```

Two 1-px frames, 3 px and 4 px **outside** every object carrying the `DEFAULT`
flag, of any type.

**EmuTOS-correct behavior.** `just_draw` **never reads the `DEFAULT` flag** — a grep
of `references/emutos/aes/gemoblib.c` finds `DEFAULT` **only** in `ob_sst`
(`gemobjop.c:63-64`), where it makes the button's `th` more negative. The
"default look" is *entirely* a consequence of that thicker outward border (D4).
Nothing is drawn 3–4 px out; there is a `1..2 px` gap between NewTOS's fake frame
and the button that EmuTOS never produces.

**Why it's wrong for QED.** The default button shows a detached pair of thin frames
floating around it instead of a solid heavy outline hugging the edge.

**3D relevance.** Removing this does not lose the 3D look — the heavy default frame
is reproduced by D4's `th=-2/-3` negative border, which sits correctly against the
button edge under both 2D and 3D.

**Proposed fix sketch.** Delete the `DEFAULT` block entirely; its visual role is
supplied by D4. (`object-form.md:758-761` explicitly warns "must not invent" 3D
drawing — this is the same class of mistake.)

**Spec impact.** None (spec documents no DEFAULT frame).

---

## D7 — `FL3DBAK` panels wrongly grown + beveled  **[CONFIRMED · BUG]**

**Current behavior** — the grow/bevel predicate binds to the **2-bit mask**
`FL3DMASK (0x0600)`, so `FL3DBAK (0x0400)` is treated like an activator/indicator:

- `object.rs:1599`  `let d3 = ob_flags & FL3DMASK;`
- `object.rs:1607-1611`  `if d3 != 0 { grow by ADJ3DSTD each side } …`
- `object.rs:1901-1905`  `FL3DIND | FL3DBAK => add_3d_effect(…, false, …)` — recessed bevel.

So a `FL3DBAK` panel is grown 2 px on every side **and** given a recessed 3D bevel.

**EmuTOS-correct behavior.** The grow and the bevel key on the **single bit
`FL3DOBJ (0x0200)`**, which `FL3DBAK (0x0400)` does **not** set
(`references/emutos/include/obdefs.h:92-97`):

```c
#define FL3DOBJ  0x0200   /* indicator OR activator */
#define FL3DMASK 0x0600
#define  FL3DIND 0x0200   #define  FL3DACT 0x0600   #define  FL3DBAK 0x0400
```

- Grow: `if (flags & FL3DOBJ)` — `gemoblib.c:504` → `0x0400 & 0x0200 == 0` → **not grown.**
- Bevel: `if (flags & FL3DOBJ) add_3d_effect(...)` — `gemoblib.c:950` → **no bevel.**

`FL3DBAK` instead gets (a) a solid **background-colour** fill when hollow-white
(`gemoblib.c:631-634`, already handled at `object.rs:1666-1672`), and (b) when
`OUTLINED`, a special 3-line nested grey/white bevel via **`draw_3d_outline`**
(`gemoblib.c:875-878`, `draw_3d_outline` at `gemoblib.c:153-214`) instead of the
classic black+white double frame.

**Why it's wrong.** Every `form_alert`/`form_error` box (`form.rs:214-217`,
`G_BOX|FL3DBAK|OUTLINED`) is rendered with its background panel **grown 2 px past
its rect** (oversized, over-drawing neighbours), given a **bevel it must not have**,
and its OUTLINED edge drawn as the flat double-frame rather than the 3D outline. The
alert frame looks wrong on every alert the AES raises.

**3D relevance.** This *is* the 3D path and the fix keeps the 3D look — a correct
`FL3DBAK` panel is a flat grey panel with the `draw_3d_outline` recess, which is the
EmuTOS 3D background optic. It must not be turned into a raised/recessed button bevel.

**Proposed fix sketch.** Gate the grow and the `add_3d_effect` call on
`ob_flags & 0x0200` (i.e. `d3 == FL3DIND || d3 == FL3DACT`), **not** `d3 != 0`.
Keep the `FL3DBAK` background-colour fill. Add a `draw_3d_outline` helper for the
`OUTLINED && FL3DBAK` case (minor sub-item **N3**; currently `object.rs:1861-1864`
draws the classic double frame for it). The hit-test grow (`find_hit`,
`object.rs:2167-2175`) already keys on `FL3DMASK` and must be changed in lockstep so
draw and hit-test agree (EmuTOS grows the hit rect only for `FL3DOBJ`, and
`SHADOWED` is the documented exception — `gemoblib.c:1062-1072`).

**Spec impact.** Gap, not error: `object-form.md:37` defers "the pixel-for-pixel 3D
bevel". Recommend adding a one-line note that grow/bevel key on `FL3DOBJ (0x0200)`
and `FL3DBAK` is flat — route to Researcher as a spec *tightening*, not a blocker.

---

## D5 — negative border 1 px too thin / not edge-inclusive  **[CONFIRMED · BUG]** (couples with D4)

**Current behavior** — `object.rs:1950-1959`:

```rust
core::cmp::Ordering::Less => {
    let t = -th;
    draw_border(backend, x - t, y - t, w + 2*t, h + 2*t, t, col, clip);  // thickness = |th|
}
```

For `th = -1` this draws a **single 1-px ring** at `x-1` (the outer ring only).

**EmuTOS-correct behavior** — `references/emutos/aes/gemgraf.c:813-832` (`gr_box`):

```c
if (th < 0) th--;                       // -1 → -2
do { th += (th>0)?-1:1;                 // draws at th = -1, then 0
     rc_copy(&t,&n); gr_inside(&n,th); gsx_box(&n); } while (th != 0);
```

For `th=-1` this draws **two** nested outlines — `gr_inside(-1)` at `(x-1,y-1)` and
`gr_inside(0)` at the rect edge `(x,y)` — i.e. `|th|+1` rings, **edge-inclusive**
(the innermost ring is on the object's own edge). General rule: negative `th=-n`
draws `n+1` rings spanning `x-n .. x` (a border of width `n+1` whose inner pixel is
the rect edge).

**Why it's wrong.** The extra inner ring only survives visually when the interior is
**hollow** — which is exactly the button case (`ipat=IP_HOLLOW` → `gr_rect` draws
nothing, `gemgraf.c:552-553`) and hollow boxes. So a 2D button's border is drawn 1
px too thin (single ring instead of the 2-ring frame). (For *solid*-filled boxes the
fill overpaints the inner ring, so NewTOS already matches — this is why D5 reads as
"buttons/hollow only", narrower than the raw hypothesis.)

**3D relevance.** Applies under 3D too once D4 stops zeroing the button `th`.

**Proposed fix sketch.** Negative branch: `draw_border(x-t, y-t, w+2t, h+2t, t+1, …)`
(thickness `|th|+1`, outer corner at `x-|th|`, inner pixel at the rect edge `x`).
The existing border-before-fill order (`object.rs:1691-1696`) already matches
EmuTOS (`gr_box` then `gr_rect`), so a solid fill still correctly overpaints the
edge ring. Do together with D4.

**Spec impact.** None — `object-form.md:82-90` documents the outward-growing negative
border; the `|th|+1` edge-inclusive count is a code detail the spec need not pin.

---

## D2 — no `SMALL` (6×6) font; `te_font` ignored  **[CONFIRMED · BUG, structural]**

**Current behavior.** `tos-api/src/vdi/font.rs:11,13` define only `CELL_W=8`,
`CELL_H=16`; the module is the 8×16 system font only. `draw_text`
(`object.rs:1963-1997`) and `draw_icon` (`object.rs:2037-2102`) always emit 8×16
glyphs; the `G_TEXT/BOXTEXT/FTEXT/FBOXTEXT` arm (`object.rs:1742-1802`) never reads
`ted.te_font`. There is no `SMALL` font anywhere.

**EmuTOS-correct behavior.** The font is a real parameter:

- `G_TEXT` family → `gr_gtext(edblk.te_just, edblk.te_font, …)` — honours `te_font`
  (`references/emutos/aes/gemoblib.c:757`; `gr_gtext` at `gemgraf.c:595-605`).
- Icons → **always `SMALL`** for both the char and the label:
  `gsx_tblt(SMALL, …)` (`gemgraf.c:802`) and `gr_gtext(TE_CNTR, SMALL, …)`
  (`gemgraf.c:806`).
- `SMALL` resolves to the 6×6 system font — `references/emutos/bios/fnt_st_6x6.c`:
  `max_cell_width = 6`, `form_height = 6` (cell **6×6**); selected as the *minimum*
  font height in `gsx_start` (`gemgraf.c:405-407`, `vst_height(ws_chminh, …)`),
  read back by `ch_width`/`ch_height` (`gemgraf.c:456-477`).
- `G_STRING/G_TITLE/G_BUTTON` and `G_BOXCHAR` use **IBM** (8×16) — so those are
  correctly 8×16 today; the hypothesis' "labels use SMALL" is **wrong**, only icon
  text and `te_font==SMALL` text objects use SMALL.

**Why it's wrong.** Desktop/app icon labels and any `SMALL`-font text object render
in the big 8×16 font — visibly oversized and overflowing their `ib_wtext` boxes.
**QED impact is likely low** (a text editor's dialog fields normally use IBM font
and it uses no icons), so this ranks major-but-not-QED-blocking; it is a real
structural gap for the desktop.

**3D relevance.** None (font choice is orthogonal to the bevel).

**Proposed fix sketch (large).** Add a 6×6 `SMALL` font to `vdi::font` (generate
like the 8×16), thread a `font: {IBM,SMALL}` parameter (with per-font cell metrics)
through `draw_text`/`draw_icon`, pass `ted.te_font` for the text arm and `SMALL` for
icons/`G_BOXCHAR`-stays-IBM. This is a multi-file change — schedule after the 2D/3D
geometry fixes (A–C below).

**Spec impact.** None (spec references `iconblk.md` for the SMALL icon font; no error).

---

## D9 — `G_TITLE` indented 1 cell + spurious white fill  **[CONFIRMED · BUG]** (shares D1's code)

**Current behavior** — `object.rs:1735-1741`:

```rust
G_TITLE => {
    backend.fill_rect(x, y, w, h, fill, clip);                    // ← EmuTOS draws no fill
    let s = read_cstr(ob_spec);
    draw_text(backend, x + crate::vdi::font::CELL_W, y, w, h, s, TE_LEFT, ink, clip); // ← +8px
}
```

**EmuTOS-correct behavior.** `G_TITLE` shares the `G_STRING/G_TITLE/G_BUTTON` text
block (`gemoblib.c:801-846`) and draws its text at **`t.g_x`** (no indent), IBM
font. It is **not** in the box-drawing switch, so **no fill/box is drawn**; the
SELECTED (menu-dropped) highlight comes from the trailing XOR block
(`gemoblib.c:932-943`, `xor_is_ok(G_TITLE)==TRUE` → `changecol` false → whole-rect
XOR). `ob_sst` gives it `th=1` (`gemobjop.c:44-45`) but that `th` is only consumed
by the (skipped) box switch, so nothing draws from it.

**Why it's wrong.** Menu-bar titles are shifted 8 px right; the explicit white fill
is invisible on the white menu bar today but would erase a coloured/3D bar and
diverges from EmuTOS regardless.

**3D relevance.** None.

**Proposed fix sketch.** Draw title text at `x` (drop `+ CELL_W`); drop the
`fill_rect` (let the bar show through), and produce the SELECTED highlight by
inverting like the other types (fill BLACK / ink WHITE is an acceptable b/w
equivalent of EmuTOS's XOR). Same "draw at `ob_x`" change as D1 → group them.

**Spec impact.** None.

---

## D6 — `G_USERDEF` (24) and `G_CICON` (33) draw nothing  **[CONFIRMED · BUG, large scope]**

**Current behavior** — `object.rs:1840-1842`: the `match ob_type` ends with
`_ => {}`, so `G_USERDEF`, `G_CICON`, and any unknown type render **nothing**.

**EmuTOS-correct behavior.**

- `G_USERDEF` → `ob_user(tree, obj, &t, spec, state, state)` — calls the app's
  `ub_code` callback with a `PARMBLK` (`gemoblib.c:792-794`; `ob_user` at
  `gemoblib.c:433-448`, `USERBLK`/`PARMBLK` in `obdefs.h:229-246`).
- `G_CICON` → shares the `G_ICON` path with `cicon = ((CICONBLK*)spec)->mainlist`
  and draws a colour icon (`gemoblib.c:780-789`, `gr_gicon` colour branch).

**Why it's wrong for QED.** Any `G_USERDEF` object (custom-drawn control) is
invisible. **QED impact is uncertain** (a plain editor RSC may use none) → mark the
QED-specific severity **PLAUSIBLE** pending inspection of QED's RSC; the divergence
itself is CONFIRMED.

**3D relevance.** None directly.

**Proposed fix sketch.** `G_CICON` is bounded (reuse `draw_icon` with the colour
mainlist, or mono-`ICONBLK` fallback). `G_USERDEF` is **large** — it needs the
`ub_code` callback plumbing to call app code at EL0 with a `PARMBLK`; assess whether
any target app actually needs it before investing. Schedule separately, last.

**Spec impact.** None (`object-form.md:217-218` documents `G_USERDEF` correctly).

---

## D8 — `INDIRECT` (0x0100) flag ignored  **[CONFIRMED · BUG]**

**Current behavior.** No `INDIRECT` handling exists anywhere in
`tos-api/src/aes/` (grep). `ObjTree::spec` (`object.rs:1261-1270`) always returns
the raw `ob_spec`.

**EmuTOS-correct behavior** — `references/emutos/aes/gemobjop.c:36-37` (`ob_sst`):

```c
*pspec = objptr->ob_spec;
if (objptr->ob_flags & INDIRECT)   *pspec = *(LONG *)objptr->ob_spec;   // one-level deref
```

`INDIRECT = 0x0100` (`obdefs.h:91`).

**Why it's wrong.** If a program sets `INDIRECT`, NewTOS treats the *pointer-to-spec*
as the spec, dereferencing garbage (wrong box word or a wild string pointer). Rare
in practice; low QED likelihood → **PLAUSIBLE** QED impact, CONFIRMED divergence.

**3D relevance.** None.

**Proposed fix sketch.** In `ObjTree::spec` (or `just_draw`'s snapshot), when
`ob_flags & 0x0100`, dereference one level **at the tree's ABI width** (32-bit LONG
for an ILP32 app, 64-bit for LP64) — EmuTOS reads `*(LONG*)`, i.e. 32-bit; NewTOS
must read the width matching how the app stored the indirect spec. Small, isolated.

**Spec impact.** None (`object-form.md:781-783` documents `INDIRECT`).

---

## N1 — post-body decorations use the full rect, not the `th`-inset rect  **[CONFIRMED · minor]**

EmuTOS applies `if (th>0) gr_inside(&t, th)` **before** the `CHECKED`/`CROSSED`/
`DISABLED`/`SELECTED` blocks (`gemoblib.c:889-943`), so those decorations are drawn
against the border-inset rect for positive-`th` boxes. NewTOS draws them against the
full `(x,y,w,h)` (`object.rs:1871-1897`). For `th=0` objects (all strings, buttons
once D4 makes them negative) there is no difference; only positive-`th` filled boxes
with those states show a `th`-px edge discrepancy. Minor; fold into the D3/D4/D5
button-geometry group if convenient, otherwise leave. (`SELECTED` is additionally
folded into each type's inline colour inversion rather than a trailing XOR — an
**acceptable b/w-equivalent deviation**, except a selected black&white box's *border*
does not invert as EmuTOS's whole-rect XOR would; unobservable enough to leave.)

---

## Verified correct — do NOT touch

Each was driven against the EmuTOS source and matches (cite = EmuTOS · NewTOS):

- **`OUTLINED` double frame (non-3D)** — `gemoblib.c:882-885` · `object.rs:1861-1864`.
  Black `+6/th1` at `x-3` then white `+4/th2` at `x-2`; pixel-exact. (The
  `FL3DBAK`-`OUTLINED` → `draw_3d_outline` special case is the only gap → **N3**,
  part of D7.)
- **`SHADOWED` geometry** — `gemoblib.c:889-901` · `object.rs:1871-1882`. Traced for
  both `th>0` (inset rect) and `th<0` (`th'=|th|`): bottom fill `(sx, sy+sh+th', sw+th',
  2th')` and right fill `(sx+sw+th', sy, 2th', sh+3th')` match exactly.
- **`CROSSED`** — two WHITE transparent diagonals — `gemoblib.c:910-915` ·
  `object.rs:1886-1889` (modulo the N1 inset for `th>0`).
- **`DISABLED`** — 50 % `IP_4PATT` transparent dither — `gemoblib.c:917-930` ·
  `object.rs:1893-1897` (modulo N1).
- **`G_IMAGE`** — transparent mono blit in `bi_color`, SELECTED = XOR-solid background
  + inverted ink — `gemoblib.c:759-775` · `object.rs:1804-1828`.
- **`ob_format`** — two-direction template merge, `@`-empty, `TE_RIGHT` fills
  backward — `gemoblib.c:361-408` · `object.rs:1550-1585` (matches the D-091
  correction; `TE_CNTR` == `TE_LEFT` here).
- **`gr_crack`** colour-word decode — `gemgraf.c:612-628` · `object.rs:1492-1500`.
- **`gr_rect`** HOLLOW/SOLID/pattern in REPLACE mode — `gemgraf.c:547-559` ·
  `object.rs:1522-1539`.
- **`spec == -1` sentinel** skip-but-recurse and **`HIDETREE`** subtree stop —
  `gemoblib.c:491`, `everyobj gemobjop.c:106` · `object.rs:1449-1471`.
- **`G_BOXCHAR` char source** — byte 0 of `ob_spec` (`(spec>>24)&0xFF` native-int) ·
  `gemobjop.c:72` · `object.rs:1704-1708`; IBM-font, centred — matches.
- **`objc_find`/`ob_add`/`ob_delete`/`ob_order`/`objc_edit`** tree logic — ported
  faithfully with documented defensive guards (`object.rs:733-843, 1022-1065,
  2142-2207`); `objc_find` correctly returns DISABLED objects.
- **`objc_sysvar`** 3D-look variables + defaults — `gemoblib.c:1327-1423` ·
  `object.rs:868-947`.
- **`G_BUTTON` non-3D colours** (hollow-white interior, `bcol=BLACK`) —
  `gemoblib.c:639-658` · `object.rs:1652-1657` — correct (the *thickness* is D4, the
  colours are right).

---

## Proposed grouping for the serial Fix-B campaign

Order by (visual payoff ÷ size), 2D-geometry first (hits QED directly), 3D next,
structural last:

**Group A — string/title position (D1 + D9).** Shared code: both draw text at the
wrong x. Draw at `ob_x`; move the CHECKED glyph to `ob_x+2px`; drop the `G_TITLE`
fill. Tiny change, largest visual win. **Coupling:** confirm the menu-string builder
carries EmuTOS's leading blanks before/after — screendump the menu bar (menu owner).
Do **first**.

**Group B — button/box border thickness & signed border (D3 + D4 + D5, opt. N1).**
One physical bug family. Derive button `th = -1 -(EXIT) -(DEFAULT)`; use it for 2D
**and** 3D (stop zeroing `th` for 3D); delete the invented `DEFAULT` double-frame;
make the negative branch of `draw_signed_border` draw `|th|+1` edge-inclusive px.
Verify a plain button, an EXIT button, a DEFAULT button, and a 3D alert button
(outline + bevel). Do **second**.

**Group C — `FL3DBAK` flat panel (D7 + N3).** Gate grow + `add_3d_effect` on
`FL3DOBJ (0x0200)` so `FL3DBAK (0x0400)` is neither grown nor beveled; keep its
bg-colour fill; add `draw_3d_outline` for `OUTLINED && FL3DBAK`; change `find_hit`'s
grow predicate in lockstep. Verify with a `form_alert` screendump. Independent of A/B.

**Group D — `SMALL` font + font plumbing (D2). LARGE / STRUCTURAL.** New 6×6 font in
`vdi::font` + a font parameter through `draw_text`/`draw_icon`; pass `te_font` for
text objects and `SMALL` for icon char/label. Multi-file. Schedule after A–C.

**Group E — `G_USERDEF` + `G_CICON` (D6).** `G_CICON` = colour-icon draw (medium);
`G_USERDEF` = `ub_code`/`PARMBLK` EL0 callback plumbing (**large** — assess whether
any target app needs it first). Schedule last / separately.

**Group F — `INDIRECT` (D8).** One-level ABI-width deref in `ObjTree::spec`. Small,
isolated — slot in whenever convenient (no dependency).

**Spec work (parallel, Researcher):** optional tightening of `object-form.md:37` to
state the 3D grow/bevel predicate is `FL3DOBJ (0x0200)` and `FL3DBAK` is flat. Not a
blocker — the spec is not *wrong*, only silent, and the code fixes do not depend on it.

---

## Cross-side citation index (quick reference)

| ID | EmuTOS (spec) | NewTOS (current) |
|----|---------------|------------------|
| D1 | `aes/gemoblib.c:801-846` (tmpx=t.g_x @817, IBM @845), `:903-908` (check @t.g_x+2) | `aes/object.rs:1714-1733` |
| D2 | `aes/gemgraf.c:456-477,595-605,802,806`; `gsx_start :405-407`; `bios/fnt_st_6x6.c` (6×6) | `vdi/font.rs:11,13`; `object.rs:1742-1802,1963-1997,2037-2102` |
| D3 | `aes/gemoblib.c` (no DEFAULT); flag only in `gemobjop.c:63-64` | `object.rs:1857-1860` |
| D4 | `aes/gemobjop.c:59-65`; border `gemoblib.c:663-667`; bevel-after `:950-951` | `object.rs:1657-1663` |
| D5 | `aes/gemgraf.c:813-832` (`gr_box` negative → `|th|+1` edge-inclusive) | `object.rs:1950-1959` |
| D6 | `aes/gemoblib.c:792-794` (`ob_user`), `:780-789` (`G_CICON`) | `object.rs:1840-1842` (`_ => {}`) |
| D7 | `include/obdefs.h:92-97`; grow `gemoblib.c:504`; bevel `:950`; `draw_3d_outline :153-214,875-878`; bg fill `:631-634` | `object.rs:1599,1607-1611,1901-1905`; hit `:2167-2175` |
| D8 | `aes/gemobjop.c:36-37`; `INDIRECT` `obdefs.h:91` | (absent) `object.rs:1261-1270` |
| D9 | `aes/gemoblib.c:801-846` (shared text block, no fill) | `object.rs:1735-1741` |
| N1 | `aes/gemoblib.c:889-943` (inset before decorations) | `object.rs:1871-1897` |
| — reach | `form_alert` 3D tree | `tos-api/src/aes/form.rs:214-236` |
