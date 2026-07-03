# AES shared state — process descriptors, ids, the message pipe

> **PHASE-00b D8 (AES).** Source: EmuTOS @ `1a7484ab` —
> `aes/aesvars.h` (private AES globals), `aes/geminit.c` (start-up init),
> `aes/struct.h` (`AESPD`/`QPB`/`EVB`), `aes/gemlib.h` (`THEGLO`/`AESPROCESS`),
> `aes/gempd.c` (descriptor lookup), `aes/gemqueue.c` (the message pipe).
>
> Referenced by every AES manager spec for the shared identity/queue model.
> Per-call state (the `AESGLOBAL` `global[]` array) is documented in
> [`application.md`](./application.md); the trap/marshalling in
> [`_dispatcher.md`](./_dispatcher.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** the *observable* facts — the ap_id space (small integers; 0/1
> system, 2+ apps/DAs), `ap_count = 1` reported, the 8-char space-padded name
> format, the 16-byte standard message, the `-1` not-found convention.
> **Reference (NewTOS reimplements freely):** the `AESPD`/`EVB`/`QPB` structs,
> the ready/deferred/not-ready run lists, `dsptch`/`ev_block`, and the fixed
> `p_queue[128]` layout — these are the EmuTOS cooperative-scheduler internals,
> not something a program observes.

---

## 1. Process descriptors and the run lists

The AES models each application (and each desk accessory) as an **`AESPD`**
process descriptor (`aes/struct.h:119-155`). At start-up
(`geminit.c:810-877`):

- `num_accs = count_accs()` desk accessories are counted (`geminit.c:810`);
- `totpds = num_accs + 2` descriptors exist — pids **0 and 1 are the two AES
  system processes** (the desktop/ctlmgr internals), pids **2 … totpds-1** are
  desk accessories / the running application (`geminit.c:819`,
  `struct.h:36` `NUM_PDS = NUM_ACCS + 2`);
- `curpid` hands out pids sequentially (`gempd.c:60-70`, `geminit.c:875-877`).

The descriptors are held inside the single AES global area `THEGLO D`
(`geminit.c:141`, `aes/gemlib.h:118-144`): `D.g_int[2]` for the two system
processes and `D.g_acc[num_accs]` for accessories (`gemlib.h:118,143`).
`pd_index(i)` maps an index to its descriptor (`gempd.c:32-35`).

**Run-list roots** (`aes/aesvars.h:17-20`, `geminit.c:131`):

| Global | Role |
|--------|------|
| `rlr` | **run-list root** — the currently running AES process descriptor |
| `drl` | deferred/ready list |
| `nrl` | not-ready list (blocked processes) |
| `eul` | free `EVB` list; `dlr`/`zlr` deferred/zombie event lists |
| `indisp` | non-zero while inside `dsptch` (`aesvars.h:38`) |

`rlr` is what `appl_init`/`appl_exit` read and write (`rlr->p_pid`,
`rlr->p_flags`). NewTOS's `rlr` equivalent is "the current AES process".

---

## 2. Application id (`ap_id`)

The application id returned by `appl_init` and stored in `global[2]` is simply
`AESPD.p_pid` (`gemsuper.c:94`, `struct.h:129`). Properties (contract):

- Small integers; **0 and 1 are system**, DAs/apps are **2+**.
- `ap_count` reported by `appl_init` is hard-coded **1** (`gemsuper.c:93`) even
  though EmuTOS runs several AES processes — the *reported* count is 1.
- `appl_find` resolves an 8-char name → pid (or -1); `appl_read`/`appl_write`
  resolve a pid → descriptor via `fpdnm(NULL, pid)` (`gempd.c:38-57`).

**Name format** (`gempd.c:77-86`, `p_nameit`): the descriptor's `p_name`
(`AP_NAMELEN = 8`, `struct.h:111,125`) is the first ≤8 filename chars, stopping
at the first `.` (extension stripped), **space-padded to 8** with `' '`.
`appl_find` compares all 8 bytes with `strncmp` (`gempd.c:48`).

---

## 3. The message pipe (16-byte standard message)

Each descriptor owns a fixed queue: `p_queue[QUEUE_SIZE]` with
`QUEUE_SIZE = 128` (`struct.h:57,152`), a cursor `p_qaddr` and byte count
`p_qindex` (`struct.h:150-151`; initialised `p_qaddr = p_queue`,
`p_qindex = 0` at start-up, `geminit.c:864-865`).

- **`appl_write` (`MU_SDMSG`)** appends a message to the *target's* queue;
  **`appl_read` (`MU_MESAG`)** removes one from the *caller's* queue
  (`gemaplib.c:69-89`, `gemqueue.c:aqueue`/`doq`).
- The **GEM standard message is 16 bytes** = a `WORD msg[8]` array; `msg[0]` is
  the message id (`MN_SELECTED`/`WM_*`/`AC_*`, `include/aesdefs.h:227-243`),
  `msg[1]` = sender ap_id, `msg[2]` = extra bytes, `msg[3..7]` message-specific.
- `ap_rdwr` has a fast 16-byte path (no blocking) when the queue holds exactly
  16 bytes; otherwise it blocks on `ev_block` (`gemaplib.c:77-88`).
- The same pipe backs `evnt_mesag`/`evnt_multi` (`gemevlib.c:116`) — the event
  and application managers share it. See [`event.md`](./event.md).

`doq` coalesces redraw messages (unions `WM_REDRAW` rects for the same window,
overwrites pending `WM_ARROWED`) — a reference-level pipe optimisation
(`gemqueue.c:44-72`).

---

## 4. Process flags (`p_flags`)

`aes/struct.h:115-117`:

| Flag | Value | Meaning |
|------|------:|---------|
| `AP_OPEN`    | 0x0001 | application is between `appl_init` and `appl_exit` |
| `AP_MESAG`   | 0x0002 | application has waited for a message |
| `AP_ACCLOSE` | 0x0004 | application has seen an `AC_CLOSE` message |

`appl_init` sets `AP_OPEN`; `appl_exit` clears it (`gemaplib.c:60,260`).
`evnt_mesag`/`evnt_multi` set `AP_MESAG`/`AP_ACCLOSE`; `appl_exit` uses
`wait_for_accs(AP_ACCLOSE)` to let DAs drain their close (`gemaplib.c:255`).

---

## 5. Other shared AES globals used by the application manager

| Global | Source | Used by |
|--------|--------|---------|
| `gl_nplanes` | `geminit.c` (screen planes) | `appl_init` → `ap_planes` |
| `rs_trees[DESKTOP]` | resource manager | `appl_init` → `ap_private` (DESKTOP root `ob_spec`) |
| `D` (`THEGLO`) | `geminit.c:141` | `appl_init` → `ap_3resv = &D`; `D.g_valstr` scratch used by `appl_exit` |
| `gl_play`/`gl_recd`/`gl_rbuf`/`gl_rlen` | `gemaplib.c:45-48` | `appl_tplay`/`appl_trecord` tape state |

---

## 6. Event-manager input, timer, and double-click state

The event manager ([`event.md`](./event.md)) reads a separate set of AES
globals that mirror the current mouse / keyboard state and the timing config.
These are **reference** internals (fed by the input path and the system tick);
what a program observes is the values the `evnt_*` calls report, not the globals.

| Global | Decl. | Meaning | Read by |
|--------|-------|---------|---------|
| `xrat`, `yrat` | `geminput.h:16` | live mouse x, y | `ev_rets`, `in_mrect` |
| `button` | `geminput.h:16` | live mouse-button state | `ev_rets`, `ev_mouse`, `evnt_multi` quick-check |
| `kstate` | `geminput.h:16` | keyboard shift state (`EV_KS`) | `ev_rets` |
| `mclick` | `geminput.h:16` | last button click count (`EV_BRET`) | `evnt_multi`/`evnt_button` |
| `mtrans` | `geminput.h:16` | pending button-transition flag (>1 ⇒ use the `pr_*` snapshot) | `ev_rets` |
| `pr_xrat`/`pr_yrat`/`pr_button`/`pr_mclick` | `geminput.h:17` | transition-time snapshot of the above | `ev_rets`, `evnt_multi` |
| `gl_mowner` | `geminput.h:19` | the AES process that currently owns the mouse | `evnt_multi` (button/rect quick-checks gated on `rlr == gl_mowner`) |
| `gl_ticktime` | `gemevlib.c:36` | ms per system tick (`gsx_tick`, `geminit.c:704`) | `ev_timer`, `ev_dclick` (ms→ticks) |
| `gl_dcindex` | `gemevlib.c:34` | current double-click **speed index** 0..4 (returned by `evnt_dclick`) | `ev_dclick` |
| `gl_dclick` | `gemevlib.c:35` | click-window in **ticks** (`gl_dcrates[idx]/gl_ticktime`) | `b_click` (multi-click timing) |
| `gl_dcrates[5]` | `gemevlib.c:33` | rate table `{450,330,275,220,165}` ms, indices 0..4 | `ev_dclick` |
| `rlr->p_msg` | `struct.h:142-144` | control-manager fast-path `WM_ARROWED` (`action`/`wh`) that bypasses the queue | `ev_mesag` |

- The click-count / double-click timing lives in `b_click`/`b_delay`
  (`geminput.c:114-179`): a first click starts a `gl_dclick`-tick window; further
  clicks into the same state increment `gl_bclick` and extend the window; mouse
  motion of more than a couple of pixels cancels it (`mchange`,
  `geminput.c:360-368`). The **default speed is index 3** (220 ms),
  `ev_dclick(3, TRUE)` at start-up (`geminit.c:708`).
- **Contract vs. reference:** the *values* the event calls report — mouse x/y,
  button/shift state, click count, and the double-click speed index and its ms
  semantics — are contract; the `xrat`/`mtrans`/`pr_*`/`gl_mowner` mirrors and
  the `b_click` timing machinery are EmuTOS internals NewTOS reimplements over
  its own USB-HID/IKBD input path and system tick.

---

## 6a. Menu-manager state — the active menu bar and DA registration

The menu manager ([`menu.md`](./menu.md)) keeps a small amount of shared state
identifying the currently installed menu bar and the registered desk
accessories. These are **reference** internals (fed by `menu_bar`/`menu_register`);
what a program observes is the drawn bar, the `MN_SELECTED`/`AC_OPEN` messages,
and the `menu_register` slot-id/-1 return.

| Global | Decl. | Meaning | Read/written by |
|--------|-------|---------|-----------------|
| `gl_mntree` | `gemmnlib.c:62` | ptr to the **active menu OBJECT tree** (`NULL` = no bar) | set/reset by `mn_bar` (`menu_bar`); read by the control manager's tracking loop `mn_do` |
| `gl_mnppd` | `gemmnlib.c:63` | the AES process descriptor that **owns** the current bar | set by `mn_bar`; used as the `MN_SELECTED` recipient (`gemctrl.c:353`) |
| `gl_ctwait` | `gemctrl.h:13` | the control-manager mouse-wait rectangle (bar extent when a bar is up, else full desktop `gl_rmenu`, `gsxdefs.h:108`) | `mn_bar` (`gemmnlib.c:715-726`) |
| `gl_dafirst` | `gemmnlib.c:68` | object # of the first desk-accessory entry in the tree | `menu_fixup`; the pick router (`gemctrl.c:337`) |
| `D.g_accreg` | `gemlib.h` (`THEGLO`) | count of registered DAs (bounded by `NUM_ACCS`) | `mn_register`/`mn_init` |
| `D.g_acctitle[NUM_ACCS]` | `gemlib.h` (`THEGLO`) | the DA menu-item **title string pointers** (stored, not copied) | `mn_register`/`mn_cleanup`/`mn_init` |
| `desk_ppd[NUM_ACCS]` | `gemmnlib.c:65` | the owner process descriptor per DA slot | `mn_register`; `mn_getownid` (route `AC_OPEN`) |
| `acc_display[NUM_ACCS]` | `gemmnlib.c:66` | display-slot → DA menu-id lookup table | `build_menuid_lookup`; `mn_getownid` |
| `gl_submenu`/`gl_submenu_hwm` | `gemmnext.c:57-58` | submenu-info (`SMIB`) array (`CONF_WITH_MENU_EXTENSION`) | `menu_attach`/`menu_istart`; freed by `mn_cleanup` |

- **DA-slot model (contract vs. reference).** `NUM_ACCS = 6`
  (`include/sysconf.h:37`) → six DA slots. `menu_register` returns the slot index
  `0..5` (the DA's *menu id*) or `-1` when full/invalid — **the slot-id/-1 return
  is contract**; the `g_acctitle[]`/`desk_ppd[]`/`acc_display[]` storage and
  `gl_mntree`/`gl_mnppd` pointers are EmuTOS internals NewTOS reimplements.
- **Single-app note.** `menu_register` and the `AC_OPEN` routing presume
  concurrent AES processes (the accessories); at NewTOS single-app bring-up there
  are none, so this state is inert until the DA machinery exists (cf.
  [`application.md`](./application.md) `wait_for_accs`).

---

## 7. NewTOS mapping (contract vs. reference)

- **Contract to keep:** ap_id = a small process id (system 0/1, apps 2+);
  `appl_init` reports `ap_count = 1`; the 8-char space-padded name; the 16-byte
  message; `-1` for not-found.
- **Free to reimplement:** the `AESPD`/`EVB`/`QPB` structs, the run-list
  scheduler (`dsptch`/`ev_block`/`mwait`), the fixed 128-byte `p_queue`, and the
  `THEGLO` monolith — NewTOS uses its Phase-06 process/IPC model. The AES
  application *is* a NewTOS process; `appl_init`/`appl_exit` bracket its life as
  `Pexec`/`Pterm` do at GEMDOS level.
- **Single-app vs. multi-app:** EmuTOS's concurrent-DA scheduling is reference;
  NewTOS's cooperative single-AES-app (desktop) bring-up satisfies the reported
  `ap_count = 1` with one process and can add DA concurrency later. Open
  question — see [`application.md`](./application.md) NewTOS notes.
