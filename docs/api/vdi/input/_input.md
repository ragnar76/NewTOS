# VDI logical input devices (Phase 00b D6)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_mouse.c` (`vdi_v_locator`),
> `vdi/vdi_input.c` (`vdi_v_choice`, `vdi_v_string`, `vdi_vsin_mode`).
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md).
> `vqin_mode` (115) is in
> [`../inquiry/_color-pixel-input.md`](../inquiry/_color-pixel-input.md).

Each logical input device has a **mode**: REQUEST (block until input)
or SAMPLE (poll, return immediately). The mode is set by `vsin_mode`
and stored in `loc_mode`/`val_mode`/`chc_mode`/`str_mode`
(0 = request, 1 = sample, internally; see note).

| Op | Fn | Device | Request vs. Sample |
|----|----|--------|--------------------|
| 28 | `v_locator` | locator (mouse/keys) | `vrq_locator` / `vsm_locator` |
| 30 | `v_choice`  | choice (function keys) | `vrq_choice` / `vsm_choice` |
| 31 | `v_string`  | string (keyboard) | `vrq_string` / `vsm_string` |
| 33 | `vsin_mode` | set input mode | — |

## `vsin_mode` (33)

`INTIN[0]` = device (1=locator, 2=valuator, 3=choice, 4=string);
`INTIN[1]` = mode (1=request, 2=sample). Stores `mode-1` in the
matching global and echoes `INTOUT[0] = INTIN[1]`.

## `v_locator` (28)

Initial position from `PTSIN[0..1]` → `GCURX`/`GCURY`.
- **Request** (`loc_mode==0`): show cursor, loop until a button or
  keyboard event (`gloc_key()`); return `INTOUT[0]` = terminating char
  (`TERM_CH & 0xff`), `PTSOUT[0..1]` = final position, `CONTRL[2]=1`
  (one point), `CONTRL[4]=1` (one char); hide cursor.
- **Sample** (`loc_mode!=0`): poll once; if a key event, set
  `CONTRL[4]=1` + `INTOUT[0]`; if a motion event, set `CONTRL[2]=1` +
  `PTSOUT[0..1]`.

## `v_choice` (30)

`gchc_key()`; `INTOUT[0] = TERM_CH & 0xff` — the choice value (function
key) the user pressed.

## `v_string` (31)

`INTIN[0]` = max length `j`; negative `j` = return raw scancodes
(`mask=0xffff`), positive = ASCII only (`mask=0xff`).
- **Request** (`str_mode==0`): read chars until `j` reached or CR
  (`0x0d`); each goes to `INTOUT[i]`. CR is not counted. `CONTRL[4]` =
  char count.
- **Sample** (`str_mode!=0`): drain available chars (up to `j`) without
  blocking; `CONTRL[4]` = count.

## NewTOS implementation notes

- All four bind to the **keyboard/mouse input subsystem**
  (`gloc_key`/`gchc_key`/`gchr_key` = the VDI input event sources).
  Until that subsystem exists for QEMU, REQUEST-mode calls must block
  on the real event queue (as console input already does, Phase 06),
  not return canned values; SAMPLE-mode returns "no event" cleanly.
- The valuator device (`v_valuator`, op 29) is absent from EmuTOS
  (no analogue input on Atari) — `vsin_mode`/`vqin_mode` still track
  `val_mode` for ABI completeness, but there is no `v_valuator`
  handler. Note in the dispatcher.
- Char/scancode encoding (`mask` 0xff vs 0xffff) matches the BIOS
  keyboard codes; reuse the Phase-06 keyboard decode.
