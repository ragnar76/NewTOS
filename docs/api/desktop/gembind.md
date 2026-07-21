# `desk/gembind.c` + `aesbind.h` — the desktop's AES/GEMDOS call surface

> Phase-00 extraction, Track A. Source: EmuTOS @ `1a7484ab`, `desk/gembind.c`,
> `desk/gembind.h`, `desk/aesbind.h`, `desk/deskbind.h`, `include/gemdos.h`,
> `include/bdosbind.h`. **New doc, written during this validation pass**
> (2026-07-21) because none of the existing `docs/api/desktop/*.md` files
> document this layer — `_overview.md` previously marked it "out of scope —
> already covered by `docs/api/aes/`," which is **incomplete**: `docs/api/
> aes/` documents the AES's own per-function *contract* (what `wind_get`
> means), not *which of those functions the desktop actually calls, how
> often, from where, and in what order* — the inventory a 1:1, call-for-call
> EmuTOS-faithful rebuild needs. This doc is that inventory. Scope: **every**
> AES call the whole `desk/` directory makes (not just this task's
> `deskapp.c`/`deskobj.c`/`deskglob.c` chunk), because `gembind.c`/`aesbind.h`
> are directory-wide binding files with no per-caller split — but the doc
> flags exactly which calls originate in *this task's* three files.

## Why this exists at all: two real trap boundaries, not one monolith

EmuTOS's `desk/` is linked into the same ROM image as the AES/BIOS/GEMDOS,
but it does **not** call AES-internal functions as plain C calls. It goes
through two genuine trap-based binding layers, both implemented the same way
(software trap + shared parameter block):

- **AES calls** — `desk/gembind.c` composes a `GEMBLK` (six array/pointer
  fields: control, global, int_in, int_out, addr_in, addr_out,
  `gembind.c:53-70`) and executes `trap #2` with `d0=200` (`gem()`,
  `gembind.c:72-87`). The opcode/parameter-count header is packed into
  `control[]` by `gem_if0()`/`AES_CTRL_CODE()` (`gembind.c:45-51,89-97`).
  This is **exactly** the same mechanism a real, out-of-process GEM
  application uses — the desktop is, mechanically, an ordinary AES client.
- **GEMDOS calls** — `include/gemdos.h`'s `dos_*` inline wrappers
  (e.g. `dos_sfirst` → `Fsfirst`, `gemdos.h:60-63`) call macros in
  `include/bdosbind.h` that expand to `trap1(0x4e, ...)` etc.
  (`bdosbind.h:19,51`) — a **real** `trap #1`, the same GEMDOS entry point
  any TOS program uses (`bdosbind.h:19`: `extern long trap1(int, ...); /*
  Not reentrant! Do not call for Pexec() */`).

**Both are real traps**, not shortcuts — this is the load-bearing fact for
"go through the AES/GEMDOS exactly like EmuDesk does." The one **exception**
found this pass (see "Not everything is a trap call" below) is `gsx_trans()`,
which is a direct C call into `aes/gemgraf.c` — possible only because EmuTOS
links `desk/` and `aes/` into one binary; a genuinely separate NewTOS desktop
process cannot do this the same way.

## The full `aesbind.h` catalogue vs. what `desk/` actually calls

`aesbind.h` declares every AES entry point `gembind.c` can bind; several are
commented out (`/* unused */`) in **both** files, meaning **no code path in
this EmuTOS checkout can call them** — they are not merely rare, they are
dead in `desk/`. The table below lists every **live** (non-commented-out)
declaration, whether `desk/*.c` (any file, not just this task's three)
actually calls it, and its `tos-api` counterpart.

| AES function | Live in `aesbind.h`? | Called by `desk/*.c`? (file, count) | `tos-api` server fn |
|---|---|---|---|
| `appl_init` | yes | `deskmain.c` (1) | `tos-api/src/aes/appl.rs::appl_init` |
| `appl_exit` | yes | `deskmain.c` (1) | `appl.rs::appl_exit` |
| `appl_write`/`appl_read`/`appl_find`/`appl_tplay`/`appl_trecord` | **no** (`/* unused */`, `aesbind.h:23-27`) | never | present in `tos-api` but unused by `desk/` |
| `evnt_button` | yes | `deskact.c`, `deskmain.c` (3) | `event.rs::evnt_button` |
| `evnt_keybd`/`evnt_mouse`/`evnt_mesag` | **no** (`aesbind.h:43-47`) | never (keyboard/mouse/message are read as *fields of `evnt_multi`'s output*, not separate calls) | present, unused by `desk/` |
| `evnt_timer` | yes | `deskmain.c` (1) | `event.rs::evnt_timer` |
| `evnt_multi` | yes | `deskact.c`, `deskdir.c`, `deskinf.c`, `deskmain.c` (7) | `event.rs::evnt_multi` |
| `evnt_dclick` | yes | `deskinf.c`, `deskmain.c` (3) | `event.rs::evnt_dclick` |
| `menu_bar` | yes | `deskmain.c`, `desksupp.c` (4) | `menu.rs::menu_bar` |
| `menu_icheck` | yes | `deskmain.c` (15) | `menu.rs::menu_icheck` |
| `menu_ienable` | yes | `deskmain.c` (13) | `menu.rs::menu_ienable` |
| `menu_tnormal` | yes | `deskmain.c` (3) | `menu.rs::menu_tnormal` |
| `menu_text` | declared, **but shadowed** — see below | **0 real calls** | `menu.rs::menu_text` (present, unused by `desk/`) |
| `menu_register` | **no** (`aesbind.h:59`, `/* unused */`) | **0** (grepped `desk/*.c`, no hit outside the comment) | present, unused by `desk/` |
| `objc_add` | yes | `deskobj.c` (3; **this task's chunk**) | `object.rs::objc_add` |
| `objc_delete` | **no** (`aesbind.h:66`) | never | present, unused |
| `objc_draw` | yes | `deskdir.c`, `deskinf.c`, `desksupp.c` (4) | `object.rs::objc_draw` |
| `objc_find` | yes | `deskact.c` (1) | `object.rs::objc_find` |
| `objc_order` | yes | `deskwin.c` (2, incl. `win_top()`, see below) | `object.rs::objc_order` |
| `objc_offset` | yes | `deskdir.c` (1) | `object.rs::objc_offset` |
| `objc_edit`/`objc_change` | **no** (`aesbind.h:73-76`) | never | present, unused |
| `form_do` | yes | `deskdir.c`, `deskfun.c`, `deskinf.c`, `deskmain.c`, `deskins.c`, `desksupp.c` (11) | `form.rs::form_do` |
| `form_dial` | yes | `deskdir.c`, `deskinf.c`, `desksupp.c` (6) | `form.rs::form_dial` |
| `form_alert` | yes | `deskfun.c`, `deskmain.c` (3) | `form.rs::form_alert` |
| `form_error` | yes, gated `#if CONF_WITH_SHOW_FILE \|\| CONF_WITH_PRINTER_ICON` (`aesbind.h:86-88`) | `desksupp.c` (2) | `form.rs::form_error` |
| `form_center` | yes | `deskdir.c`, `deskinf.c` (4) | `form.rs::form_center` |
| `form_keybd`/`form_button` | **no** (`aesbind.h:90-93`) | never | present, unused |
| `graf_rubbox` | yes | `deskact.c` (1) | `graf.rs::graf_rubberbox` (renamed) |
| `graf_dragbox`/`graf_mbox` | **no** (`aesbind.h:103-105`) | never | present, unused |
| `graf_growbox_grect` | yes | `desksupp.c` (2) | `graf.rs::graf_growbox` |
| `graf_shrinkbox_grect` | yes | `deskfun.c`, `desksupp.c` (3) | `graf.rs::graf_shrinkbox` |
| `graf_watchbox`/`graf_slidebox` | **no** (`aesbind.h:110-111`) | never | present, unused |
| `graf_handle` | yes | `deskmain.c` (1) | `graf.rs::graf_handle` |
| `graf_mouse` | yes | `deskact.c`, `desksupp.c` (14) | `graf.rs::graf_mouse` |
| `graf_mkstate` | yes | `deskact.c`, `deskmain.c`, `desksupp.c` (3) | `graf.rs::graf_mkstate` |
| `scrp_read`/`scrp_write` | **no** (`aesbind.h:122-123`) | never | present, unused |
| `fsel_input` | **no** (`aesbind.h:130-132`) | never | present, unused |
| `fsel_exinput` | yes, gated `#if CONF_WITH_DESKTOP_SHORTCUTS \|\| CONF_WITH_READ_INF` (`aesbind.h:133-135`) | `deskapp.c` (1; **this task's chunk**), `desksupp.c` (1) | `fsel.rs::fsel_exinput` |
| `wind_create_grect` | yes | `deskwin.c` (1) | `window.rs::wind_create` |
| `wind_open_grect` | yes | `desksupp.c` (1) | `window.rs::wind_open` |
| `wind_close` | yes | `deskfun.c`, `deskmain.c` (2) | `window.rs::wind_close` |
| `wind_delete` | yes | `deskwin.c` (1) | `window.rs::wind_delete` |
| `wind_get`/`wind_get_grect` | yes | `deskfun.c`, `deskmain.c`, `deskwin.c`, `desksupp.c` (real call sites — **not** `deskobj.c`, see caveat below) | `window.rs::wind_get` |
| `wind_set`/`wind_set_grect` | yes | `deskmain.c`, `deskwin.c`, `desksupp.c` (13) | `window.rs::wind_set` |
| `wind_find` | yes | `deskact.c`, `deskmain.c` (2) | `window.rs::wind_find` |
| `wind_update` | yes | `deskmain.c`, `desksupp.c` (10) | `window.rs::wind_update` |
| `wind_calc_grect` | yes | `deskwin.c` (1) | `window.rs::wind_calc` |
| `wind_new` | yes — declared unconditionally in `aesbind.h:152`, but its **implementation** in `gembind.c:827-832` is gated `#if CONF_WITH_READ_INF` (the header prototype itself is not gated; only compiled call sites under the same `#if`, e.g. `deskmain.c:243` et al., avoid a link error) | `deskmain.c` (1) | `window.rs::wind_new` |
| `rsrc_load` | yes | `deskapp.c` (1; **this task's chunk**, `load_icon_file()`) | `resource.rs::rsrc_load` |
| `rsrc_free` | yes | `deskapp.c` (3; **this task's chunk**) | `resource.rs::rsrc_free` |
| `rsrc_gaddr`/`rsrc_saddr`/`rsrc_rcfix` | **no** (`aesbind.h:161-162`) | never | present, unused |
| `rsrc_obfix` | yes | `deskmain.c` (1) | `resource.rs::rsrc_obfix` |
| `shel_read` | **no** (`aesbind.h:170`) | never | present, unused |
| `shel_write` | yes | `deskmain.c`, `deskpro.c`, `desksupp.c` (7) | `shell.rs::shel_write` |
| `shel_get` | yes | `deskapp.c` (1; **this task's chunk**, `app_start()`) | `shell.rs::shel_get` |
| `shel_put` | yes | `deskapp.c` (2; **this task's chunk**) | `shell.rs::shel_put` |
| `shel_find`/`shel_envrn`/`shel_rdef` | **no** (`aesbind.h:175-177`) | never | present, unused |
| `shel_wdef` | yes, gated `#if WITH_CLI` (`aesbind.h:179-181`) | `deskmain.c` (2) | `shell.rs::shel_wdef` |

Counts are from `grep -c` across `desk/*.c` excluding `gembind.c` itself
(which also contains commented-out reference implementations that would
otherwise inflate the count).

### `menu_text` is a false positive — EmuTOS's own desktop bypasses the AES call

`deskmain.c:72`:
```c
#define menu_text(tree,inum,ptext) (((tree)+(inum))->ob_spec = (LONG)(ptext))
```
This **local macro** shadows the AES binding of the same name for the rest of
`deskmain.c`'s translation unit — every apparent "`menu_text()` call" in
`deskmain.c` is actually a direct `ob_spec` field write on the desktop's own
menu `OBJECT` tree, **not a trap**. This is a deliberate optimisation (the
desktop owns the menu tree outright, so there's no reason to round-trip
through the AES to change one `LONG` field) and is the **same category of
finding** as `kernel/src/desk/menu.rs`'s own documented decision to bypass
`menu_icheck`/`menu_ienable` (`menu.rs:12-23`) — except EmuTOS itself only
applies this shortcut to `menu_text`, and (per the table above) **does** use
the real `menu_icheck`/`menu_ienable` traps 15+13 times from `men_update()`.
**Cross-check for the lead:** `kernel/src/desk/menu.rs`'s bypass of
`menu_icheck`/`menu_ienable` is a **wider** deviation from EmuTOS's own call
pattern than EmuTOS's own `menu_text` shortcut — EmuTOS really does call the
AES for icheck/ienable; NewTOS's `menu.rs` comment gives its own reasoning
(a `tos-api` `object::change` redraw-timing issue) for going further. Not a
defect — just flagged so the "1:1 through the AES" charter's one known
exception is precisely scoped, not conflated with EmuTOS's own (narrower)
`menu_text` shortcut.

### Not everything is a trap call: `gsx_trans` is a direct AES-internal call

`setup_iconblks()` (`deskapp.c:526-529`, **this task's chunk**) and
`app_tran()` (`deskapp.c:445`) call `gsx_trans(addr, wb, h)` to transform a
mono icon bitmap from "standard form" to the device-specific form. `gsx_trans`
is defined in `aes/gemgraf.c:369-376` — **not** declared in `aesbind.h`, and
**not** reached via `trap #2`. It is a plain, direct C function call into the
AES's own internals, possible only because `desk/` and `aes/` are compiled
into one link unit (a monolithic ROM image). **This is a genuine dependency a
1:1, separate-process NewTOS desktop cannot satisfy the same way**: either
(a) the desktop reimplements the mono→device bitmap transform itself (it is
a small, self-contained routine — `gsx_trans` ultimately calls `vrn_trnfm`,
not traced this pass), or (b) a new AES opcode is added to expose it. This is
a **decision for the lead/Developer**, not resolved here — flagged per the
"flag, don't fabricate" rule. Contrast with `r_set`/`r_get`: an **earlier
version of `deskapp.md` mis-cited these as living in `aes/gemwrect.c`**
(corrected in this pass, see `deskapp.md`) — they are trivial `static
__inline__` field-copy helpers in the **shared** header `include/obdefs.h:
258-272`, included by both `desk/` and `aes/`; calling them is not a
dependency on AES internals at all, and they cost nothing to reimplement.

### The `win_top()` → `objc_order` finding (belongs to `deskwin.md`, noted here for completeness)

`win_top()` (`deskwin.c:218-240`, out of this task's file scope but cited by
this task's `_data-model.md` for `g_wfirst`) calls **`objc_order(G.g_screen,
thewin->w_root, NIL)`** (`deskwin.c:222`) to move the window's `OBJECT` to
the front of the tree's own draw/hit-test order — a real AES call this task's
`_data-model.md` did not previously mention against the `g_wfirst` row. Added
there as a citation; full narrative is `deskwin.md`'s territory.

## GEMDOS call surface, scoped to this task's three files

`deskapp.c`/`deskobj.c`/`deskglob.c` only (the full-`desk/` GEMDOS surface —
`deskfun.c`/`deskdir.c`'s file-copy/move/delete calls — is out of scope for
this task; see `_overview.md`'s directory map for which doc owns it, if any):

| `dos_*` wrapper (`gemdos.h`) | Real GEMDOS call (`bdosbind.h`) | Call sites in this task's files |
|---|---|---|
| `dos_alloc_anyram` | `Malloc`/`Mxalloc` (`bdosbind.h:45,47`, both wrap the same trap family) | `deskobj.c:86`, `deskapp.c:476,618,629,765,1511` |
| `dos_avail_anyram` | (no single GEMDOS opcode — computed by BIOS-side memory-info helpers, not traced) | `deskobj.c:76` |
| `dos_free` | `Mfree` (`bdosbind.h:48`) | `deskapp.c:1013,1299,1545` |
| `dos_load_file` | wraps `Fopen`/`Fread`/`Fclose` (not a single opcode; helper in `bios/`, not traced this pass) | `deskapp.c:688,1520` |
| `dos_write` | `Fwrite` (`bdosbind.h:41`) | `deskapp.c:1058` |
| `dos_sfirst` | `Fsfirst` (`bdosbind.h:51`) | `deskapp.c:553` |
| `dos_sdrv` | `Dsetdrv` (`bdosbind.h:26`) | `deskapp.c:714` |
| `dos_gdrv` | `Dgetdrv` (`bdosbind.h:30`) | `deskapp.c:714` |
| `dos_create` | `Fcreate` (`bdosbind.h:37`) | `deskapp.c:1054` |
| `dos_close` | `Fclose` (`bdosbind.h:39`) | `deskapp.c:1059` |

All of these go through `trap1()` (`util/miscasm.S`, not read this pass) —
the same real GEMDOS trap entry any TOS program uses. **No GEMDOS shortcut
analogous to `gsx_trans`/`menu_text` was found in this task's three files.**

## Cross-reference to `kernel/src/desk/mod.rs` (context, not validated against source — code, not spec)

For the lead's synthesis only (this is a code observation, not a spec claim —
no EmuTOS citation applies to NewTOS's own Rust): `kernel/src/desk/mod.rs`
**already implements** a real client-side "aesbind" (`svc #2`, selector 200
in `x0`, AESPB in `x1` — the same mechanical pattern as `gembind.c`'s
`trap #2`/`d0=200`), with wrapper functions for `evnt_button`, `evnt_multi`,
`menu_bar`, `menu_tnormal`, `objc_draw`, `objc_find`, `form_do`, `form_alert`,
`graf_rubberbox`, `graf_dragbox`, `graf_mouse`, `graf_mkstate`,
`wind_create/open/close/delete/get/set/find/calc`. Per the table above, this
**does not yet cover**: `appl_init`/`appl_exit`, `menu_icheck`/`menu_ienable`
(deliberately bypassed per `menu.rs:12-23`, see above), `objc_add`/
`objc_offset`/`objc_order`, `form_dial`/`form_center`/`form_error`,
`graf_growbox`/`graf_shrinkbox`/`graf_handle`, `fsel_exinput`, `wind_new`/
`wind_update`, `rsrc_load`/`rsrc_free`/`rsrc_obfix`, `shel_write`/`shel_get`/
`shel_put`/`shel_wdef`. Some of these belong to `desk/` modules outside this
task's scope (`dir.rs`/`inf.rs`/`insapp.rs`/`rez.rs` in the NewTOS layout);
whether they are already wired there was **not checked** (out of this
docs-only task's scope) — flagged so the lead can confirm coverage against
the table above before treating any of #5-#9 as call-surface-complete.

## 1:1-readiness assessment

**Ready**, with two explicit carve-outs the Developer must decide, not
infer:
1. `gsx_trans`'s direct-call dependency (reimplement vs. new opcode).
2. Whether NewTOS's `menu_icheck`/`menu_ienable` bypass (already a considered,
   documented decision in `menu.rs`) is accepted as a deliberate, wider
   deviation from EmuTOS's own call pattern, now that this doc shows EmuTOS
   itself does call those two traps from `men_update()`.

Everything else in the table is a straight, mechanically-verifiable
function-for-function correspondence between `gembind.c`'s catalogue and
`tos-api/src/aes/*`'s server-side opcodes — no gaps found on the AES-server
side; the open question is 100% about the desktop-side client wiring
(`kernel/src/desk/*.rs`), which is a code-completeness question, not a spec
gap.
