# AES Event Manager (`evnt_*`) — opcodes 20–26

> **PHASE-00b D8, Batch 2 (full-contract refinement).**
> Source: EmuTOS @ `1a7484ab`.
> Primary: `aes/gemevlib.c` (`ev_*`); dispatch arm `aes/gemsuper.c:125-165`
> (`crysbind`); event-flag + slot macros `include/aesdefs.h`; input helpers
> `aes/geminput.c` (`in_mrect`/`downorup`/`b_click`/`bchange`) and the key/fork
> pre-check `aes/gemdisp.c` (`chkkbd`/`forker`); the message pipe is shared with
> the application manager (`aes/gemaplib.c:ap_rdwr`, `aes/gemqueue.c`).
>
> Call convention + AESPB layout: [`_dispatcher.md`](./_dispatcher.md).
> Shared AES globals (message pipe, `rlr`, mouse/button/keyboard state):
> [`_state.md`](./_state.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** opcode numbers (20–26); the `MU_*` event-flag bit values; the
> `int_in`/`int_out`/`addr_in` slot layout of every call (especially the
> `evnt_multi` slot map and the `int_out[1..6]` return block shared by all
> waits); the `MOBLK`/`GRECT` 5-WORD rectangle layout; the enter/leave (`m_out`)
> rectangle semantics; the button clicks/mask/state packing; the 16-byte standard
> message; the double-click-rate speed range (0..4) and its table; the
> `EV_KRET` scancode|ascii key encoding; the multi-event "which fired" bitmask
> return. **Reference (NewTOS reimplements freely):** `ev_block`/`iasync`/`mwait`/
> `apret`/`acancel`, the `EVB`/`QPB` event blocks, the fork queue (`forker`/
> `forkq`), the `gl_mowner`/`p_cda` cooperative-scheduler internals, and the
> `xrat`/`yrat`/`button`/`mtrans`/`pr_*` global mirrors — these are the EmuTOS
> cooperative-multitasking mechanics NewTOS drives from its own USB-HID/IKBD
> input path + per-process message pipe.

---

## Event-type flags (`MU_*`, `include/aesdefs.h:256-263`)

The bit mask that selects which events a wait blocks on. Passed in
`int_in[0]` (`MU_FLAGS`) of `evnt_multi`; each single-event call is a fixed
one-flag wait.

| Flag | Value | Event |
|------|------:|-------|
| `MU_KEYBD`    | 0x0001 | a key was pressed |
| `MU_BUTTON`   | 0x0002 | the mouse-button state matched the requested clicks/mask/state |
| `MU_M1`       | 0x0004 | mouse entered/left rectangle 1 |
| `MU_M2`       | 0x0008 | mouse entered/left rectangle 2 |
| `MU_MESAG`    | 0x0010 | a message arrived in the caller's pipe |
| `MU_TIMER`    | 0x0020 | the timer expired |
| `MU_TOSVALID` | 0x003F | mask of all six valid TOS bits (`KEYBD..TIMER`) |
| `MU_M3`       | 0x0100 | **internal only** — rectangle 3, `CONF_WITH_MENU_EXTENSION` (see below) |

> **Correction vs. previous spec:** the old table gave `MU_TOSVALID = 0x3F`
> as "all of the above" and omitted `MU_M3 = 0x0100`. `MU_TOSVALID` is exactly
> **0x003F** (the six standard bits) and is used to **mask off** any extra bits
> a caller sets before `evnt_multi` reaches the library — so a user program
> **cannot** request `MU_M3` (`gemsuper.c:153`, `ev_multi(MU_FLAGS & MU_TOSVALID, …)`).
> `MU_M3` (0x0100) is reserved for the menu-extension code's internal third
> rectangle. Two further internal-use bits exist but are never valid event
> selectors: `MU_SDMSG = 0x0040`, `MU_MUTEX = 0x0080` (`aes/gemlib.h:25-26`) —
> the "send" and "mutex" pseudo-events used by `appl_write`/internal locking,
> not by `evnt_*`.

---

## `MOBLK` — the mouse rectangle (M1/M2, `aes/gemlib.h:28-32`)

`MOBLK { BOOL m_out; GRECT m_gr; }` where `GRECT { WORD g_x, g_y, g_w, g_h; }`
(`include/obdefs.h:155-162`). As it arrives **on the wire** in the AES
parameter block it is **five consecutive WORDs**, cast in place from the
`int_in[]` slots (`(MOBLK *)&MO_FLAGS`, `gemsuper.c:133`):

| Wire WORD | Field | Meaning |
|:---------:|-------|---------|
| 0 | `m_out` (flags) | **0 = fire when the mouse ENTERS** the rect; **1 = fire when it LEAVES** (is *outside*) it |
| 1 | `g_x` | rectangle x |
| 2 | `g_y` | rectangle y |
| 3 | `g_w` | rectangle width |
| 4 | `g_h` | rectangle height |

The condition tested is `in_mrect` (`geminput.c:76-79`):
`m_out != inside(xrat, yrat, &m_gr)` — i.e. the event fires when the mouse's
inside-ness differs from `m_out`. With `m_out = 0` the wait ends when the
cursor is inside the rectangle; with `m_out = 1` when it is outside.

> **Layout note (native vs. wire).** The C `struct moblk` uses `BOOL m_out`
> (`BOOL = int`, 4 bytes on the host — `include/portab.h:120`), so the *native*
> `sizeof(MOBLK)` is 12, not 10. But the AES ABI passes the rectangle as the
> 5-WORD `int_in` slice above (`MO_FLAGS..MO_HEIGHT` / `MMO1_*` / `MMO2_*`,
> `aesdefs.h:182-216`); only the low WORD of `m_out` carries the flag and the
> in-place cast lines the `GRECT` up with `int_in[1..4]`. The **contract is the
> 5-WORD slot layout**; the wider native struct is an EmuTOS host detail. NewTOS
> reads five WORDs per rectangle from the parameter block.

---

## `evnt_multi` (25) — the central multiplexer

**Source:** `gemsuper.c:145-162` (`EVNT_MULTI` arm) → `ev_multi()`
(`gemevlib.c:151-294`). The heart of every GEM event loop: block until **any**
of the requested events fires, return the bitmask of which one(s) did, and
report the mouse / keyboard / button state for all of them in one shot.

### Parameter-block interface

`int_in` layout (`aesdefs.h:193-224`):

| `int_in` | Macro | Meaning |
|:--------:|-------|---------|
| 0 | `MU_FLAGS` | event mask (`MU_*` bits; masked with `MU_TOSVALID` = 0x3F) |
| 1 | `MB_CLICKS` | button: **low byte** = number of clicks to wait for, **high byte** = enter(0)/leave(1) flag (`bclicks = LOBYTE(HIWORD(p))`, `geminput.c:559`; `flag = (buparm>>24)&0xff`, `:348-357`) |
| 2 | `MB_MASK` | button: which buttons to test (bitmask; byte) |
| 3 | `MB_STATE` | button: desired button state (byte) |
| 4 | `MMO1_FLAGS` | rect 1 `m_out` (0 enter / 1 leave) |
| 5..8 | `MMO1_X/Y/WIDTH/HEIGHT` | rect 1 `GRECT` |
| 9 | `MMO2_FLAGS` | rect 2 `m_out` |
| 10..13 | `MMO2_X/Y/WIDTH/HEIGHT` | rect 2 `GRECT` |
| 14 | `MT_LOCOUNT` | timer count, low WORD (milliseconds) |
| 15 | `MT_HICOUNT` | timer count, high WORD |

| `addr_in` | Macro | Meaning |
|:---------:|-------|---------|
| 0 | `MME_PBUFF` | pointer to the caller's 16-byte message buffer (`WORD msg[8]`) |

`int_out` layout (`aesdefs.h:194-199`) — the **shared return block** (also used
by the single-event waits, which fill `int_out[1..4]` via `&EV_MX`):

| `int_out` | Macro | Meaning |
|:---------:|-------|---------|
| 0 | `RET_CODE` | **which events fired** (a `MU_*` bitmask; can be several at once) |
| 1 | `EV_MX` | mouse x |
| 2 | `EV_MY` | mouse y |
| 3 | `EV_MB` | mouse button state (bit set = down) |
| 4 | `EV_KS` | keyboard shift state (as from `vq_key_s()`/`gsx_kstate`) |
| 5 | `EV_KRET` | key code: **scancode in the high byte, ASCII in the low byte** (only if `MU_KEYBD` fired and a char was ready) |
| 6 | `EV_BRET` | mouse-button click count (only if `MU_BUTTON` fired) |

> **Correction vs. previous spec.** The old `evnt_multi` `int_in` table was
> **off by one for the rectangles and the timer** (it placed the M1 rect at
> `int_in[4..8]` correctly but listed the M2 rect at `9–13` and the timer at
> `14/15` in a way that conflated `flags,x,y,w,h`, and named `int_in[1..3]` only
> as generic "button clicks/mask/state"). The exact macros are `MB_CLICKS =
> int_in[1]`, `MB_MASK = int_in[2]`, `MB_STATE = int_in[3]`, `MMO1_* =
> int_in[4..8]`, `MMO2_* = int_in[9..13]`, `MT_LOCOUNT = int_in[14]`,
> `MT_HICOUNT = int_in[15]` (`aesdefs.h:202-221`). The **`int_out` return names**
> are `EV_MX/EV_MY/EV_MB/EV_KS/EV_KRET/EV_BRET` = `int_out[1..6]`
> (`aesdefs.h:194-199`) — note the last slot is **`EV_BRET` (button click
> count), not "mouse click count"**, and the requested `EV_MOUT`/`EV_BUTTON`/
> `EV_KRET`/`EV_MMGPBUFF` names from older docs do **not** exist in this EmuTOS
> build (only `EV_KRET` matches). The old table's slot *meanings* for
> `int_out[3..6]` were right; the input-side rectangle/timer/button mapping and
> the `int_out[1..2]` mouse-position slots are the fixed corrections.

### Behaviour

The dispatcher arm (`gemsuper.c:146-162`): if `MU_MESAG` is requested it sets
`rlr->p_flags |= AP_MESAG`; if `MU_TIMER` is requested it builds the 32-bit
millisecond count from `MT_HICOUNT`/`MT_LOCOUNT` via `MAKE_ULONG`; it packs the
button spec with `combine_cms(MB_CLICKS, MB_MASK, MB_STATE)`; then calls
`ev_multi(MU_FLAGS & MU_TOSVALID, rect1, rect2, [rect3=NULL,] count, buparm,
MME_PBUFF, &EV_MX)`. After it returns, if the fired set includes `MU_MESAG`
and the delivered `msg[0] == AC_CLOSE` it also sets `AP_ACCLOSE`.

`ev_multi` itself (`gemevlib.c:151-294`):

1. **Pre-check + drain forks.** `chkkbd()` samples the keyboard (queues a
   `kchange` fork if a key or a shift-state change is pending); `forker()`
   flushes the fork queue so any already-pending button/mouse/key changes are
   applied before the quick checks (`:169-170`).
2. **Quick-check keyboard** (`MU_KEYBD`): if a character is queued on the
   caller's console queue (`rlr->p_cda->c_q`), dequeue it into `EV_KRET`
   (`prets[4]`) and mark `MU_KEYBD` fired (`:173-182`).
3. **Quick-check button / rectangles** (only if the caller owns the mouse,
   `rlr == gl_mowner`): `MU_BUTTON` uses `downorup(button, buparm)` on the
   current (or, if a transition is pending, the transition-time `pr_button`)
   state, and reports the click count in `EV_BRET` (`prets[5]`); `MU_M1`/`MU_M2`
   use `in_mrect` (`:184-213`).

   > **This gate is quick-check-only, and `gl_mowner` itself is frozen during
   > a control-manager track.** `rlr == gl_mowner` above decides only whether
   > this *non-blocking* check runs at all — it says nothing about the later
   > blocking arm (see the note after point 6). Separately, ownership does not
   > move by mouse-down while the control manager is tracking the menu bar:
   > `bchange`'s reassignment of `gl_mowner` is wrapped in `if (!gl_ctmown &&
   > (new == MB_DOWN) && (button == 0)) gl_mowner = chkown();`
   > (`aes/geminput.c:334-335`), and `gl_ctmown` is set for the whole of
   > `mn_do`'s tracking loop (`ct_mouse(TRUE)`/`ct_mouse(FALSE)`,
   > `aes/gemmnlib.c:277,513`; `aes/gemctrl.c:400-417`). A fresh button-down
   > landing anywhere on screen during that bracket does **not** hand the
   > mouse to the process under the cursor — it stays with `ctl_pd`. Likewise
   > `set_mown` (called from `w_setactive`'s `ct_chgown`, `aes/gemctrl.c:392-397`
   > at the head of the control-manager loop, `aes/gemctrl.c:441-442`) is a
   > no-op while a button is physically held (`if (!button) { … }`,
   > `aes/geminput.c:222-235`), so ownership cannot pass to an application
   > mid-click either. See [`menu.md`](./menu.md) "The drop-down tracking
   > loop" for the resulting **contract**: the click that selects a bar item
   > is never delivered to the application as `MU_BUTTON`.
4. **Quick-check timer:** if `tmcount == 0` the timer is already "expired" →
   `MU_TIMER` fires immediately (**a 0-ms timer is a poll**, `:216-220`).
5. **Quick-check message:** if the caller's pipe is non-empty (`p_qindex > 0`)
   or a control-manager fast-path message is pending (`p_msg.action >= 0`),
   deliver it with `ev_mesag(mebuff)` and mark `MU_MESAG` (`:222-229`).
6. **If nothing fired yet**, arm an async wait (`iasync`) for each requested
   event and block on `mwait(flags)` until one signals, then `acancel`s the
   others (`:231-265`).

   > **The blocking `MU_BUTTON` arm is not gated on ownership at all.**
   > `iasync(MU_BUTTON, buparm)` (`:238-239`) calls `abutton`
   > (`aes/gemasync.c:154-156` → `aes/geminput.c:545-565`), whose entry check
   > is an unconditional level test of the live `button` global: `if
   > (downorup(button, p)) azombie(e, 1);` (`geminput.c:549-551`) — no
   > `rlr == gl_mowner` test, unlike the quick-check in point 3. So a caller
   > that (re-)arms this wait while `button` already matches its `buparm`
   > gets an immediate `MU_BUTTON`, **regardless of who owns the mouse** —
   > including during a control-manager track where `gl_mowner == ctl_pd`
   > (see the note after point 3, and [`menu.md`](./menu.md) "The drop-down
   > tracking loop" for the one documented case this produces: a long press
   > held across the app's next `evnt_multi` call). If instead the level does
   > *not* match, the wait truly sleeps on the caller's own
   > `p_cda->c_bsleep` list, and is satisfied only by `post_button(gl_mowner,
   > …)` (`geminput.c:463-483`) — i.e. only while the caller *is*
   > `gl_mowner` — or discarded unsatisfied by `acancel` when some other
   > requested event fires first.

   > **The blocking `MU_M1`/`MU_M2` arm is tested once, at arm time, then
   > only through ownership (A73, 2026-09-20).** `iasync(MU_M1, (LONG)pmo1)` /
   > `iasync(MU_M2, (LONG)pmo2)` (`:242,244`) both route through `amouse`
   > (`aes/gemasync.c:157-162` → `aes/geminput.c:568-587`): it checks
   > `in_mrect` exactly **once**, against the pointer position *at the
   > instant the wait is armed* (`geminput.c:575-576`) — ownership-blind,
   > like `abutton`'s level test above. If the rectangle already matches at
   > that instant the event fires immediately; otherwise the request is
   > queued on the caller's own `p_cda->c_msleep` list and from then on
   > satisfied **only** by `post_mouse(gl_mowner, …)`
   > (`geminput.c:446-456`) — i.e. only while the caller owns the mouse, or
   > from the moment ownership returns to it. Two sites drive `post_mouse`:
   > `mchange` (`geminput.c:388-390`), on every raw mouse move, and
   > `set_mown` (`geminput.c:220-235`), once when the mouse changes hands.
   > A rectangle the pointer crosses *while the control manager owns the
   > mouse* (a bar/drop-down track, `gl_mowner == ctl_pd`) therefore never
   > reaches a parked M1/M2 arm — `post_mouse(ctl_pd, …)` only walks
   > `ctl_pd`'s own `c_msleep`, never the application's. If some other
   > requested event (typically the track's own `MN_SELECTED` message)
   > wakes the caller first, `acancel` discards the unsatisfied M1/M2 arm
   > without re-testing it (`aes/gemevlib.c:264`; `acancel` itself
   > `aes/gemasync.c:217-243`) — the rectangle condition is re-tested only
   > by the *next* call's fresh arm.
   >
   > Traced against a bar-title-select track ([`menu.md`](./menu.md) "Mouse
   > ownership over the track"): **selection on press** — the button is
   > still down when `ct_msgup` busy-waits (`aes/gemctrl.c:100-101`), so
   > `w_setactive` (and the `set_mown` inside it) does not run until *after*
   > the application has already consumed the `MN_SELECTED` message and
   > returned from `evnt_multi` — the app gets `MU_MESAG` alone, and any
   > armed `MU_M1`/`MU_M2` only on its *next* call, where the now-live
   > pointer position satisfies the fresh arm at once. **Selection on
   > release** — `button == 0` when `mn_do` returns, so `ctlmgr`'s next pass
   > reaches `w_setactive` (`aes/gemctrl.c:442`) → `set_mown(app)`
   > (`aes/gemwmlib.c:321-334` → `aes/geminput.c:220-231`) →
   > `post_mouse(app, xrat, yrat)` against the *live* position, before the
   > application is dispatched — so that same call returns `MU_MESAG |
   > MU_M1` (or `M2`) together.
7. **Fill the return block** with `ev_rets` (mouse x/y/button + shift state,
   `:42-58,268`), then, if the wait actually blocked, collect each fired
   event's result via `apret` — `EV_KRET` for `MU_KEYBD`, `EV_BRET` for
   `MU_BUTTON` (`:270-290`). Return the "which fired" bitmask as `RET_CODE`.

`ev_rets` reports the transition-time snapshot (`pr_xrat`/`pr_yrat`/`pr_button`)
when a button transition is pending (`mtrans > 1`), otherwise the live
`xrat`/`yrat`/`button`; `EV_KS` is always the live `kstate` (`gemevlib.c:42-58`).

---

## The single-event waits (opcodes 20–24, 26)

Each parks on one fixed event; the dispatch arms live at `gemsuper.c:125-165`.
`ev_block(code, arg)` (`gemevlib.c:64-69`) is the AES scheduler primitive:
`mwait(iasync(code, arg))` then `apret(code)` — arm the async event, yield to
other AES processes, and return that event's result. Its mechanics are
**reference**; the per-call contract is below.

### `evnt_keybd` (20)

**Source:** `gemsuper.c:126-128` → `ev_block(MU_KEYBD, 0)`.

| Slot | Meaning |
|------|---------|
| `int_in` / `addr_in` | (none) |
| `int_out[0]` | **return: the key code** — scancode in high byte, ASCII in low byte |

Blocks until a key is pressed and returns its packed code. **Note the return
slot:** unlike `evnt_multi` (where the key goes in `EV_KRET` = `int_out[5]`),
`evnt_keybd` returns the key in **`int_out[0]` (`RET_CODE`)** directly. The
encoding is the raw AES/VDI keyboard word (`gsx_char` → VDI string input,
`gemgsxif.c:459-471`): high byte = IKBD scancode, low byte = ASCII (0 for
non-ASCII keys such as the function/cursor keys).

### `evnt_button` (21)

**Source:** `gemsuper.c:129-131` → `ev_button(B_CLICKS, B_MASK, B_STATE,
&EV_MX)` (`gemevlib.c:84-94`).

| Slot | Macro | Meaning |
|------|-------|---------|
| `int_in[0]` | `B_CLICKS` | **low byte** = number of clicks to wait for; **high byte** (not bit!) = enter(0)/leave(1) flag. `combine_cms` packs the whole WORD into bits 31..16 of the LONG (`gemevlib.h:34-48`), so `downorup` reads the flag as `(buparm>>24)&0xff` and `abutton` reads the count as `LOBYTE(HIWORD(p))` |
| `int_in[1]` | `B_MASK` | button bitmask (which buttons to test; byte) |
| `int_in[2]` | `B_STATE` | desired button state (byte) |
| `int_out[0]` | `RET_CODE` | **return: the actual click count reached** |
| `int_out[1..4]` | `EV_MX/MY/MB/KS` | mouse x, y, button state, shift state (filled via `&EV_MX`) |

Blocks until the button state satisfies the `downorup` predicate the requested
number of times (or the multi-click delay window closes), and returns how many
clicks actually occurred. The clicks/mask/state are packed by
`combine_cms(clicks, mask, state)` into a LONG: **clicks in the high WORD, mask
in byte 2, state in byte 3** (`gemevlib.h:34-50`), and decoded by
> **Corrected 2026-09-01.** Two entries in this file previously said "high **bit**"
> where the source says "high **byte**". They are not the same: TeraDesk asks with
> `bclicks = 0x102`, whose high *bit* is 0 but whose high *byte* is 1. Implementing
> the "high bit" reading leaves the button condition inverted — which is exactly the
> defect measured on hardware and fixed in `2d3865b`. The spec carried the bug.

`downorup(new, buparm)` as `flag = (buparm>>24)&0xff` (the enter/leave bit, from
the top byte of the clicks word), `mask = (buparm>>8)&0xff`, `val = buparm&0xff`
(`geminput.c:348-357`). The predicate is
`((mask & (state ^ new)) == 0) != flag`.

### `evnt_mouse` (22)

**Source:** `gemsuper.c:132-134` → `ev_mouse((MOBLK *)&MO_FLAGS, &EV_MX)`
(`gemevlib.c:123-128`).

| Slot | Macro | Meaning |
|------|-------|---------|
| `int_in[0]` | `MO_FLAGS` | rectangle `m_out`: 0 = wait to ENTER, 1 = wait to LEAVE |
| `int_in[1..4]` | `MO_X/Y/WIDTH/HEIGHT` | the `GRECT` |
| `int_out[0]` | `RET_CODE` | return: 1 (`ev_mouse` is `void`; `crysbind`'s `ret` stays `TRUE`) |
| `int_out[1..4]` | `EV_MX/MY/MB/KS` | mouse x, y, **live** button state, shift state |

Blocks until the mouse crosses the rectangle boundary in the requested
direction (`ev_block(MU_M1, &MOBLK)`), then fills the mouse position/state.
`ev_mouse` overwrites `rets[2]` with the **current** `button` after the wait,
so the reported button is always live (`gemevlib.c:127`).

> **The wait is `amouse`'s, and there is no pre-drain (A90/A92,
> 2026-09-21).** `ev_block(MU_M1, pmo)` is `mwait(iasync(MU_M1, pmo))`
> (`gemevlib.c:64-69`) → `amouse` (`aes/gemasync.c:157-162` →
> `aes/geminput.c:568-587`): the rectangle is tested **once, at arm time**,
> ownership-blind (`in_mrect` → `azombie`, `:575-576`), and otherwise parked on
> the caller's `c_msleep`, which only `post_mouse(gl_mowner, …)` walks
> (`:446-456`) — the same rule as `evnt_multi`'s `MU_M1`/`MU_M2` arm (point 6
> note above). Unlike `ev_multi`, `ev_mouse` runs **no** `chkkbd(); forker();`
> before arming (`:123-128` vs `:169-170`): its arm-time position is the one
> the previous dispatch left (`super()` → `dsptch()`, `gemsuper.c:452-464`);
> a packet queued since is processed inside the wait (`mwait` → `dsptch` →
> `forker`) and delivered under ownership. So a rectangle crossed while the
> Control Manager holds the mouse — e.g. during `ct_msgup`'s wait for the
> selecting press to be released (`aes/gemctrl.c:100-101`) — is delivered only
> once ownership returns (`w_setactive` → `set_mown(app)` → `post_mouse(app,
> xrat, yrat)`, `:441-442`, `aes/geminput.c:225-231`), against the position
> at that moment.

### `evnt_mesag` (23)

**Source:** `gemsuper.c:135-141` → `ev_mesag((WORD *)ME_PBUFF)`
(`gemevlib.c:100-117`).

| Slot | Macro | Meaning |
|------|-------|---------|
| `addr_in[0]` | `ME_PBUFF` | pointer to the 16-byte message buffer to fill |
| `int_out[0]` | `RET_CODE` | return: 1 (`void` handler) |

Sets `rlr->p_flags |= AP_MESAG` (the arm, `gemsuper.c:137`), then blocks until a
16-byte message is available and copies it into the caller's buffer. Two
sources feed it:

- The **normal pipe** (`ap_rdwr(MU_MESAG, rlr, 16, mebuff)`, `gemevlib.c:116`) —
  the same 16-byte queue that `appl_write`/`appl_read` use (see
  [`application.md`](./application.md), [`_state.md`](./_state.md) §3).
- A **control-manager fast-path** `WM_ARROWED`: when the pipe is empty
  (`p_qindex == 0`) and a pending scroll message is set (`p_msg.action >= 0`,
  posted by `gemctrl.c:129-133` while an arrow gadget is held), `ev_mesag`
  synthesizes a full `WM_ARROWED` message in place (`msg[0]=WM_ARROWED`,
  `msg[1]=p_pid`, `msg[3]=p_msg.wh`, `msg[4]=p_msg.action`) and clears
  `p_msg.action = -1` (`gemevlib.c:102-114`). **`p_pid` here is the reader's
  own pid — 0 for the application chain** — unlike the normal *queued*
  `WM_ARROWED`, which arrives via `ap_sendmsg` from the Screen Manager with
  `msg[1] = 1` (`gemctrl.c:125`; see [`application.md`](./application.md)
  §"AES process identity and pid", 2026-09-20/A62+A56).

After the arm returns, if `msg[0] == AC_CLOSE` the dispatcher sets
`rlr->p_flags |= AP_ACCLOSE` (`gemsuper.c:139-140`) — the desk-accessory close
handshake `appl_exit` waits on.

### `evnt_timer` (24)

**Source:** `gemsuper.c:142-144` → `ev_timer(MAKE_ULONG(T_HICOUNT, T_LOCOUNT))`
(`gemevlib.c:134-137`).

| Slot | Macro | Meaning |
|------|-------|---------|
| `int_in[0]` | `T_LOCOUNT` | delay, low WORD (milliseconds) |
| `int_in[1]` | `T_HICOUNT` | delay, high WORD |
| `int_out[0]` | `RET_CODE` | return: 1 (`void` handler) |

Waits the given number of **milliseconds** (a 32-bit count assembled hi:lo).
Internally the ms count is divided by `gl_ticktime` (ms per system tick,
`geminit.c:704`) to get a tick count for `ev_block(MU_TIMER, …)`. A count of
**0 ms returns immediately** (see the `evnt_multi` quick-check; the same
0-is-a-poll rule applies).

### `evnt_dclick` (26)

**Source:** `gemsuper.c:163-165` → `ev_dclick(EV_DCRATE, EV_DCSETIT)`
(`gemevlib.c:300-309`).

| Slot | Macro | Meaning |
|------|-------|---------|
| `int_in[0]` | `EV_DCRATE` | new speed index **0..4** (used only when setting) |
| `int_in[1]` | `EV_DCSETIT` | 0 = **get** current speed, non-zero = **set** to `EV_DCRATE` |
| `int_out[0]` | `RET_CODE` | **return: the (new or current) speed index** `gl_dcindex` |

Get or set the double-click speed. When `EV_DCSETIT != 0` it stores
`gl_dcindex = rate` and recomputes the click-window `gl_dclick =
gl_dcrates[rate] / gl_ticktime` (ticks). It **always** returns the current
`gl_dcindex`, so `evnt_dclick(x, 0)` reads the setting without changing it.

**The rate table** `gl_dcrates[5] = {450, 330, 275, 220, 165}`
(`gemevlib.c:33`) — the click-window in **milliseconds** for indices 0
(slowest) … 4 (fastest). The AES initialises the speed to **index 3** (220 ms)
at start-up (`ev_dclick(3, TRUE)`, `geminit.c:708`).

> **Convention note.** Compendium documents `evnt_dclick(rate, setit)` with
> `rate = -1` sometimes cited for "query"; in **this EmuTOS build the query is
> selected by `setit == 0`**, and the `rate` argument is ignored when querying.
> A caller wanting the current speed passes `setit = 0` (any `rate`).

---

## Trap interface (parameter-block form)

All `evnt_*` calls enter via the GEM trap (`svc #2`) with **selector 200 in
`x0`** and an **AESPB pointer in `x1`** — see [`_dispatcher.md`](./_dispatcher.md).
Per call the dispatcher reads `control[0]` = opcode, `control[1..3]` = #int_in /
#int_out / #addr_in, and returns via `int_out[0]` = `RET_CODE`.

| Opcode | Function | `int_in` | `addr_in` | `int_out[0]` return | Also fills |
|-------:|----------|----------|-----------|---------------------|-----------|
| 20 | `evnt_keybd`  | — | — | key code (scancode<<8 \| ascii) | — |
| 21 | `evnt_button` | `[0]`=clicks(+flag), `[1]`=mask, `[2]`=state | — | actual click count | `int_out[1..4]` mouse x/y/btn/shift |
| 22 | `evnt_mouse`  | `[0]`=m_out, `[1..4]`=GRECT | — | 1 | `int_out[1..4]` mouse x/y/btn/shift |
| 23 | `evnt_mesag`  | — | `[0]`=16-byte buffer | 1 | buffer ← message; `AP_MESAG`/`AP_ACCLOSE` |
| 24 | `evnt_timer`  | `[0]`=lo ms, `[1]`=hi ms | — | 1 | — |
| 25 | `evnt_multi`  | `[0]`=flags, `[1..3]`=button, `[4..8]`=rect1, `[9..13]`=rect2, `[14..15]`=timer | `[0]`=16-byte buffer | which events fired (`MU_*` mask) | `int_out[1..6]` mouse/key/button block |
| 26 | `evnt_dclick` | `[0]`=rate(0..4), `[1]`=setit | — | current/new speed index | — |

---

## Inputs / Outputs summary

Inputs arrive in the AESPB arrays (no register/stack params). Every wait that
reports mouse/keyboard state shares the **`int_out[1..6]` return block**:
`[1]`=mouse x (`EV_MX`), `[2]`=mouse y (`EV_MY`), `[3]`=button state (`EV_MB`),
`[4]`=shift state (`EV_KS`), `[5]`=key code (`EV_KRET`), `[6]`=button click
count (`EV_BRET`). The single-event waits fill only `[1..4]` (via `&EV_MX`);
`evnt_multi` also fills `[5]`/`[6]` when the corresponding event fired.
`int_out[0]` (`RET_CODE`) is the per-call return (event mask for `evnt_multi`,
key code for `evnt_keybd`, click count for `evnt_button`, speed index for
`evnt_dclick`, `1` for the `void` waits `evnt_mouse`/`evnt_mesag`/`evnt_timer`).

---

## Error conditions

The event manager has **no negative-error surface** — there are no `E*` TOS
error codes and no documented failure returns. Every `evnt_*` call is a
successful wait/query:

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `evnt_keybd` — always returns a key code | `gemsuper.c:127` | key code (never negative) |
| `evnt_button` — returns click count (may be 0 if the delay window closes) | `gemevlib.c:84-94` | click count ≥ 0 |
| `evnt_multi` — returns the fired-event mask (never 0 in practice: it blocks until something fires, or a 0-ms timer/poll fires immediately) | `gemevlib.c:293` | `MU_*` mask |
| `void` waits (`evnt_mouse`/`evnt_mesag`/`evnt_timer`) | `gemsuper.c:85` | 1 (`crysbind` default `ret`) |
| any opcode not compiled (n/a here — all 20–26 present) | `gemsuper.c:406-417` | -1 |

All seven event opcodes (20–26) have a dispatch `case`, so none of them hit the
dispatcher's unsupported-opcode → `-1` path (see [`_dispatcher.md`](./_dispatcher.md)).

---

## Verification criteria

- **`evnt_multi(MU_KEYBD)` key:** with `int_in[0] = MU_KEYBD` and a key ready,
  the call returns `MU_KEYBD` in `int_out[0]` and the packed key code
  (scancode high byte, ASCII low byte) in `int_out[5]` (`EV_KRET`)
  (`gemevlib.c:179-181,275`).
- **`evnt_keybd` return slot:** `evnt_keybd` returns the key code in
  **`int_out[0]`** (not `int_out[5]`); high byte = scancode, low byte = ASCII
  (`gemsuper.c:127`, `gemgsxif.c:459-471`).
- **`evnt_multi` mouse block:** on any fired event, `int_out[1]`/`int_out[2]`
  hold the current (or transition-time) mouse x/y, `int_out[3]` the button
  state, `int_out[4]` the shift state (`gemevlib.c:42-58,268`).
- **`evnt_button` double-click:** with `int_in[0] = 2` (clicks=2, enter),
  `int_in[1] = 1` (mask=left), `int_in[2] = 1` (state=down), two presses inside
  the current click window (`gl_dcrates[gl_dcindex]` ms) return `2` in
  `int_out[0]`; a second press *after* the window returns `1`
  (`geminput.c:114-147,340-357`).
- **`evnt_mouse` enter vs leave:** `int_in[0]=0` over a rect fires when the
  cursor moves **inside** it; `int_in[0]=1` fires when it moves **outside** —
  `in_mrect` = `m_out != inside(x,y,rect)` (`geminput.c:76-79`).
- **`evnt_timer` poll:** `evnt_timer(0)` (`int_in[0]=int_in[1]=0`) returns
  immediately; `evnt_multi` with `MU_TIMER` and a 0-count fires `MU_TIMER`
  without blocking (`gemevlib.c:216-220`).
- **`evnt_mesag` delivery + AC_CLOSE:** after a message is delivered, the
  16-byte buffer holds it and `AP_MESAG` is set; if `msg[0] == AC_CLOSE`,
  `AP_ACCLOSE` is also set (`gemsuper.c:137-140`).
- **`evnt_dclick` get:** `evnt_dclick(rate, 0)` returns the current
  `gl_dcindex` unchanged (default **3** after boot); `evnt_dclick(1, 1)` sets
  index 1 (330 ms window) and returns `1` (`gemevlib.c:300-309`).

---

## Edge cases discovered in source

1. **`evnt_keybd` returns the key in `int_out[0]`, not `int_out[5]`.**
   The single-event keyboard wait puts the code in `RET_CODE`; only
   `evnt_multi` uses the `EV_KRET` (`int_out[5]`) slot (`gemsuper.c:127` vs
   `gemevlib.c:179`). Programs must read the right slot per call.
2. **A 0-count timer is a poll.** `evnt_timer(0)` and `evnt_multi` with a 0-ms
   `MU_TIMER` fire immediately without yielding — the idiom for a non-blocking
   "check for events now" (`gemevlib.c:216-220`).
3. **`MU_M3` (0x100) cannot be requested by an application.** The dispatcher
   masks `MU_FLAGS & MU_TOSVALID` (0x3F) before calling `ev_multi`, so the
   third rectangle is internal to the menu-extension code only
   (`gemsuper.c:153`, `aesdefs.h:262-263`).
4. **The enter/leave flag for buttons lives in the high bit of the clicks
   word.** `combine_cms` puts clicks in the high WORD of the packed LONG, and
   `downorup` reads the enter/leave `flag` from bit 24 (the top byte of the
   clicks word) — so `int_in[0]` / `MB_CLICKS` with its top bit set means "wait
   to LEAVE the state" (`gemevlib.h:34-50`, `geminput.c:348-357`).
5. **`evnt_mouse` reports the *live* button, not the snapshot.** After the rect
   wait it overwrites `rets[2]` with the current `button`, unlike the other
   waits that may report the transition-time `pr_button` (`gemevlib.c:127`).
6. **The click count can be 0 or grow beyond the request.** `ev_button`
   documents that it returns *how many times the state was actually reached*
   within the delay window — this may be fewer than requested (window closed) or
   the accumulated `mclick`/`gl_bclick` count (`gemevlib.c:72-94`,
   `geminput.c:114-147`).
7. **`evnt_mesag` has a queue-bypass fast path.** A held-down window arrow
   posts `p_msg.action`/`p_msg.wh` directly on the process descriptor; when the
   real pipe is empty, `ev_mesag` synthesizes the `WM_ARROWED` message from
   those fields instead of reading the queue (`gemevlib.c:100-114`,
   `gemctrl.c:129-133`) — auto-repeat scrolling without flooding the 128-byte
   queue. The synthesized message's `msg[1]` is the reader's own pid (0),
   whereas the normal queued `WM_ARROWED` carries `msg[1] = 1` (Screen
   Manager) — see [`application.md`](./application.md) §"AES process
   identity and pid" (2026-09-20/A62+A56).
8. **`evnt_multi` pre-drains the fork queue.** It calls `chkkbd()` + `forker()`
   before the quick checks so a keystroke or button/mouse change that arrived
   just before the call is seen on this pass, not the next (`gemevlib.c:169-170`).
9. **The double-click *speed index* is the return, not the ms value.**
   `evnt_dclick` returns `gl_dcindex` (0..4), never the `gl_dcrates[]`
   millisecond figure (`gemevlib.c:308`).
10. **Multiple bits can be set in the `evnt_multi` return.** If several armed
    events are already satisfied on entry (e.g. a message *and* a matching
    button), the quick-check phase can OR several `MU_*` bits into `RET_CODE`
    (`gemevlib.c:165-229`).

---

## Dependencies

### Requires (must be initialized / functional)

- The **AES scheduler** primitives (`ev_block`/`iasync`/`mwait`/`apret`/
  `acancel`/`forker`) and the run lists (`rlr`, `gl_mowner`) — the cooperative
  multitasking core (`gemevlib.c`, `gemasync.c`, `gemdisp.c`;
  [`_state.md`](./_state.md)). **Reference** — NewTOS reimplements the wait on
  its own process model.
- The **message pipe** (`ap_rdwr`/`gemqueue.c`) for `evnt_mesag`/`evnt_multi`
  `MU_MESAG` — the same 16-byte queue the application manager fills with
  `appl_write` (see [`application.md`](./application.md)).
- **Mouse/keyboard input feeding the AES state** (`xrat`/`yrat`/`button`/
  `kstate`/`mclick`/`gl_mowner`, plus the `bchange`/`mchange`/`kchange` fork
  handlers). In EmuTOS this comes from the IKBD via the VDI `user_but`/
  `user_mot` vectors and `gsx_char`/`gsx_kstate`; in NewTOS from USB-HID/IKBD
  (see the VDI mouse cross-reference below).
- **`gl_ticktime`** (ms per system tick, `geminit.c:704`) for the timer and
  double-click conversions, and the **system tick** driving `b_delay` (Phase-07
  timer / `Vsync`/`CNTPCT` clock).

### Used by

- **Every GEM application's main loop** is `evnt_multi` (or one of the
  single-event waits) plus the window/menu message handlers.
- The **desktop** (`desk/`) and the AES itself call `evnt_multi` to drive
  redraws, menu selection, and window control.
- `evnt_dclick` is called by control panels / the desktop preferences to expose
  the double-click speed slider.

---

## Related data structures

- **`MU_*` event-flag table** — values above (`include/aesdefs.h:256-263`);
  contract.
- **`MOBLK` / `GRECT` rectangle** — the 5-WORD wire layout above
  (`aes/gemlib.h:28-32`, `include/obdefs.h:155-162`).
- **`evnt_multi` slot map** — the `int_in[0..15]` + `addr_in[0]` +
  `int_out[0..6]` tables above (`aesdefs.h:178-224`).
- **The 16-byte standard message** — `WORD msg[8]`; `msg[0]` = message id
  (`MN_SELECTED` 10, `WM_*` 20–31, `AC_OPEN` 40 / `AC_CLOSE` 41,
  `aesdefs.h:227-243`), `msg[1]` = sender ap_id (**0** = application chain,
  **1** = Screen-Manager sender stamp only — see
  [`application.md`](./application.md) §"AES process identity and pid"),
  `msg[2]` = extra-byte count, `msg[3..7]` message-specific. Shared with the
  application manager ([`application.md`](./application.md),
  [`_state.md`](./_state.md) §3).
- **The double-click rate table** — `gl_dcrates[5] = {450, 330, 275, 220, 165}`
  ms, indices 0..4 (`gemevlib.c:33`); default index 3.
- **`combine_cms` / `downorup` button packing** — clicks:WORD ∥ mask:byte ∥
  state:byte, enter/leave in the clicks-word high bit (`gemevlib.h:34-50`,
  `geminput.c:348-357`); reference-level, but the `int_in` slot values are
  contract.
- **`EVB`/`QPB`/`CQUEUE` event blocks + `p_cda`/`p_msg`** — cooperative
  scheduler internals (`aes/struct.h`); **reference** — see
  [`_state.md`](./_state.md).

---

## Cross-references

### Atari Compendium / Profibuch

- **Not present in `references/`** (no PDFs). Cross-check of the `evnt_multi`
  `int_in`/`int_out` slot map, the `MU_*` bit values, the `MOBLK` enter/leave
  semantics, the `evnt_dclick` speed range (0..4) and rate table, and the
  `EV_KRET` scancode|ascii encoding is **deferred** until the documentation PDFs
  are available. (Notably, Compendium sometimes documents `evnt_dclick`'s query
  as `rate = -1`; this build queries via `setit == 0` — see the convention note
  and D-050.)

### AES managers

- [`application.md`](./application.md) — the message pipe (`appl_write` posts
  the messages `evnt_mesag`/`evnt_multi` deliver; both share `ap_rdwr` and the
  16-byte format); `AP_MESAG`/`AP_ACCLOSE` flags.
- [`_state.md`](./_state.md) — the shared identity/queue model, `p_flags`, the
  16-byte message, the mouse/keyboard state globals.
- [`_dispatcher.md`](./_dispatcher.md) — selector-200 call model, `control[]`
  counts, `int_out[0]` return, the unsupported-opcode `-1` path.

### VDI

- [`../vdi/input/_mouse.md`](../vdi/input/_mouse.md) — the mouse position/button
  source. The AES installs `user_but`/`user_mot` via VDI `vex_butv`/`vex_motv`
  to drive `evnt_button`/`evnt_mouse`; `vq_mouse` and the AES `xrat`/`yrat`/
  `button` share the same IKBD/HID packet path. (Do not edit the VDI docs.)

---

## Configuration / version notes

- **`CONF_WITH_MENU_EXTENSION`** (default **1**, `include/config.h:1305`) adds a
  third mouse rectangle (`MU_M3`, a `pmo3` argument to `ev_multi`). It is
  **internal**: `MU_M3` (0x100) is masked out of the application-supplied flags
  by `MU_TOSVALID`, so `evnt_multi`'s ABI is unchanged for callers regardless of
  this flag (`gemsuper.c:152-159`). With the flag off, `ev_multi` takes two
  rectangles instead of three; the contract slot map (M1 at `int_in[4..8]`, M2
  at `int_in[9..13]`) is identical either way.
- The **default double-click speed is index 3** (220 ms window),
  `ev_dclick(3, TRUE)` at AES start-up (`geminit.c:708`). This is a
  boot-time default, not a compile option.
- `gl_ticktime` (ms per tick) is read from the VDI at init (`gsx_tick`,
  `geminit.c:704`) and used to convert the ms-based timer/dclick values into
  tick counts — a **reference** detail (NewTOS uses its own tick source; the
  observable contract is that timers and click windows are specified in ms).
- No TOS-version conditionals affect the `evnt_*` opcode set: all seven
  (20–26) are present in every EmuTOS build.

---

## NewTOS implementation notes

(Empty during Phase 00.)

- **`evnt_multi` is the EC1-critical call.** A window demo's event loop is
  `evnt_multi` + the window/menu message handlers; bring-up can wire a single
  application plus the timer/keyboard/mouse event sources and add cooperative
  AES multitasking (DAs) later without changing the `evnt_*` ABI.
- **Event sources map to NewTOS input.** Keyboard/mouse come from USB-HID / the
  native IKBD ([[keyboard-via-atari-ikbd]] decoder feeds key events; the mouse
  packet feeds `evnt_mouse`/`evnt_button`); the timer from the frame/`CNTPCT`
  clock (cf. XBIOS `Vsync`). Messages come from the per-process 16-byte pipe
  shared with `appl_write` (see [`application.md`](./application.md)).
- **The wait mechanics are free; the flag model + arrays are contract.**
  `ev_block`/`iasync`/`mwait`/`apret`/`acancel` and the fork queue are EmuTOS
  cooperative-scheduler internals — NewTOS parks the calling process on its own
  scheduler. What must match exactly: the `MU_*` bit values, the `int_in`/
  `int_out`/`addr_in` slot layout (especially the `evnt_multi` map and the
  `int_out[1..6]` block), the 16-byte message format, the double-click speed
  range/table and its ms semantics, the enter/leave rectangle rule, and the
  `EV_KRET` scancode|ascii encoding.
- **Pointers in `addr_in[0]` (message buffer) and the cast `MOBLK`/`GRECT`
  slices are user (application) pointers** — apply the same boundary handling as
  other AES `addr*`/parameter reads (see [`_dispatcher.md`](./_dispatcher.md)
  §NewTOS notes). The m68k-vs-native width follows the caller ABI; the WORD
  slot arrays are identical in both worlds.
