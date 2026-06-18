# AES Menu Manager (`menu_*`) — opcodes 30–39

> **PHASE-00b D8, Batch 5.** Source: EmuTOS @ `1a7484ab` —
> `aes/gemmnlib.c` (`mn_*`, `do_chg`), `aes/gemmnext.c` (popup/attach
> extensions), dispatch `aes/gemsuper.c`.
>
> Call convention: [`_dispatcher.md`](./_dispatcher.md).

## The menu is an OBJECT tree

A menu bar is just an **OBJECT tree** (Batch 3) with a fixed shape:
the root is a `G_IBOX`; its children are the **titles** (`G_TITLE`),
each title's child is a `G_BOX` drop-down holding the **items**
(`G_STRING`). The AES draws it, tracks the mouse over titles, drops the
box, highlights items, and on release posts an `MN_SELECTED` message
(Batch 2) carrying `[title obj, item obj]`. So most `menu_*` calls just
flip an object's `ob_state` and redraw — hence they route through the
shared `do_chg()` helper.

`do_chg(tree, obj, stateflag, set, redraw, isTitle)` sets/clears one
`ob_state` bit on one object and optionally redraws it.

## Functions

| Op | Name | Dispatch | Effect |
|---:|------|----------|--------|
| 30 | `menu_bar`    | `mn_bar(tree, show)` | install (`show`=1) / remove (`show`=0) the menu bar `tree` at the top of the screen |
| 31 | `menu_icheck` | `do_chg(tree, item, CHECKED, check, …)` | add/remove the ✓ check mark on an item |
| 32 | `menu_ienable`| `do_chg(tree, item&0x7fff, DISABLED, !enable, item&0x8000, …)` | enable/disable an item (greyed). Bit 15 of `item` flags a title-bar/special item |
| 33 | `menu_tnormal`| `do_chg(tree, title, SELECTED, !normal, TRUE, TRUE)` | draw a title normal (`normal`=1) or highlighted/inverted (0) — used to un-highlight after a selection |
| 34 | `menu_text`   | `strcpy(tree[item].ob_spec, ptext)` | replace an item's text string in place |
| 35 | `menu_register` | `mn_register(pid, str)` | register a **desk-accessory** menu slot; returns the accessory's menu id (or -1) |
| 36 | `menu_popup`  | `mn_popup(in, x, y, &out)` | display a **pop-up menu** at (x,y); fills `out` with the choice (ext., `CONF_WITH_MENU_EXTENSION`) |
| 37 | `menu_attach` | `mn_attach(flag, tree, item, &out)` | attach/detach/query a **sub-menu** on an item (ext.) |
| 38 | `menu_istart` | `mn_istart(flag, tree, item, item2)` | set the starting item of a scrollable/sub menu (ext.) |
| 39 | `menu_settings` | `mn_settings(flag, set)` | get/set menu behaviour (pop-up delay/scroll) (ext.) |

`intin` macros: `MM_ITREE`=addrin[0] (the tree), `ITEM_NUM`/`TITLE_NUM`=
intin[1], `CHECK_IT`/`ENABLE_IT`/`NORMAL_IT`=intin[2]; `MM_PTEXT`/`MM_PSTR`
=addrin[1]; `MM_PID`=intin[0]. Each returns 1 on success in `RET_CODE`.

The opcodes 36–39 are GEM 3 / EmuTOS extensions gated on
`CONF_WITH_MENU_EXTENSION`; 30–35 are the original TOS set.

## `MN_SELECTED` message (the payload)

When the user picks an item, the AES posts to the owner's pipe:
`msg[0]=MN_SELECTED, msg[3]=title object index, msg[4]=item object index`.
The application then acts and usually calls `menu_tnormal` to un-highlight
the title.

---

## NewTOS implementation notes

- Because the menu **is** an object tree, this manager is mostly
  `objc_change` (Batch 3) + the **menu-bar event tracking** loop (mouse
  over titles → drop box → highlight item → post `MN_SELECTED`). That
  tracking is the only genuinely new logic; the rest reuses `do_chg`.
- Depends on: Object Manager (draw/change), Event Manager (post message,
  read mouse), VDI (draw, save/restore the area under a dropped menu —
  the AES snapshots the screen rectangle under the menu box and restores
  it on close; needs a VDI raster save/`vro_cpyfm`).
- `menu_register` is the **desk-accessory** hook → depends on multiple AES
  processes; defer with the rest of the DA machinery (cf. Batch 1
  `wait_for_accs`). Bring-up: original 30–34 first, extensions + DA later.
- The top-of-screen menu bar occupies the system-font row height
  (`gl_hchar` from `graf_handle`, Batch 4).
