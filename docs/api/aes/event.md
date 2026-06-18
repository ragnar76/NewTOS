# AES Event Manager (`evnt_*`) — opcodes 20–26

> **PHASE-00b D8, Batch 2.** Source: EmuTOS @ `1a7484ab` —
> `aes/gemevlib.c`, dispatch `aes/gemsuper.c:126-167`, scheduler
> `aes/gemdisp.c` (`ev_block`), input `aes/geminput.c`.
>
> Call convention: [`_dispatcher.md`](./_dispatcher.md).

## Event-type flags (`include/aesdefs.h:256-262`)

Bit mask selecting which events a wait blocks on (used by `evnt_multi`):

| Flag | Value | Event |
|------|------:|-------|
| `MU_KEYBD`  | 0x01 | a key was pressed |
| `MU_BUTTON` | 0x02 | mouse button event matched |
| `MU_M1`     | 0x04 | mouse entered/left rectangle 1 |
| `MU_M2`     | 0x08 | mouse entered/left rectangle 2 |
| `MU_MESAG`  | 0x10 | a message arrived in the pipe |
| `MU_TIMER`  | 0x20 | the timer expired |
| `MU_TOSVALID` | 0x3F | all of the above |

`MOBLK` (mouse rectangle, for M1/M2): `{ WORD flags, x, y, w, h }` —
`flags` selects enter (0) vs leave (1) of the rect.

## `evnt_multi` (25) — the central multiplexer

`gemsuper.c:145` → `ev_multi()` `gemevlib.c`. The heart of every GEM
event loop: blocks until **any** of the requested events fires, returns
the bitmask of which ones did, and reports the mouse/keyboard/button
state for all of them at once.

| intin | arg | intin | arg |
|------:|-----|------:|-----|
| 0 | flags (MU_*) | 9–13 | m2 rect (flags,x,y,w,h) |
| 1 | button clicks | 14 | timer low count (ms) |
| 2 | button mask | 15 | timer high count |
| 3 | button state | addrin[0] | message buffer (16 bytes) |
| 4–8 | m1 rect (flags,x,y,w,h) | | |

| intout | result |
|------:|--------|
| 0 | **which events** fired (MU_* bitmask) — `RET_CODE` |
| 1,2 | mouse x, y |
| 3 | button state |
| 4 | keyboard shift state |
| 5 | key code (scancode<<8 \| ascii) |
| 6 | mouse click count |

EmuTOS sets `AP_MESAG` in `p_flags` when `MU_MESAG` is requested,
combines the button params (`combine_cms`), builds the timer count from
hi/lo, and dispatches to `ev_multi`. If a returned message is `AC_CLOSE`
(a desk accessory being closed) it sets `AP_ACCLOSE`.

## The single-event waits

Each is a special case of `evnt_multi`:

| Op | Name | Dispatch | Waits for / returns |
|---:|------|----------|---------------------|
| 20 | `evnt_keybd`  | `ev_block(MU_KEYBD)` | a key; returns scancode<<8 \| ascii |
| 21 | `evnt_button` | `ev_button(clicks, mask, state, &mx)` | button match; returns click count; fills mouse x/y, button, shift |
| 22 | `evnt_mouse`  | `ev_mouse(MOBLK, &mx)` | mouse enters/leaves a rect; fills x/y |
| 23 | `evnt_mesag`  | `ev_mesag(pbuff)` | a 16-byte message in the pipe; sets `AP_MESAG`; `AC_CLOSE` → `AP_ACCLOSE` |
| 24 | `evnt_timer`  | `ev_timer(ms)` | `ms` milliseconds (hi/lo WORD count) |
| 26 | `evnt_dclick` | `ev_dclick(rate, setit)` | get (setit=0) or set the double-click speed |

`ev_block(code, arg)` is the AES scheduler primitive: it parks the
calling AES process on event `code` and yields to others until the event
fires (`gemdisp.c`).

## The standard 16-byte message (`evnt_mesag` / the pipe)

GEM messages are a `WORD msg[8]` array; `msg[0]` is the message type.
The AES posts these (window/menu events) and applications read them via
`evnt_mesag`/`evnt_multi`. Common types (defined in `include/aesdefs.h`):
`MN_SELECTED`, `WM_REDRAW`, `WM_TOPPED`, `WM_CLOSED`, `WM_FULLED`,
`WM_ARROWED`, `WM_HSLID`, `WM_VSLID`, `WM_SIZED`, `WM_MOVED`,
`AC_OPEN`, `AC_CLOSE`. Detailed with the Window/Menu managers.

---

## NewTOS implementation notes

- `ev_multi`/`ev_block` need the **AES scheduler** (cooperative,
  `gemdisp.c`) — multiple AES processes yielding to each other. This sits
  on the Phase-06 process model; bring-up can run a single application +
  the timer/keyboard/mouse event sources and add multitasking later.
- **Event sources:** keyboard + mouse come from the IKBD
  ([[keyboard-via-atari-ikbd]] decoder feeds key events; mouse packets
  feed `evnt_mouse`/button); the timer from the frame/`CNTPCT` clock
  (cf. XBIOS `Vsync`). Messages come from the `appl_*` pipe (Batch 1).
- This is the **EC1-critical** manager: a window demo's event loop is
  `evnt_multi` + the window/menu messages.
