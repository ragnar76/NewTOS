# AES spec extraction — working list (PHASE-00b D8)

> Tracks the per-function AES spec extraction. Order mirrors the Phase 13
> implementation deliverables (D2–D9). One spec file per function under
> `docs/api/aes/<manager>/<opcode>-<name>.md`, produced by reading the
> EmuTOS source per `docs/SOURCE-ANALYSIS-PROTOCOL.md`.
>
> Batch 0 (call convention + dispatcher) is done:
> [`_dispatcher.md`](./_dispatcher.md).

## EmuTOS source file per manager

| Manager | Library source | Notes |
|---------|----------------|-------|
| Application | `aes/gemaplib.c` (`ap_*`) | + `gempd.c` (process descriptors), `gemqueue.c` (message pipe) |
| Event | `aes/gemevlib.c` (`ev_*`) | + `geminput.c`, `gemasync.c`, `gemflag.c` |
| Menu | `aes/gemmnlib.c` (`mn_*`) | + `gemmnext.c` |
| Object | `aes/gemoblib.c` (`ob_*`) | + `gemobed.c` (edit), `gemobjop.c` |
| Form | `aes/gemfmlib.c` (`fm_*`) | + `gemfmalt.c` (alerts) |
| Graphics | `aes/gemgrlib.c` (`gr_*`) | + `gemgraf.c` (primitives) |
| Scrap | `aes/gemsclib.c` (`sc_*`) | |
| File selector | `aes/gemfslib.c` (`fs_*`) | |
| Window | `aes/gemwmlib.c` (`wm_*`) | + `gemwrect.c` (rectangle list) |
| Resource | `aes/gemrslib.c` (`rs_*`) | RSC file format |
| Shell | `aes/gemshlib.c` (`sh_*`) | process launch / desktop |
| Kernel | `gemctrl.c`, `gemdisp.c`, `geminit.c`, `gemsuper.c` | dispatch, scheduler, init |

## Batches (= spec deliverables, mapped to Phase 13)

### Batch 1 — Application Manager → Phase 13 D2
`gemaplib.c` + `gemqueue.c` + `gempd.c`. The AES lifecycle + message pipe.

| Op | Name | |
|---:|------|--|
| 10 | `appl_init`    | register app, allocate AESGLOBAL/PD, return ap_id |
| 19 | `appl_exit`    | deregister |
| 11 | `appl_read`    | read from the app's message pipe |
| 12 | `appl_write`   | write to another app's pipe |
| 13 | `appl_find`    | find an app by name |
| 17 | `appl_yield`   | cooperative yield |
| 14/15 | `appl_tplay`/`appl_trecord` | (PC-GEM record/play — likely stub) |

### Batch 2 — Event Manager → Phase 13 D3
`gemevlib.c` (+ `geminput.c`, `gemflag.c`). The event multiplexer.

| Op | Name | |
|---:|------|--|
| 25 | `evnt_multi`  | the central wait-for-anything multiplexer |
| 20 | `evnt_keybd`  | wait for a key |
| 21 | `evnt_button` | wait for mouse buttons |
| 22 | `evnt_mouse`  | wait for mouse to enter/leave a rect |
| 23 | `evnt_mesag`  | wait for a message |
| 24 | `evnt_timer`  | wait for a timer |
| 26 | `evnt_dclick` | get/set double-click speed |

### Batch 3 — Object Manager + Form Manager → Phase 13 D6
`gemoblib.c`/`gemobed.c` + `gemfmlib.c`/`gemfmalt.c`. Object trees +
dialog/form rendering — the heart of GEM UI.

| Op | Name | | Op | Name | |
|---:|------|--|---:|------|--|
| 42 | `objc_draw`   | draw an object tree | 50 | `form_do`     | run a dialog |
| 43 | `objc_find`   | hit-test            | 51 | `form_dial`   | reserve/free dialog screen |
| 40 | `objc_add`    | add child           | 52 | `form_alert`  | alert box |
| 41 | `objc_delete` |                     | 53 | `form_error`  | DOS-error alert |
| 44 | `objc_offset` | absolute coords     | 54 | `form_center` | centre a tree |
| 45 | `objc_order`  |                     | 55 | `form_keybd`  | dialog key handling |
| 46 | `objc_edit`   | editable text field | 56 | `form_button` | dialog button handling |
| 47 | `objc_change` | change state + redraw | | | |
| 48 | `objc_sysvar` | system look settings | | | |

### Batch 4 — Window Manager + Graphics → Phase 13 D4
`gemwmlib.c`/`gemwrect.c` + `gemgrlib.c`. Windows + rubber-band feedback.

| Op | Name | | Op | Name | |
|---:|------|--|---:|------|--|
| 100 | `wind_create` | | 105 | `wind_set`    | set window field |
| 101 | `wind_open`   | | 106 | `wind_find`   | window under point |
| 102 | `wind_close`  | | 107 | `wind_update` | begin/end update |
| 103 | `wind_delete` | | 108 | `wind_calc`   | work↔border area |
| 104 | `wind_get`    | get field | 109 | `wind_new` | reset all |
| 70–79 | `graf_*` | rubbox/dragbox/growbox/shrinkbox/watchbox/slidebox/handle/mouse/mkstate | | | |

### Batch 5 — Menu Manager → Phase 13 D5
`gemmnlib.c` (+ `gemmnext.c`).

| Op | Name | | Op | Name | |
|---:|------|--|---:|------|--|
| 30 | `menu_bar`    | show/hide menu bar | 35 | `menu_register` | desk-accessory slot |
| 31 | `menu_icheck` | check mark         | 36 | `menu_popup`    | pop-up menu |
| 32 | `menu_ienable`| enable/disable     | 37 | `menu_attach`   | sub-menu |
| 33 | `menu_tnormal`| highlight title    | 38 | `menu_istart`   | |
| 34 | `menu_text`   | change item text   | 39 | `menu_settings` | |

### Batch 6 — Resource + File selector → Phase 13 D7+D8
`gemrslib.c` + `gemfslib.c`.

| Op | Name | | Op | Name | |
|---:|------|--|---:|------|--|
| 110 | `rsrc_load` | load .RSC, fix up | 90 | `fsel_input`   | file selector |
| 111 | `rsrc_free` |                   | 91 | `fsel_exinput` | + title |
| 112 | `rsrc_gaddr`| index → address   | | | |
| 113 | `rsrc_saddr`| set address       | | | |
| 114 | `rsrc_obfix`| char→pixel coords | | | |

### Batch 7 — Scrap + Shell → Phase 13 D9
`gemsclib.c` + `gemshlib.c`. Clipboard + process/desktop launch.

| Op | Name | | Op | Name | |
|---:|------|--|---:|------|--|
| 80 | `scrp_read`  | clipboard path | 120 | `shel_read`  | get cmdline |
| 81 | `scrp_write` |                | 121 | `shel_write` | launch a program |
| 82 | `scrp_clear` |                | 122–127 | `shel_get/put/find/envrn/rdef/wdef` | |

## Structures to extract (shared)

- **`AESGLOBAL`** — per-app state (the `global[]` array) — Batch 1.
- **`OBJECT`** — object-tree node (`ob_next/head/tail`, `ob_type`,
  `ob_flags`, `ob_state`, `ob_spec`, `ob_x/y/w/h`) — Batch 3, `include/`.
- **`TEDINFO`** / **`ICONBLK`** / **`BITBLK`** — object `ob_spec` payloads
  — Batch 3.
- **`MFDB`** — memory form definition block (raster) — shared with VDI.
- **RSC file header** — Batch 6.
