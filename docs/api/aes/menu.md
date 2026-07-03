# AES Menu Manager (`menu_*`) — opcodes 30–39

> **PHASE-00b D8, Batch 5 (full-contract refinement).**
> Source: EmuTOS @ `1a7484ab`.
> Primary: `aes/gemmnlib.c` (`mn_bar`/`do_chg`/`mn_register`/`mn_cleanup`/
> `mn_getownid`, the drop-down tracking `mn_do`), `aes/gemmnext.c`
> (`mn_popup`/`mn_attach`/`mn_istart`/`mn_settings` — AES 3.30 extensions);
> dispatch arm `aes/gemsuper.c:167-201` (`crysbind`); the MN_SELECTED message is
> built by the control manager `aes/gemctrl.c:328-371` (`hctl_rect`/`ct_msgup`)
> → `aes/gemwmlib.c:851-863` (`ap_sendmsg`); opcode + slot macros
> `include/aesdefs.h:36-45,266-299`; menu structs `aes/gemmnext.h:16-33`.
>
> Call convention + AESPB layout: [`_dispatcher.md`](./_dispatcher.md).
> Shared AES globals (the active menu tree `gl_mntree`/`gl_mnppd`, the DA
> registration slots, the message pipe): [`_state.md`](./_state.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** opcode numbers (30–39); the `int_in`/`addr_in` slot layout of
> each call (**every menu selector puts its primary index in `int_in[0]` and its
> state/flag in `int_in[1]`**, not `int_in[1]`/`int_in[2]`); `menu_bar`'s
> `show=1`/`0` draw/erase with the tree in `addr_in[0]`; the object `ob_state`
> bits flipped (`CHECKED`/`DISABLED`/`SELECTED`); the `menu_ienable` item bit-15
> = *redraw-now* flag; the **MN_SELECTED (10) message layout** (`msg[3]`=title
> obj, `msg[4]`=item obj, and — with `CONF_WITH_MENU_EXTENSION` — `msg[5..7]` =
> tree-ptr hi/lo + submenu parent); the `menu_register` slot-id / -1 return; the
> `ME_INQUIRE`/`ME_ATTACH`/`ME_REMOVE` flags; the `MENU`/`MN_SET` struct field
> layout. **Reference (NewTOS reimplements freely):** `do_chg`/`menu_sub`/
> `menu_down`/`menu_sr`, the `mn_do` mouse-tracking state machine, `bb_save`/
> `bb_restore`/`popup_blit` raster save-restore, the `gl_mntree`/`gl_mnppd`/
> `desk_ppd[]`/`D.g_acctitle[]`/`gl_submenu` internals, and `ct_msgup`'s
> button-wait loop — these are the EmuTOS AES internals NewTOS reimplements on
> the object manager + its own event/IPC model.

---

## The menu bar IS an OBJECT tree

A menu bar is an ordinary **OBJECT tree** (see [`object-form.md`](./object-form.md))
with a fixed shape. Object indices used by the menu code
(`aes/gemmnlib.c:46-48`, `aes/gemctrl.c:42`):

| Index | Constant | Object | Role |
|------:|----------|--------|------|
| 0 | `THESCREEN` (= `ROOT`, `obdefs.h:20`) | `G_IBOX` | the root box covering the whole bar area |
| 1 | `THEBAR` | `G_BOX` | the visible bar strip drawn by `menu_bar` |
| 2 | `THEACTIVE` | `G_IBOX` | the box holding the application titles |
| 3 | `THEDESK` | `G_TITLE` | the **Desk** title (system menu; must equal `DESKMENU` in the desktop resource) |

Under `THEACTIVE` the **titles** (`G_TITLE`) are siblings; each title's child is
a `G_BOX` drop-down whose children are the **items** (`G_STRING`). The AES draws
the bar, tracks the mouse over titles, drops the box, highlights the item under
the cursor, and on button release posts an **`MN_SELECTED`** message carrying
`[title obj, item obj]`. So most `menu_*` calls simply flip one object's
`ob_state` bit and (sometimes) redraw it — routed through the shared `do_chg`
helper. The **tree layout, the drop-down tracking result, and the message
format are contract**; the pixel drawing is the object manager's job
(`ob_draw`/`ob_change`, cross-ref — do not re-specify here).

### `do_chg` — the shared state-setter (reference)

`do_chg(tree, iitem, chgvalue, dochg, dodraw, chkdisabled)`
(`gemmnlib.c:145-171`) sets (`dochg≠0`) or clears (`dochg=0`) the `chgvalue`
bit in object `iitem`'s `ob_state`, optionally redraws it (`dodraw`), and — if
`chkdisabled` — refuses when the item is already `DISABLED`. It returns `TRUE`
unless the disabled-guard blocked it. **The important per-call detail is which
of `dochg`/`dodraw`/`chkdisabled` each menu selector passes** (table below);
`do_chg` itself is a reimplementable helper.

### Object `ob_state` bits touched (`include/obdefs.h:100-107`)

| Bit | Value | Set by |
|-----|------:|--------|
| `SELECTED` | 0x0001 | `menu_tnormal` (title highlight), the tracking loop (item/title hilite) |
| `CHECKED`  | 0x0004 | `menu_icheck` (the ✓ mark) |
| `DISABLED` | 0x0008 | `menu_ienable` (greyed / non-selectable) |
| `SUBMENU`  | 0x0800 | `menu_attach` (`ob_flags`, marks an item as having a submenu) |

---

## Parameter-block slot map (`include/aesdefs.h:266-294`)

Every menu call enters via the GEM trap (`svc #2`, **selector 200 in `x0`**,
AESPB pointer in `x1`); the dispatcher reads `control[0]`=opcode,
`control[1..3]`=#int_in/#int_out/#addr_in, and returns `int_out[0]`=`RET_CODE`
(see [`_dispatcher.md`](./_dispatcher.md)). The exact `aesdefs.h` slot macros:

| Macro | Slot | Used by |
|-------|------|---------|
| `MM_ITREE`  | `addr_in[0]` | bar, icheck, ienable, tnormal, text |
| `MM_PSTR`   | `addr_in[0]` | register (the DA menu-item string) |
| `MM_PTEXT`  | `addr_in[1]` | text (the replacement string) |
| `SHOW_IT`   | `int_in[0]`  | bar (1 = show, 0 = erase) |
| `ITEM_NUM`  | `int_in[0]`  | icheck, ienable (item object index; ienable: bit 15 = redraw-now) |
| `TITLE_NUM` | `int_in[0]`  | tnormal (title object index) |
| `MM_PID`    | `int_in[0]`  | register (the caller's AES pid) |
| `CHECK_IT`  | `int_in[1]`  | icheck (1 = add ✓, 0 = remove) |
| `ENABLE_IT` | `int_in[1]`  | ienable (1 = enable, 0 = disable) |
| `NORMAL_IT` | `int_in[1]`  | tnormal (1 = normal, 0 = highlighted) |
| `MPOP_IN`   | `addr_in[0]` | popup (`MENU*` in), attach/istart (`OBJECT*` tree) |
| `MPOP_OUT`  | `addr_in[1]` | popup / attach (`MENU*` result) |
| `MPOP_SET`  | `addr_in[0]` | settings (`MN_SET*`) |
| `MPOP_XPOS` | `int_in[0]`  | popup (x) |
| `MPOP_YPOS` | `int_in[1]`  | popup (y) |
| `MPOP_FLAG` | `int_in[0]`  | attach/istart/settings (operation selector) |
| `MPOP_ITEM` | `int_in[1]`  | attach (item) / istart (menu box object) |
| `MPOP_ITEM2`| `int_in[2]`  | istart (start item) |

> **Correction vs. the previous spec.** The old file placed `ITEM_NUM`/
> `TITLE_NUM` at `int_in[1]` and the state flags (`CHECK_IT`/`ENABLE_IT`/
> `NORMAL_IT`) at `int_in[2]`. The source is the opposite: the **primary index
> is `int_in[0]`** (`ITEM_NUM`/`TITLE_NUM`/`MM_PID`/`SHOW_IT`) and the **state/
> flag is `int_in[1]`** (`aesdefs.h:275-283`). See D-051.

---

## Function table (dispatch → effect)

| Op | Name | Dispatch (`gemsuper.c:167-201`) | Return (`int_out[0]`) |
|---:|------|---------------------------------|-----------------------|
| 30 | `menu_bar`     | `mn_bar(MM_ITREE, SHOW_IT)` | `1` (`mn_bar` is `void`; `ret` stays `TRUE`) |
| 31 | `menu_icheck`  | `do_chg(MM_ITREE, ITEM_NUM, CHECKED, CHECK_IT, FALSE, FALSE)` | `1` (`do_chg` `TRUE`) |
| 32 | `menu_ienable` | `do_chg(MM_ITREE, ITEM_NUM&0x7fff, DISABLED, !ENABLE_IT, (ITEM_NUM&0x8000)!=0, FALSE)` | `1`/`0` (`do_chg`) |
| 33 | `menu_tnormal` | `do_chg(MM_ITREE, TITLE_NUM, SELECTED, !NORMAL_IT, TRUE, TRUE)` | `1`/`0` (`do_chg`; `0` if title `DISABLED`) |
| 34 | `menu_text`    | `strcpy(tree[ITEM_NUM].ob_spec, MM_PTEXT)` | `1` (`void`; `ret` stays `TRUE`) |
| 35 | `menu_register`| `mn_register(MM_PID, MM_PSTR)` | slot id `0..5`, or `-1` |
| 36 | `menu_popup`†  | `mn_popup(MPOP_IN, MPOP_XPOS, MPOP_YPOS, MPOP_OUT)` | `1` (item chosen) / `0` |
| 37 | `menu_attach`† | `mn_attach(MPOP_FLAG, MPOP_IN, MPOP_ITEM, MPOP_OUT)` | inquire/attach/remove result (`1`/`0` or `s_start`) |
| 38 | `menu_istart`† | `mn_istart(MPOP_FLAG, MPOP_IN, MPOP_ITEM, MPOP_ITEM2)` | the submenu's `s_start` item, or `0`/`FALSE` |
| 39 | `menu_settings`†| `mn_settings(MPOP_FLAG, MPOP_SET)` | `1` (`void`; `ret` stays `TRUE`) |

† **`#if CONF_WITH_MENU_EXTENSION`** (default **1**, `include/config.h:1305`),
opcodes 36–39, the whole of `aes/gemmnext.c` (`gemsuper.c:188-201`). With the
flag off, opcodes 36–39 have **no `case`** → the dispatcher `default` arm returns
**`-1`** (see [`_dispatcher.md`](./_dispatcher.md) §unsupported-opcode). Opcodes
30–35 are unconditional (the original TOS set).

---

## `menu_bar` (30)

**Source:** `gemsuper.c:168-170` → `aes/gemmnlib.c:703-732` (`mn_bar`).
Helpers: `menu_fixup()` (`gemmnlib.c:89-125`, splices the DA items in),
`ob_draw`/`ob_actxywh` (object manager), `gsx_sclip`/`gsx_cline` (VDI draw),
`post_keybd(ctl_pd->p_cda, 0)` (wakes the control manager to re-measure its
wait rectangle). **Globals written:** `gl_mntree` (the active menu tree ptr;
`NULL` when no bar), `gl_mnppd` (the AES process that owns the bar),
`gl_ctwait.m_gr` (the control-manager wait rectangle) (`gemmnlib.c:62-63`,
`712-726`; see [`_state.md`](./_state.md)).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`MM_ITREE`) | pointer to the menu OBJECT tree |
| `int_in[0]` (`SHOW_IT`) | **1 = install/draw** the bar; **0 = remove** it |
| `int_out[0]` | `1` (`mn_bar` is `void`; `crysbind` `ret` stays `TRUE`) |

**Behaviour.** `show=1`: record `gl_mnppd = rlr`'s descriptor and
`gl_mntree = tree`; call `menu_fixup()` to append the desk-accessory items to
the tree; stretch `THEBAR`'s width to the screen; set the control-wait rect to
`THEACTIVE`'s extent; draw `THEBAR` (`ob_draw(tree, THEBAR, MAX_DEPTH)`); draw
the separator line under the bar in black replace-mode. `show=0`: clear
`gl_mntree = NULL` and restore the control-wait rect to the full desktop
(`gl_rmenu`) so clicks no longer hit the bar. Either way, post a null key to
the control manager's console queue so it re-reads the wait rectangle
(`gemmnlib.c:730`).

> `mn_bar` ignores `tree` when `show=0` (`gemmnlib.c:723-726`) — the tree used
> for erase is the one recorded in `gl_mntree` from the matching `show=1`.

---

## `menu_icheck` (31)

**Source:** `gemsuper.c:171-173` → `do_chg(tree, ITEM_NUM, CHECKED, CHECK_IT,
FALSE, FALSE)`.

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`MM_ITREE`) | menu tree |
| `int_in[0]` (`ITEM_NUM`) | item object index |
| `int_in[1]` (`CHECK_IT`) | **1 = add** the ✓ check mark, **0 = remove** it |
| `int_out[0]` | `1` (`do_chg` returns `TRUE`; `chkdisabled=FALSE`) |

Sets/clears the `CHECKED` (0x0004) `ob_state` bit. **`dodraw = FALSE`** — the
change is *not* drawn immediately; the check mark appears the next time the
drop-down is drawn (`gemmnlib.c:172`). Contract detail: `menu_icheck` alone
never repaints.

---

## `menu_ienable` (32)

**Source:** `gemsuper.c:174-177` → `do_chg(tree, ITEM_NUM & 0x7fff, DISABLED,
!ENABLE_IT, (ITEM_NUM & 0x8000) != 0, FALSE)`.

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`MM_ITREE`) | menu tree |
| `int_in[0]` (`ITEM_NUM`) | item object index in bits 0–14; **bit 15 = redraw-now flag** |
| `int_in[1]` (`ENABLE_IT`) | **1 = enable**, **0 = disable** the item |
| `int_out[0]` | `1` (`do_chg` returns `TRUE`; `chkdisabled=FALSE`) |

Sets (`ENABLE_IT=0`) or clears (`ENABLE_IT=1`) the `DISABLED` (0x0008) bit — a
disabled item is greyed and not selectable. The item index is masked with
`0x7fff`; **bit 15 of the item word is the `dodraw` flag**: if set, `do_chg`
redraws the item immediately, otherwise the change is deferred to the next draw
(`gemsuper.c:175-176`).

> **Correction vs. the previous spec.** The old file called `item & 0x8000` a
> "title-bar/special item" flag. It is not — it is the **redraw-now** (`dodraw`)
> flag passed as `do_chg`'s 5th argument (`gemsuper.c:176`). See D-051.

---

## `menu_tnormal` (33)

**Source:** `gemsuper.c:178-180` → `do_chg(tree, TITLE_NUM, SELECTED, !NORMAL_IT,
TRUE, TRUE)`.

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`MM_ITREE`) | menu tree |
| `int_in[0]` (`TITLE_NUM`) | **title** object index |
| `int_in[1]` (`NORMAL_IT`) | **1 = draw normal**, **0 = draw highlighted** (inverted) |
| `int_out[0]` | `1` (`do_chg` `TRUE`), or **`0` if the title is `DISABLED`** (`chkdisabled=TRUE`) |

Sets (`NORMAL_IT=0`) or clears (`NORMAL_IT=1`) the title's `SELECTED` (0x0001)
bit and **always redraws** (`dodraw=TRUE`). This is how an application
un-highlights a menu title after acting on an `MN_SELECTED` message — the
standard idiom is `menu_tnormal(tree, title, 1)`. **`chkdisabled=TRUE`**: on a
disabled title `do_chg` refuses and returns `0`, leaving the state unchanged.

---

## `menu_text` (34)

**Source:** `gemsuper.c:181-184` → **inline** `strcpy((char *)tree[ITEM_NUM].
ob_spec, (char *)MM_PTEXT)` (no `mn_*` helper).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`MM_ITREE`) | menu tree |
| `addr_in[1]` (`MM_PTEXT`) | new NUL-terminated string |
| `int_in[0]` (`ITEM_NUM`) | item object index |
| `int_out[0]` | `1` (inline `strcpy`, no return; `crysbind` `ret` stays `TRUE`) |

Replaces the item's text **in place**: copies the new string over the buffer at
`tree[item].ob_spec`. **No length check, no redraw** — the caller must ensure
the new string is no longer than the original allocation, and repaint the item
itself (e.g. via `objc_draw`, or a subsequent `menu_ienable` with bit 15).

> **Edge case / danger.** The in-place `strcpy` (`gemsuper.c:183`) will overrun
> the original `ob_spec` buffer if the replacement is longer. This is TOS
> behaviour a program relies on being "same length or shorter"; it is contract
> that `menu_text` overwrites the existing buffer (does not allocate a new one).

---

## `menu_register` (35)

**Source:** `gemsuper.c:185-187` → `aes/gemmnlib.c:789-816` (`mn_register`).
Helpers: `menu_fixup()` (splices the DA into the tree), `build_menuid_lookup()`
(`gemmnlib.c:766-786`, the display-slot → menu-id table). **Globals written:**
`D.g_accreg` (count of registered DAs), `D.g_acctitle[NUM_ACCS]` (the DA title
string pointers), `desk_ppd[NUM_ACCS]` (the DA owner process descriptors),
`acc_display[]` (the lookup table) (`gemmnlib.c:64-65,798-810`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` (`MM_PID`) | the registering application's AES pid |
| `addr_in[0]` (`MM_PSTR`) | pointer to the menu-item title string |
| `int_out[0]` | the assigned **slot id** `0 .. NUM_ACCS-1` (0–5), or **`-1`** on failure |

Registers a **desk-accessory** entry under the *Desk* menu. If the pid is valid
(`≥ 0`) and there is room (`D.g_accreg < NUM_ACCS`, `NUM_ACCS = 6`,
`sysconf.h:37`): increment the count, find the first free `g_acctitle[]` slot,
record `desk_ppd[slot] = rlr` and `g_acctitle[slot] = pstr` (**stores the caller's
pointer, exactly like Atari TOS — the string is not copied**), splice the item
into the menu tree (`menu_fixup`), rebuild the lookup, and **return the slot
index** (`gemmnlib.c:798-810`). If the pid is negative or all six slots are
full, return **`-1`** (`gemmnlib.c:815`).

The returned slot id is the accessory's *menu id*: when the user later picks the
DA entry, the control manager maps the display slot back to this id via
`acc_display[]`/`mn_getownid` and sends the owner an **`AC_OPEN`** message with
that id in `msg[4]` (`gemctrl.c:337-346`, `gemmnlib.c:838-849`).

> The string pointer is stored, not duplicated (`gemmnlib.c:806`) — the caller
> must keep the title string alive for the life of the registration.

---

## `menu_popup` (36) — `CONF_WITH_MENU_EXTENSION`

**Source:** `gemsuper.c:189-191` → `aes/gemmnext.c:304-357` (`mn_popup`).
Helpers: `wm_update(BEG_MCTRL/END_MCTRL)` (grab/release the mouse),
`ev_button` (wait for button-up), `popup_blit` (save/restore background),
`handle_popup`/`submenu_event` (the pop-up tracking loop, sets `keystate`),
`clamp_ypos` (keep the box on-screen).

| Slot | Meaning |
|------|---------|
| `addr_in[0]` (`MPOP_IN`) | input `MENU *` describing the pop-up (tree/menu box/start item/scroll) |
| `int_in[0]` (`MPOP_XPOS`) | x of the **start item** (not the box) |
| `int_in[1]` (`MPOP_YPOS`) | y of the start item |
| `addr_in[1]` (`MPOP_OUT`) | output `MENU *` — filled with the choice |
| `int_out[0]` | **`1`** if an item was chosen, **`0`** otherwise |

Displays the pop-up menu at (x,y), where (x,y) is the position of the *start*
item; the parent box y is derived by subtracting the start item's y offset, then
the box is clamped to the screen. Saves the background, runs the tracking loop,
restores the background. On a pick, fills `*mdata`: `mn_tree`/`mn_menu` copied
from the input (**no submenus from popups — restriction #1 below**), `mn_item` =
the chosen object number, `mn_scroll` copied, `mn_keystate` = the shift state at
selection; returns `1`. On no pick, sets `mdata->mn_keystate = 0` and returns
`0` (`gemmnext.c:337-353`).

---

## `menu_attach` (37) — `CONF_WITH_MENU_EXTENSION`

**Source:** `gemsuper.c:192-194` → `aes/gemmnext.c:535-556` (`mn_attach`).
Helpers: `query_submenu`/`attach_submenu` (`gemmnext.c` — manage the `SMIB`
submenu-info array `gl_submenu`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` (`MPOP_FLAG`) | operation: `ME_INQUIRE`=0, `ME_ATTACH`=1, `ME_REMOVE`=2 (`aesdefs.h:297-299`) |
| `addr_in[0]` (`MPOP_IN`) | the main-menu-bar `OBJECT *` tree |
| `int_in[1]` (`MPOP_ITEM`) | the item to attach the submenu to |
| `addr_in[1]` (`MPOP_OUT`) | `MENU *` — submenu descriptor (in for attach, out for inquire) |
| `int_out[0]` | `1`/`TRUE` on success, `0`/`FALSE` on failure |

Attaches (1), removes (2), or queries (0) a submenu on a menu-bar item.
**Guard:** the target item must be a `G_STRING` (`tree[item].ob_type & 0xff ==
G_STRING`), else return `FALSE` immediately (`gemmnext.c:539-540`). Attaching
sets the item's `SUBMENU` (0x0800) `ob_flags` bit and writes a right-arrow
marker (0x03) two chars from the end of the item text (`gemmnext.c:521-528`).

> **EmuTOS restriction (`gemmnext.c:12-27`):** submenus can be attached **only
> to the main menu bar** — one level of submenus, no submenus of submenus, and
> no submenus from popups.

---

## `menu_istart` (38) — `CONF_WITH_MENU_EXTENSION`

**Source:** `gemsuper.c:195-197` → `aes/gemmnext.c:562-587` (`mn_istart`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` (`MPOP_FLAG`) | **0 = inquire**, **1 = update**; any other = `FALSE` |
| `addr_in[0]` (`MPOP_IN`) | the tree containing the submenu |
| `int_in[1]` (`MPOP_ITEM`) | the submenu box object |
| `int_in[2]` (`MPOP_ITEM2`) | the desired start item (update only) |
| `int_out[0]` | the submenu's **`s_start`** item after the operation, or **`FALSE` (0)** if no such submenu / bad flag |

Gets or sets the *starting* (top) item of a scrollable submenu. On update it
clamps `start` to the submenu box's `[ob_head, ob_tail]` child range and stores
it as `s_start`; on inquire it just returns the current `s_start`. Returns
`FALSE` (0) if the tree/menu has no attached submenu or the flag is neither 0
nor 1 (`gemmnext.c:566-587`).

---

## `menu_settings` (39) — `CONF_WITH_MENU_EXTENSION`

**Source:** `gemsuper.c:198-200` → `aes/gemmnext.c:596-618` (`mn_settings`).

| Slot | Meaning |
|------|---------|
| `int_in[0]` (`MPOP_FLAG`) | **0 = get** current values, **1 = set** new values |
| `addr_in[0]` (`MPOP_SET`) | pointer to a `MN_SET` struct (out for get, in for set) |
| `int_out[0]` | `1` (`mn_settings` is `void`; `crysbind` `ret` stays `TRUE`) |

Reads or writes the global submenu/scroll timing settings. `flag=0` copies the
current `settings` into `*set`; `flag=1` copies each **non-negative** field from
`*set` into `settings` (negative values are ignored, matching TOS), clamping
`height` to `[MIN_HEIGHT_VALUE=5, …]` (`gemmnext.c:600-616`).

> **Minimal implementation (`gemmnext.c:594`):** "none of these values are
> relevant" — EmuTOS stores the settings but its submenu code does not act on
> the delay/drag/speed values (the scroll flag is ignored, restriction #2). The
> **`MN_SET` field layout and the get/set-non-negative contract** hold; the
> timing *behaviour* those fields imply is not realised in this build.

---

## Trap interface (parameter-block form)

All `menu_*` calls enter via the GEM trap (`svc #2`) with **selector 200 in
`x0`** and an **AESPB pointer in `x1`** — see [`_dispatcher.md`](./_dispatcher.md).

| Op | Function | `int_in` | `addr_in` | `int_out[0]` return |
|---:|----------|----------|-----------|---------------------|
| 30 | `menu_bar`     | `[0]`=show(1/0) | `[0]`=tree | `1` |
| 31 | `menu_icheck`  | `[0]`=item, `[1]`=check(1/0) | `[0]`=tree | `1` |
| 32 | `menu_ienable` | `[0]`=item(+bit15 redraw), `[1]`=enable(1/0) | `[0]`=tree | `1` |
| 33 | `menu_tnormal` | `[0]`=title, `[1]`=normal(1/0) | `[0]`=tree | `1`/`0` (0 if title disabled) |
| 34 | `menu_text`    | `[0]`=item | `[0]`=tree, `[1]`=new text | `1` |
| 35 | `menu_register`| `[0]`=pid | `[0]`=title str | slot `0..5` / `-1` |
| 36 | `menu_popup`   | `[0]`=x, `[1]`=y | `[0]`=`MENU*` in, `[1]`=`MENU*` out | `1`/`0` *(ext.)* |
| 37 | `menu_attach`  | `[0]`=flag(0/1/2), `[1]`=item | `[0]`=tree, `[1]`=`MENU*` | `1`/`0` *(ext.)* |
| 38 | `menu_istart`  | `[0]`=flag(0/1), `[1]`=menu box, `[2]`=start | `[0]`=tree | `s_start` / `0` *(ext.)* |
| 39 | `menu_settings`| `[0]`=flag(0/1) | `[0]`=`MN_SET*` | `1` *(ext.)* |

---

## Inputs / Outputs summary

Inputs arrive in the AESPB arrays (no register/stack params). Outputs are
`int_out[0]` only; there are no `addr_out` results (the menu manager never uses
`addrout` — only `rsrc_gaddr` does, see [`_dispatcher.md`](./_dispatcher.md) §2).
`menu_popup`/`menu_attach` write their result **through the caller-supplied
`MENU *` in `addr_in[1]`/`addr_in[0]`**, not through `addr_out`. The
**`MN_SELECTED` message** (below) is delivered asynchronously through the
16-byte message pipe, *not* as a return of any `menu_*` call.

---

## `MN_SELECTED` message (the payload)

When the user picks an application menu item, the control manager posts a 16-byte
message to the owning application's pipe. Built by `ct_msgup` →
`ap_sendmsg(appl_msg, MN_SELECTED, owner, title, item, treehi, treelo, parent)`
(`gemctrl.c:353-368`, `gemwmlib.c:851-863`):

| `msg[]` word | Value | Meaning |
|:---:|-------|---------|
| `[0]` | `MN_SELECTED` = **10** (`aesdefs.h:227`) | message id |
| `[1]` | `rlr->p_pid` | sender (the AES/control pid) |
| `[2]` | `0` | extra-byte count |
| `[3]` | **`title`** | the **title** object index the item lives under |
| `[4]` | **`item`** | the selected **item** object index |
| `[5]` | `treehi` = `HIWORD(tree)` | menu-tree ptr, high WORD *(`CONF_WITH_MENU_EXTENSION` only; else 0)* |
| `[6]` | `treelo` = `LOWORD(tree)` | menu-tree ptr, low WORD *(ext.; else 0)* |
| `[7]` | `parent` = `get_par(tree,item)` | submenu parent object *(ext.; else 0)* |

> **Correction / completion vs. the previous spec.** The old file gave only
> `msg[3]=title, msg[4]=item` (correct) but omitted `msg[5..7]`. With
> `CONF_WITH_MENU_EXTENSION` (the default build) `msg[5]`/`msg[6]` carry the
> menu-tree pointer (hi/lo) and `msg[7]` the submenu parent object; without the
> extension all three are 0 (`gemctrl.c:347-368`). See D-051.

When the item picked is a **desk-accessory** entry (`title == THEDESK` and the
item is at/after `gl_dafirst`), the control manager instead sends **`AC_OPEN`**
(40) to the accessory owner, with the DA's menu id in `msg[4]` and `msg[5..7]=0`
(`gemctrl.c:337-349`). After acting on `MN_SELECTED`, the application usually
calls `menu_tnormal(tree, title, 1)` to un-highlight the title — and the control
manager itself already clears the title's `SELECTED` bit before/after sending
for the DA case (`gemctrl.c:341`).

---

## The drop-down tracking loop (reference)

The mouse tracking that turns a click on the bar into an `MN_SELECTED` message
lives in the **control manager**, not in the `menu_*` dispatch: `hctl_rect`
(`gemctrl.c:318-371`) calls `mn_do` (`gemmnlib.c:249` extension variant / `:518`
plain variant), a state machine (`START/INTITLE/INITEM/OUTSIDE/SUBMENU_STATE`)
that, while the button is held, uses `ob_find` to locate the title/item under
the cursor, `menu_down`/`menu_sr` to drop the box and save the screen area
beneath it (`bb_save`/`bb_restore`), and `do_chg(..., SELECTED, ...)` to hilite
the tracked item/title. On button release over an enabled item it returns the
`(title, item)` pair, which `hctl_rect` turns into the message above.

**This whole loop is reference** — NewTOS reimplements the tracking over the
object manager (hit-testing, `objc_draw`/`objc_change`) and the VDI raster
save/restore. What is **contract** is the *result*: the correct `(title, item)`
object indices delivered in the `MN_SELECTED` message, the title left highlighted
until `menu_tnormal`, and the screen restored under a closed drop-down.

---

## Error conditions

The menu manager has almost no error surface — like the rest of the AES it is
"trusted" code. Negative/false returns:

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `menu_register` with a negative pid | `gemmnlib.c:796,815` | `-1` |
| `menu_register` when all `NUM_ACCS` (6) slots are used | `gemmnlib.c:796,815` | `-1` |
| `menu_tnormal` on a **disabled** title (`chkdisabled=TRUE`) | `gemmnlib.c:158-159` | `0` (state unchanged) |
| `menu_ienable` — no error path (`chkdisabled=FALSE`) | `gemmnlib.c:172-176` | always `1` |
| `menu_attach` on a non-`G_STRING` item | `gemmnext.c:539-540` | `0`/`FALSE` |
| `menu_istart` with no attached submenu / bad flag | `gemmnext.c:566,585` | `0`/`FALSE` |
| `menu_popup` with no item chosen | `gemmnext.c:351-353` | `0` |
| any of 36–39 when `CONF_WITH_MENU_EXTENSION == 0` | `gemsuper.c:406-417` | `-1` (unsupported opcode) |

There are **no** TOS negative error codes (`E*`) in the menu manager — returns
are `1` / `0` / slot-id / `-1` only.

---

## Verification criteria

- **`menu_bar` install:** `menu_bar(tree, 1)` sets `gl_mntree = tree`, draws
  `THEBAR`, and returns `1`; `menu_bar(tree, 0)` clears `gl_mntree = NULL` and
  returns `1` (`gemmnlib.c:709-726`).
- **`menu_icheck` sets CHECKED (deferred draw):** `menu_icheck(tree, item, 1)`
  sets the item's `CHECKED` (0x0004) `ob_state` bit and returns `1`;
  `menu_icheck(tree, item, 0)` clears it. **No redraw happens** (`dodraw=FALSE`)
  — verify by state bit, not by pixels (`gemsuper.c:172`).
- **`menu_ienable` toggles DISABLED:** `menu_ienable(tree, item, 0)` sets
  `DISABLED` (0x0008); `menu_ienable(tree, item, 1)` clears it. With bit 15 of
  `item` set, the item is redrawn immediately; without it, deferred
  (`gemsuper.c:175-176`).
- **`menu_tnormal` highlight + disabled guard:** `menu_tnormal(tree, title, 0)`
  sets the title's `SELECTED` bit and redraws (returns `1`);
  `menu_tnormal(tree, title, 1)` clears it. On a `DISABLED` title it returns
  `0` and leaves the state unchanged (`gemmnlib.c:158-159`, `179`).
- **`menu_text` in-place replace:** after `menu_text(tree, item, "New")`, the
  buffer at `tree[item].ob_spec` reads `"New"`; the pointer is unchanged (same
  buffer overwritten) and no redraw occurs (`gemsuper.c:183`).
- **`menu_register` slot / exhaustion:** `menu_register(pid, str)` with a valid
  pid returns a slot `0..5` and records `str` in `g_acctitle[slot]`; the 7th
  concurrent registration (and any negative pid) returns `-1`
  (`gemmnlib.c:796-815`; `NUM_ACCS = 6`).
- **`MN_SELECTED` layout:** on a menu pick, the owner's pipe receives
  `msg[0]=10`, `msg[3]=title obj`, `msg[4]=item obj`; in the default
  (`CONF_WITH_MENU_EXTENSION`) build `msg[5]/msg[6]` = the menu-tree ptr hi/lo
  and `msg[7]` = the submenu parent (`gemctrl.c:353-368`).
- **`menu_popup` result:** `menu_popup(in, x, y, out)` returns `1` and fills
  `out->mn_item` with the chosen object when an item is picked; returns `0` and
  sets `out->mn_keystate = 0` otherwise (`gemmnext.c:337-353`).
- **`menu_attach` guard:** `menu_attach(ME_ATTACH, tree, item, m)` on an item
  whose `ob_type` is not `G_STRING` returns `0` (`gemmnext.c:539-540`).
- **Extension gating:** with `CONF_WITH_MENU_EXTENSION = 0`, opcodes 36–39
  return `-1` (`gemsuper.c:406-417`).

---

## Edge cases discovered in source

1. **Primary index is `int_in[0]`, state is `int_in[1]`.** All of `SHOW_IT`,
   `ITEM_NUM`, `TITLE_NUM`, `MM_PID` are `int_in[0]`; `CHECK_IT`, `ENABLE_IT`,
   `NORMAL_IT` are `int_in[1]` (`aesdefs.h:275-283`). (The previous spec had
   these one slot too high.)
2. **`menu_icheck` does not redraw.** `dodraw = FALSE` — the check mark only
   appears on the next drop-down draw (`gemsuper.c:172`). `menu_tnormal` *does*
   redraw (`dodraw = TRUE`); `menu_ienable` redraws only if item bit 15 is set.
3. **`menu_ienable`'s item bit 15 is the redraw flag, not a title marker.**
   `(ITEM_NUM & 0x8000) != 0` becomes `do_chg`'s `dodraw` argument; the object
   index is `ITEM_NUM & 0x7fff` (`gemsuper.c:175-176`).
4. **`menu_tnormal` silently fails on a disabled title.** `chkdisabled = TRUE`,
   so if the title already has `DISABLED` set, `do_chg` returns `0` and does not
   toggle `SELECTED` (`gemmnlib.c:158-159`). None of `icheck`/`ienable` check
   the disabled state (`chkdisabled = FALSE`).
5. **`menu_text` is an unbounded in-place `strcpy`.** No length check, no
   allocation, no redraw (`gemsuper.c:183`). A longer replacement string
   overruns the original `ob_spec` buffer — programs must keep it ≤ the original
   length and repaint themselves.
6. **`menu_register` stores the caller's string pointer, not a copy.**
   `g_acctitle[slot] = pstr` "like Atari TOS" (`gemmnlib.c:806`) — the title
   string must outlive the registration. The return is the slot index (0..5),
   which is the DA's *menu id* used later to route `AC_OPEN`.
7. **`MN_SELECTED` carries the menu-tree pointer in `msg[5..7]`** in the
   default build. Without `CONF_WITH_MENU_EXTENSION` those three words are 0
   (`gemctrl.c:347-368`). Selecting a desk-accessory entry sends **`AC_OPEN`
   (40)** instead of `MN_SELECTED`, with the DA menu id in `msg[4]`
   (`gemctrl.c:337-349`).
8. **`menu_bar(tree, 0)` ignores its `tree` argument** and erases the bar
   recorded in `gl_mntree` from the matching `show=1` (`gemmnlib.c:723-726`).
9. **The extension calls are deliberately minimal.** `menu_settings` stores its
   `MN_SET` fields but the submenu code ignores the timing/scroll values; only
   one level of submenus is supported, attached only to the main bar, and no
   popups-with-submenus (`gemmnext.c:12-27`, `594`).
10. **Menu selection runs in the control manager, not the `menu_*` trap.** The
    `menu_*` calls only set object state / install the tree; the actual pick →
    `MN_SELECTED` happens later inside `hctl_rect`/`mn_do` while the app is in
    `evnt_multi` (`gemctrl.c:318-371`).

---

## Dependencies

### Requires (must be initialized / functional)

- **The Object Manager** ([`object-form.md`](./object-form.md)) — the menu bar
  *is* an OBJECT tree; drawing (`ob_draw`), state change (`ob_change`), and
  hit-testing (`ob_find`) are how every menu operation renders. `menu_bar`,
  `do_chg`, and the tracking loop all call it.
- **The Event Manager** ([`event.md`](./event.md)) — `MN_SELECTED`/`AC_OPEN`
  are delivered through the 16-byte message pipe that `evnt_mesag`/`evnt_multi`
  read; the tracking loop runs on the mouse/button events the event manager
  feeds. The internal third mouse rectangle `MU_M3` (0x0100) belongs to this
  menu-extension code (see [`event.md`](./event.md) §config).
- **The VDI** raster save/restore (`bb_save`/`bb_restore`/`popup_blit`, the
  reference save-the-area-under-a-dropped-menu machinery) — a `vro_cpyfm`-class
  operation in NewTOS.
- **The AES process model / message pipe** ([`_state.md`](./_state.md)) —
  `gl_mnppd`/`rlr`, the DA owner descriptors `desk_ppd[]`, `ap_sendmsg`.
- **`menu_register` / the DA machinery** needs multiple AES processes (the
  accessories) — the same DA dependency `appl_exit`/`wait_for_accs` has
  ([`application.md`](./application.md)); absent at single-app bring-up.

### Used by

- **Every GEM application with a menu bar** calls `menu_bar` once at start-up
  and `menu_tnormal` after each `MN_SELECTED`; `menu_icheck`/`menu_ienable`/
  `menu_text` maintain item state.
- **The desktop** (`desk/`) owns the primary menu bar (this is the NewTOS
  desktop's bar).
- **Desk accessories** call `menu_register` to add their entry to the *Desk*
  menu.
- The **control manager** (`gemctrl.c`) drives the tracking and posts the
  messages; the **event manager** delivers them.

---

## Related data structures

- **Menu OBJECT tree** — a standard `OBJECT` tree (see
  [`object-form.md`](./object-form.md) and `docs/api/structures/` for the
  `OBJECT` layout) with the fixed index roles `THESCREEN`(0, root `G_IBOX`) /
  `THEBAR`(1) / `THEACTIVE`(2) / `THEDESK`(3) and the title(`G_TITLE`) →
  box(`G_BOX`) → item(`G_STRING`) hierarchy (`gemmnlib.c:46-48`,
  `gemctrl.c:42`). The `ob_state` bits `CHECKED`/`DISABLED`/`SELECTED` and the
  `ob_flags` bit `SUBMENU` (`obdefs.h:98-107`) are the contract fields the menu
  calls flip.
- **`MN_SELECTED` / `AC_OPEN` messages** — the 16-byte `WORD msg[8]` layout
  above (`gemctrl.c:353-368`, `aesdefs.h:227,242`). Shared with the event and
  application managers ([`event.md`](./event.md), [`_state.md`](./_state.md) §3).
- **`MENU`** (`aes/gemmnext.h:16-22`) — popup/submenu descriptor:
  `OBJECT *mn_tree`, `WORD mn_menu` (enclosing box obj), `WORD mn_item` (start
  item), `WORD mn_scroll`, `WORD mn_keystate`. Used by `menu_popup`/`menu_attach`.
- **`MN_SET`** (`aes/gemmnext.h:27-33`) — `menu_settings` block: `LONG display`
  (submenu-display delay ms), `LONG drag` (submenu-drag delay ms), `LONG delay`
  (single-click scroll delay ms), `LONG speed` (continuous-scroll delay ms),
  `WORD height` (default scroll height in items). Get/set with non-negative-only
  writes; `height` clamped ≥ 5.
- **DA-slot model** — `D.g_acctitle[NUM_ACCS]` (title ptrs), `desk_ppd[NUM_ACCS]`
  (owner descriptors), `D.g_accreg` (count), `acc_display[]` (display-slot →
  menu-id lookup), `gl_dafirst` (first DA object in the tree); `NUM_ACCS = 6`
  (`sysconf.h:37`). **Reference** — NewTOS reimplements the storage; the
  *observable* contract is the slot-id (0..5)/-1 return and the `AC_OPEN`
  routing.

---

## Cross-references

### Atari Compendium / Profibuch

- **Not present in `references/`** (no PDFs). Cross-check of the menu opcode
  numbers (30–39), the `int_in[0]`/`int_in[1]` slot assignment, the
  `menu_ienable` item-bit-15 redraw flag, the `MN_SELECTED` `msg[3..7]` layout,
  the `menu_register` slot-id/-1 return, and the `MENU`/`MN_SET` field layouts is
  **deferred** until the documentation PDFs are available. (Compendium documents
  `menu_ienable`'s high item bit and the `msg[5..7]` tree pointer for AES 3.3;
  this build matches — verification deferred. See D-051.)

### AES managers

- [`object-form.md`](./object-form.md) — the menu bar is an OBJECT tree;
  `objc_draw`/`objc_change`/`objc_find` do all drawing, state change, and
  hit-testing. `menu_text` writes `ob_spec` directly; repaint is via `objc_draw`.
- [`event.md`](./event.md) — `evnt_mesag`/`evnt_multi` deliver `MN_SELECTED`;
  the tracking loop consumes the same mouse/button events; `MU_M3` is the
  menu-extension internal rectangle.
- [`application.md`](./application.md) — the message pipe (`ap_sendmsg` →
  `ap_rdwr`) and the DA process model `menu_register` shares.
- [`_dispatcher.md`](./_dispatcher.md) — selector-200 call model, `control[]`
  counts, `int_out[0]` return, the `CONF_WITH_MENU_EXTENSION` gating of 36–39,
  the unsupported-opcode `-1` path.
- [`_state.md`](./_state.md) — `gl_mntree`/`gl_mnppd`, the DA slots, the 16-byte
  message pipe.

### VDI

- The save/restore of the screen area under a dropped menu / popup
  (`bb_save`/`bb_restore`/`popup_blit`) maps to a VDI raster copy
  (`vro_cpyfm`-class). Do not edit the VDI docs.

---

## Configuration / version notes

- **`CONF_WITH_MENU_EXTENSION`** (default **1**, `include/config.h:1305`) gates
  opcodes **36–39** and the whole of `aes/gemmnext.c`, plus the `msg[5..7]`
  tree-pointer/parent fields of `MN_SELECTED` (`gemctrl.c:347-368`) and the
  internal `MU_M3` mouse rectangle. With it off, 36–39 return `-1` and
  `MN_SELECTED` carries only `msg[3]/msg[4]`. This flag is also one of the four
  that push `AES_VERSION` to 0x0330/0x0340 (see
  [`application.md`](./application.md)).
- The extension implementation is deliberately **minimal** (`gemmnext.c:12-27`):
  submenus only on the main bar, one level deep, no popup-submenus, scroll flag
  ignored. Popups, one-level submenus, and `menu_settings` storage do work.
- Opcodes **30–35** are unconditional across all EmuTOS builds and all TOS
  versions (the original TOS 1.x menu set); 36–39 are AES 3.30 additions.
- `NUM_ACCS = 6` (`include/sysconf.h:37`) bounds `menu_register` to 6 DA slots;
  this is a build constant, not an ABI value a program should assume beyond
  "register may fail with -1".

---

## NewTOS implementation notes

(Empty during Phase 00.)

- **The menu manager is mostly the object manager + one tracking loop.** Because
  the bar **is** an OBJECT tree, `menu_icheck`/`menu_ienable`/`menu_tnormal`
  reduce to `objc_change` on one `ob_state` bit (respecting each call's
  redraw/disabled-guard contract), and `menu_text` to writing `ob_spec` +
  repaint. The **only genuinely new logic** is the drop-down mouse-tracking
  state machine that produces `MN_SELECTED` — reimplemented over NewTOS
  hit-testing + the event pipe, not ported from `mn_do`.
- **Bring-up order:** original 30–34 (bar + item state) first, `menu_register`
  and the extensions (36–39) later with the DA / submenu machinery — matching
  the [`application.md`](./application.md) DA-deferral. `menu_register` returning
  a real slot needs multiple AES processes; until then it is
  honest-unsupported (Rule 2), not a stub returning a fake slot.
- **Save/restore under a dropped menu** is a VDI raster op (`vro_cpyfm`); the
  top-of-screen bar occupies the system-font row height (`gl_hchar`/`gl_hbox`
  from `graf_handle`, see [`window-graphics.md`](./window-graphics.md)).
- **Pointers in `addr_in[]` (the tree, the text string, the `MENU`/`MN_SET`
  blocks) are user (application) pointers** — apply the same boundary handling
  as other AES `addr*` reads (see [`_dispatcher.md`](./_dispatcher.md) §NewTOS
  notes). The m68k-vs-native `OBJECT`/`MENU` pointer widths follow the caller
  ABI (see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md) "Native vs m68k
  struct variants").
- **`menu_text`'s in-place overwrite** must be preserved as contract (programs
  rely on same-or-shorter replacement into the same buffer); NewTOS should keep
  the same-buffer semantics but may bound the copy to the original `ob_spec`
  allocation to avoid the EmuTOS overrun — an ADR-worthy safety choice that does
  not change observable behaviour for well-formed callers.
