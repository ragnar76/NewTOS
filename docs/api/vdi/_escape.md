# VDI `v_escape` (opcode 5) — alpha/cursor sub-functions (Phase 00b D6)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_esc.c` (`vdi_v_escape`,
> `esctbl[]`, `escfn0..escfn19`, `esc_init`/`esc_exit`).
> Calling convention: [`_dispatcher.md`](_dispatcher.md). Line-A cell/
> cursor globals: `include/lineavars.h:51-52` (`v_cel_mx`/`v_cel_my`),
> `bios/lineavars.S:177-185` (`_v_cel_mx`/`_v_cel_my`/`_v_cur_cx`/
> `_v_cur_cy`).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** opcode 5, the `CONTRL[5]` sub-function numbers, which
> `INTIN`/`PTSIN` slots each reads, and what `INTOUT`/`CONTRL[4]` each
> returns. **Reference:** that EmuTOS implements the cursor moves by
> writing VT-52 escape sequences to `Bconout(2,…)` and the inquiries by
> reading Line-A cell/cursor globals — NewTOS reimplements those against
> its own console/framebuffer.

`v_escape` is VDI opcode **5**; the actual sub-function is selected by
`CONTRL[5]` (`vdi_esc.c:336`). These are the VT-52-style **alpha
(text-mode) cursor** controls plus a few inquiries. They operate on the
text/alpha console layer, not on the graphics-attribute state.

---

## Trap interface

| | |
|--|--|
| Opcode | `CONTRL[0] = 5` |
| Sub-function | `CONTRL[5]` = escape id (0..19; also 99 when `HAVE_BEZIER`) |
| Handle | `CONTRL[6]` = workstation handle (resolved by the dispatcher) |
| Inputs | `INTIN[]` (per sub-function); `PTSIN[]` (sub 18) |
| Outputs | `INTOUT[]` + `CONTRL[4]` set by the inquiry sub-functions |

Dispatch (`vdi_v_escape`, `vdi_esc.c:334-350`), in prose:

1. `escfun = CONTRL[5]`.
2. `#if HAVE_BEZIER` **only**: `escfun == 99` calls `v_bez_qual()` (set
   Bezier curve quality) and returns. `HAVE_BEZIER` is **0** in this
   build (`vdi_defs.h:18`), so sub-function 99 is a no-op here.
3. If `escfun > 19` (`ldri_escape`, `vdi_esc.c:23`) → **return, no-op**.
4. Otherwise call `esctbl[escfun](vwk)` (`vdi_esc.c:293-315,349`).

The dispatcher pre-loads `{nptsout,nintout} = {0,0}` for opcode 5
(`vdi_main.c:37`); the inquiry sub-functions (1, 15, 16) set `CONTRL[4]`
themselves.

---

## Sub-function table (`CONTRL[5]`, `esctbl[]`)

| `CONTRL[5]` | Fn | Action | Source |
|:-----------:|----|--------|--------|
| 0  | `escfn0`         | no-op padding entry (simplifies lookup) | `:49` |
| 1  | `vq_chcells`     | inquiry: rows/columns — see below | `:62` |
| 2  | `v_exit_cur`     | exit alpha mode → enter **graphics** mode | `:81` |
| 3  | `v_enter_cur`    | enter alpha mode → exit graphics mode | `:93` |
| 4  | `v_curup`        | alpha cursor up one line (`ESC A`) | `:102` |
| 5  | `v_curdown`      | alpha cursor down one line (`ESC B`) | `:111` |
| 6  | `v_curright`     | alpha cursor right one column (`ESC C`) | `:120` |
| 7  | `v_curleft`      | alpha cursor left one column (`ESC D`) | `:129` |
| 8  | `v_curhome`      | alpha cursor home (`ESC H`) | `:138` |
| 9  | `v_eeos`         | erase to end of screen (`ESC J`) | `:147` |
| 10 | `v_eeol`         | erase to end of line (`ESC K`) | `:156` |
| 11 | `vs_curaddress`  | set cursor row/col — see below | `:172` |
| 12 | `v_curtext`      | output cursor-addressable text — see below | `:196` |
| 13 | `v_rvon`         | reverse video on (`ESC p`) | `:213` |
| 14 | `v_rvoff`        | reverse video off (`ESC q`) | `:222` |
| 15 | `vq_curaddress`  | inquiry: current row/column — see below | `:231` |
| 16 | `vq_tabstatus`   | inquiry: tablet availability — see below | `:248` |
| 17 | `v_hardcopy`     | screen dump via XBIOS `Scrdmp()` | `:260` |
| 18 | `v_dspcur`       | display graphics cursor — see below | `:274` |
| 19 | `v_rmcur`        | remove graphics cursor (`vdi_v_hide_c`) | `:284` |

`esc_init` (`vdi_esc.c:356`) is called from `v_opnwk` and runs `escfn2`
(sub 2, enter graphics mode); `esc_exit` (`:365`) is called from
`v_clswk` and runs `escfn3` (sub 3, enter alpha mode). See
[`workstation/01-v_opnwk.md`](workstation/01-v_opnwk.md).

---

## Sub-functions with inputs / outputs

### 1 — `vq_chcells` (`escfn1`, `vdi_esc.c:62-67`)

- **Output:** `CONTRL[4] = 2`; `INTOUT[0] = v_cel_my + 1` (**number of
  rows**), `INTOUT[1] = v_cel_mx + 1` (**number of columns**).
- `v_cel_my`/`v_cel_mx` are Line-A "rows−1 / columns−1" of the text cell
  grid (`lineavars.h:51-52`). **Reference:** NewTOS derives these from the
  framebuffer font-cell geometry (`yres/cell_h`, `xres/cell_w`).

### 2 — `v_exit_cur` (`escfn2`, `vdi_esc.c:81-84`)

- **No input, no output.** Writes the VT-52 sequence `ESC H ESC f ESC E`
  (home, hide alpha cursor, clear-and-home) to console output via
  `Bconout(2,…)`. Switches from alpha/text to **graphics** mode. Called by
  `esc_init` at `v_opnwk`.

### 3 — `v_enter_cur` (`escfn3`, `vdi_esc.c:93-96`)

- **No input, no output.** Writes `ESC E ESC e CR` (clear-and-home, show
  alpha cursor, carriage return) — the CR resets the BDOS column counter
  for tab expansion. Switches back to alpha/text mode. Called by
  `esc_exit` at `v_clswk`.

### 4–10, 13, 14 — cursor moves + erase + reverse video

- **No input, no output.** Each writes one VT-52 escape to `Bconout(2,…)`:
  `ESC A/B/C/D/H` (up/down/right/left/home), `ESC J/K` (erase to end of
  screen/line), `ESC p/q` (reverse video on/off). **Reference:** NewTOS
  maps these onto the existing console cursor/erase routines.

### 11 — `vs_curaddress` (`escfn11`, `vdi_esc.c:172-183`)

- **Input:** `INTIN[0]` = cursor **row** (1-based), `INTIN[1]` = cursor
  **column** (1-based).
- **No output.** Emits VT-52 `ESC Y <0x20+row-1> <0x20+col-1>`; the
  handler converts the 1-based row/col to the 0-based VT-52 offsets. The
  cursor is shown at the new location unless currently hidden.

### 12 — `v_curtext` (`escfn12`, `vdi_esc.c:196-207`)

- **Input:** `CONTRL[3]` = character count; `INTIN[]` = the character
  array (one char per WORD).
- **No output.** Writes each character via `Crawio()` (GEMDOS raw
  console I/O). Text appears at the current cursor position.

### 15 — `vq_curaddress` (`escfn15`, `vdi_esc.c:231-236`)

- **Output:** `CONTRL[4] = 2`; `INTOUT[0] = v_cur_cy + 1` (**current
  row**, 1-based), `INTOUT[1] = v_cur_cx + 1` (**current column**,
  1-based). `v_cur_cx`/`v_cur_cy` are Line-A current cursor col/row
  (`lineavars.S:184-185`).

### 16 — `vq_tabstatus` (`escfn16`, `vdi_esc.c:248-252`)

- **Output:** `CONTRL[4] = 1`; **`INTOUT[0] = 1`** (a "tablet device" /
  mouse is available). Atari TOS always returns 1, and EmuTOS matches it.
- **Correction:** an earlier revision of this doc claimed `INTOUT[0] = 0`.
  The source unconditionally returns **1** (`vdi_esc.c:251`). Logged in
  [`_discrepancies.md`](_discrepancies.md) D-VDI-2.

### 17 — `v_hardcopy` (`escfn17`, `vdi_esc.c:260-263`)

- **No input, no output.** Calls the XBIOS **`Scrdmp()`** (screen dump to
  printer). In EmuTOS this is a *real* screen dump, **not** a no-op.
- **NewTOS:** the bring-up profile has no printer device, so `Scrdmp()`
  is itself out of bring-up scope; `v_hardcopy` must therefore be a
  **documented no-op** (return cleanly), recorded under `docs/STATUS.md`
  "Blocked" if a program actually needs it — an honest deferral, not a
  faked success. (This is a NewTOS scope decision, not EmuTOS behaviour.)
  Logged in [`_discrepancies.md`](_discrepancies.md) D-VDI-4.

### 18 — `v_dspcur` (`escfn18`, `vdi_esc.c:274-278`)

- **Documented input** (per Atari docs): `PTSIN[0..1]` = the `x,y` where
  the graphics cursor should appear. **EmuTOS, like Atari TOS, ignores
  these** — it forces `INTIN[0] = 0` ("show regardless") and calls
  `vdi_v_show_c()` (VDI opcode 122). So the passed coordinates have no
  effect. **No output.**

### 19 — `v_rmcur` (`escfn19`, `vdi_esc.c:284-287`)

- **No input, no output.** Calls `vdi_v_hide_c()` (VDI opcode 123) to
  remove the graphics cursor.

Sub-functions 4–10, 13, 14, and 18/19 share the mouse/graphics-cursor
and console paths documented in [`input/_mouse.md`](input/_mouse.md).

---

## Verification criteria

- **`vq_chcells` (sub 1):** on a screen whose text grid is `R` rows ×
  `C` columns, returns `CONTRL[4]=2`, `INTOUT[0]=R`, `INTOUT[1]=C`.
- **`vq_curaddress` (sub 15):** with the alpha cursor at row `r`, col `c`
  (1-based), returns `CONTRL[4]=2`, `INTOUT[0]=r`, `INTOUT[1]=c`.
- **`vq_tabstatus` (sub 16):** returns `CONTRL[4]=1`, **`INTOUT[0]=1`**
  (always).
- **`vs_curaddress` (sub 11):** `INTIN=[1,1]` moves the cursor to the
  top-left cell (VT-52 `ESC Y 0x20 0x20`).
- **`v_dspcur` (sub 18):** `PTSIN` coordinates are ignored; the cursor is
  shown regardless.
- **Out-of-range:** `CONTRL[5] = 20` (or any value > 19, and ≠ 99 in a
  non-Bezier build) is a silent no-op.

---

## Edge cases discovered in source

1. **`vq_tabstatus` returns 1, not 0** (`vdi_esc.c:251`) — "there is a
   mouse". Easy to get wrong from the function name.
2. **`v_dspcur` ignores its coordinates** (`vdi_esc.c:274-278`) — Atari
   docs specify `v_dspcur(handle,x,y)` but TOS and EmuTOS both ignore
   `x,y` and show the cursor at its current position.
3. **`v_exit_cur` sends a leading `ESC H`** (`vdi_esc.c:81-84`) as a
   harmless sequence: if a `.TOS`/`.TTP` program left a bare `ESC` in the
   console before entering graphics mode, the first char of the real
   sequence would be eaten, so a throwaway `ESC H` absorbs it.
4. **Sub-function 99 = Bezier quality** (`vdi_esc.c:340-345`), only when
   `HAVE_BEZIER` — outside the normal 0..19 range, handled before the
   range check. `HAVE_BEZIER = 0` here, so it is a no-op.
5. **`v_curtext` uses GEMDOS `Crawio`, the others use BIOS `Bconout(2)`**
   (`vdi_esc.c:29-43,205`). The BIOS path is chosen for the cursor moves
   so they are uninterruptible by Ctrl-C; `v_curtext` deliberately uses
   raw GEMDOS console I/O for the text bytes.

---

## Dependencies

- `Bconout` (BIOS 3) for the VT-52 sequences; `Crawio` (GEMDOS 0x07) for
  `v_curtext`; `Scrdmp` (XBIOS 20) for `v_hardcopy`.
- `vdi_v_show_c` (122) / `vdi_v_hide_c` (123) for `v_dspcur`/`v_rmcur`.
- Line-A cell/cursor globals `v_cel_mx/my`, `v_cur_cx/cy` for the
  inquiries.
- The VT-52 console emulation (framebuffer text layer).

## Cross-references

- **Atari Compendium / Profibuch:** not present in `references/` (no PDFs).
  Cross-check of the documented `v_escape` sub-function ABI (esp. the
  `vq_tabstatus` return and the `v_dspcur` coordinate semantics) is
  **deferred** until the references are available.

## NewTOS implementation notes (Phase 12)

- The alpha cursor layer is the **VT-52 text console** the framebuffer
  already drives (Phase 06 console output). `v_enter_cur`/`v_exit_cur`
  toggle between the graphics workstation and that text layer; map them
  onto the existing console cursor/erase routines rather than porting the
  VT-52 byte streams literally.
- `vq_chcells`/`vq_curaddress` report rows/cols from the framebuffer font
  cell geometry; keep the values 1-based on output, matching EmuTOS.
- `v_hardcopy` (17) is a documented no-op (no printer) — see above.
- `v_dspcur`/`v_rmcur` bind to the mouse-cursor sprite path
  ([`input/_mouse.md`](input/_mouse.md)); ignore the `PTSIN` coordinates
  exactly as EmuTOS does.
