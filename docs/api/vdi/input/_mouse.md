# VDI mouse cursor + exchange-vector functions (Phase 00b D6)

> Source: EmuTOS @ `1a7484ab`. Verified against:
> `vdi/vdi_mouse.c:513` (`vdi_vsc_form`) → `:1138` (`linea_transform_mouse`)
> → `:450` (`set_mouse_form`); `:321` (`vdi_v_show_c`) → `:1125`
> (`linea_show_mouse`) → `:127` (`dis_cur`); `:331` (`vdi_v_hide_c`) →
> `:1133` (`linea_hide_mouse`) → `:157` (`hide_cur`); `:341`
> (`vdi_vq_mouse`); `:370`/`:391`/`:414` (`vdi_vex_butv`/`motv`/`curv`);
> `vdi/vdi_misc.c:100` (`vdi_vex_timv`); `vdi/vdi_mouse.c:438`
> (`vdi_vex_wheelv`, gated); MFORM struct `include/aesdefs.h:441-450`;
> `validate_color_index` `vdi/vdi_control.c:224`; `Tickcal()` = BIOS 0x06
> (`include/biosbind.h:25`). Opcode numbers from
> [`../_dispatcher.md`](../_dispatcher.md) jump table 2.
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md).
>
> **Contract vs. reference** — see [`../../../COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract** (must match TOS exactly): the opcode numbers, the 37-WORD
> MFORM `INTIN` layout, the hotspot/colour constraint rules, the hide-count
> semantics of show/hide, the `vq_mouse` `INTOUT`/`PTSOUT` output, and the
> `vex_*` "new in `CONTRL[7..8]`, old out in `CONTRL[9..10]`" exchange
> shape. **Reference** (NewTOS reimplements freely): `set_mouse_form`'s
> interleaved `Mcdb.maskdata[]`, the Line-A `mouse_cdb`/`mcs_ptr` storage,
> `cur_display`/`cur_replace` sprite rasterisation, and the VBL draw path —
> NewTOS uses its own `RasterBackend` (`tos-api/src/vdi/mouse.rs`,
> `backend.rs`).

| Op | Fn | Input | Output | Effect |
|----|----|-------|--------|--------|
| 111 | `vsc_form`  | `INTIN[0..36]` = MFORM (37 WORDs) | — | set cursor shape |
| 122 | `v_show_c`  | `INTIN[0]` = reset flag | — | show cursor / decrement hide-count |
| 123 | `v_hide_c`  | — | — | hide cursor / increment hide-count |
| 124 | `vq_mouse`  | — | `INTOUT[0]`=buttons; `PTSOUT[0..1]`=x,y | query mouse |
| 125 | `vex_butv`  | `CONTRL[7..8]`=new routine | `CONTRL[9..10]`=old | install button vector |
| 126 | `vex_motv`  | `CONTRL[7..8]`=new routine | `CONTRL[9..10]`=old | install motion vector |
| 127 | `vex_curv`  | `CONTRL[7..8]`=new routine | `CONTRL[9..10]`=old | install cursor-draw vector |
| 118 | `vex_timv`  | `CONTRL[7..8]`=new routine | `CONTRL[9..10]`=old; `INTOUT[0]`=ms/tick | install timer vector |
| 134 | `vex_wheelv`| `CONTRL[7..8]`=new routine | `CONTRL[9..10]`=old | install wheel vector (**`CONF_WITH_EXTENDED_MOUSE` only**) |

None of these has an error return (VDI convention: silent no-op on a bad
handle, per the dispatcher).

---

## `vsc_form` (111) — SET CURSOR FORM (install MFORM)

**Primary location:** `vdi/vdi_mouse.c:513` (`vdi_vsc_form`) →
`linea_transform_mouse()` (`:1138`) → `set_mouse_form(INTIN, &mouse_cdb)`
(`:450`).
**Helpers:** `validate_color_index()` (`vdi_control.c:224`) — clamp a
colour index; `MAP_COL[]` — pen → hardware PEL.
**Globals read:** `MAP_COL[]`, `numcolors` (via `validate_color_index`).
**Globals written:** `mouse_cdb` (the sprite); `mouse_flag`
(bracketed +1/−1 to suppress cursor redraw while redefining).
**Build config:** none.

### Parameter-block interface

`INTIN[0..36]` is a 37-WORD `MFORM` (`include/aesdefs.h:441-450`):

| INTIN index | MFORM field | Meaning |
|:-----------:|-------------|---------|
| 0 | `mf_xhot` | hot-spot x (0..15; masked `& 0x0f`) |
| 1 | `mf_yhot` | hot-spot y (0..15; masked `& 0x0f`) |
| 2 | `mf_nplanes` | reserved, must be 1 (**ignored** by `set_mouse_form`) |
| 3 | `mf_bg` | **mask** colour index (background) |
| 4 | `mf_fg` | **data** colour index (foreground) |
| 5..20 | `mf_mask[0..15]` | 16 mask rows (16 bits each, MSB = leftmost pixel) |
| 21..36 | `mf_data[0..15]` | 16 data rows |

`CONTRL[3]` (`N_INTIN`) = 37; `CONTRL[1]`/`N_PTSIN` = 0; no outputs
(`{nptsout=0, nintout=0}`).

### Behaviour

`set_mouse_form` (`vdi_mouse.c:450-492`):

1. Set `mouse_flag += 1` to disable cursor updates during the redefine.
2. **Hotspot constrained** to 0..15: `xhot = mf_xhot & 0x0f`,
   `yhot = mf_yhot & 0x0f` (`:461-464`) — values above 15 wrap, never
   overflow the 16×16 sprite.
3. **Colours mapped** through `validate_color_index` then `MAP_COL`:
   `bg_col = MAP_COL[validate_color_index(mf_bg)]`, likewise `fg_col`
   (`:466-472`). `validate_color_index` returns **1** for any index `< 0`
   or `≥ numcolors` (`vdi_control.c:224-230`) — an out-of-range mask/data
   colour silently becomes pen 1.
4. **Interleave** the 16 mask + 16 data rows into `Mcdb.maskdata[32]` as
   `mask[0], data[0], mask[1], data[1], …` (`:482-489`) — a **reference**
   storage detail (device-specific sprite format); the contract is only the
   37-WORD `INTIN` input.
5. `mouse_flag -= 1` re-enables drawing.

The default installed at init is the arrow `arrow_mform` (hotspot 1,0;
`mf_bg=1`, `mf_fg=0` — but only in the `!WITH_AES` build; with AES the AES
supplies the default form) (`vdi_mouse.c:69-110,593`).

### Verification criteria

- `vsc_form` with hotspot `(3,5)`, `mf_bg=1`, `mf_fg=0`, a known 16-row
  mask/data → the installed sprite has `xhot=3`, `yhot=5`,
  `bg_col=MAP_COL[1]`, `fg_col=MAP_COL[0]`, and its 16×16 shape matches the
  supplied rows.
- `vsc_form` with `mf_bg = 99` on a 16-colour device → the mask colour
  clamps to pen 1 (`MAP_COL[1]`).

---

## Cursor visibility — `v_show_c` (122) / `v_hide_c` (123)

**Primary locations:** `vdi_mouse.c:321` (`vdi_v_show_c`) → `linea_show_mouse`
(`:1125`) → `dis_cur` (`:127`); `vdi_mouse.c:331` (`vdi_v_hide_c`) →
`linea_hide_mouse` (`:1133`) → `hide_cur` (`:157`).
**Globals read/written:** `HIDE_CNT` (the hide-depth counter); `draw_flag`.
**Build config:** none.

### Parameter-block interface

| Slot | Meaning |
|------|---------|
| `CONTRL[0]` | opcode 122 (`v_show_c`) or 123 (`v_hide_c`) |
| `INTIN[0]` | (`v_show_c` only) **reset flag**: 0 = force show |
| — | no outputs (`{0,0}`) |

### Behaviour — hide-count model (contract)

TOS tracks a **hide depth** `HIDE_CNT`; the cursor is visible only when it
reaches 0. Show and hide are counted so nested hide/show pairs balance:

- **`v_hide_c`** (`hide_cur`, `:157-169`): `HIDE_CNT += 1`; if it becomes
  exactly 1 (was 0 → visible), physically remove the cursor
  (`cur_replace`). Further hides just deepen the count without redrawing.
- **`v_show_c`** (`dis_cur`, `:127-141`): if `HIDE_CNT != 1`, decrement it
  and, **if it went negative, clamp to 0** (`:132-133`) — so
  **`v_show_c` underflow is silently absorbed**, never uncovering the
  cursor more than a matching hide would. If `HIDE_CNT` is exactly 1, this
  is the last outstanding hide: physically display the cursor
  (`cur_display`) and decrement to 0.

### The reset flag (`v_show_c` `INTIN[0]`)

`linea_show_mouse` (`:1125-1131`): **if `INTIN[0] == 0` and the cursor is
currently hidden (`HIDE_CNT != 0`), force `HIDE_CNT = 1` before calling
`dis_cur`** — i.e. a "reset" show collapses any hide depth to a single
level so the following `dis_cur` immediately makes the cursor visible.
A non-zero `INTIN[0]` performs a normal counted show. (`v_dspcur`, escape
sub 18, calls `v_show_c` with `INTIN[0]` pre-set to 0 for exactly this
force-show, [`../_escape.md`](../_escape.md).)

### Verification criteria

- After init (`HIDE_CNT = 1`, hidden), `v_show_c(INTIN[0]=1)` → `HIDE_CNT =
  0`, cursor shown.
- `v_hide_c` ×3 then `v_show_c` ×2 → `HIDE_CNT = 1`, still hidden (nesting
  balances).
- `v_show_c` with `HIDE_CNT = 0` → stays 0 (underflow clamped, no double
  show).
- `v_show_c(INTIN[0]=0)` while `HIDE_CNT = 5` → forces to 1 then shows
  (reset-show).

---

## `vq_mouse` (124) — QUERY POSITION + BUTTONS

**Primary location:** `vdi/vdi_mouse.c:341` (`vdi_vq_mouse`).
**Globals read:** `MOUSE_BT`, `GCURX`, `GCURY` (read with interrupts
masked, `set_sr(0x2700)`, for an atomic snapshot).
**Build config:** none.

### Parameter-block interface

| Slot | Meaning |
|------|---------|
| `CONTRL[0]` | opcode = 124 |
| `CONTRL[2]` (`N_PTSOUT`) | 1 (dispatcher pre-load) |
| `CONTRL[4]` (`N_INTOUT`) | 1 (dispatcher pre-load) |
| `INTOUT[0]` | out: button bitmask (`MOUSE_BT`) |
| `PTSOUT[0]`, `PTSOUT[1]` | out: cursor x, y (`GCURX`, `GCURY`) |

### Behaviour

Atomic read (`vdi_mouse.c:343-351`): `INTOUT[0] = MOUSE_BT`,
`PTSOUT[0] = GCURX`, `PTSOUT[1] = GCURY`. Button bits: 0 = left, 1 = right
(plus 2-4 for extended buttons under `CONF_WITH_EXTENDED_MOUSE`), per the
VDI/GEM convention — see [`_mouse-packet.md`](_mouse-packet.md) for the
IKBD→`MOUSE_BT` bit swap. Position is the clamped `GCURX`/`GCURY`
maintained by the packet handler.

### Verification criteria

- With `MOUSE_BT = 0x01`, `GCURX = 100`, `GCURY = 50` → `INTOUT[0] = 1`,
  `PTSOUT[0] = 100`, `PTSOUT[1] = 50`.
- No buttons down → `INTOUT[0] = 0`.

---

## Exchange vectors — `vex_butv`/`motv`/`curv` (125-127), `vex_timv` (118), `vex_wheelv` (134)

**Primary locations:** `vdi_mouse.c:370` (`vex_butv`), `:391` (`vex_motv`),
`:414` (`vex_curv`), `:438` (`vex_wheelv`, gated); `vdi_misc.c:100`
(`vex_timv`).
**Globals written:** the matching vector slot (below).
**Build config:** `vex_wheelv` only exists when `CONF_WITH_EXTENDED_MOUSE`
is defined (`vdi_mouse.c:422-443`; opcode 134 also gated in the
dispatcher).

### The exchange shape (contract, all five identical)

Each call **installs a new routine and returns the previous one** so the
caller can chain it:

| Slot | Meaning |
|------|---------|
| `CONTRL[7]`, `CONTRL[8]` | in: pointer to the new routine (32-bit, high/low WORDs) |
| `CONTRL[9]`, `CONTRL[10]` | out: pointer to the previous routine |

Implementation (`vdi_mouse.c:372-373`, identical for each): write the old
global into `CONTRL[9..10]`, then load the new pointer from `CONTRL[7..8]`
into the global. `vex_timv` and `vex_butv`/`motv`/`curv` differ only in
which global they swap and (for `vex_timv`) an extra output.

| Fn | Op | Global swapped | Called when | Init value |
|----|:--:|----------------|-------------|-----------|
| `vex_butv` | 125 | `user_but` | button state changes (from `mouse_int`) | `just_rts` |
| `vex_motv` | 126 | `user_mot` | cursor coordinate changes | `just_rts` |
| `vex_curv` | 127 | `user_cur` | cursor is (re)drawn | `mov_cur` |
| `vex_timv` | 118 | `tim_addr` | Timer-C tick (via `tick_int`) | `just_rts` |
| `vex_wheelv` | 134 | `user_wheel` | wheel movement (`CONF_WITH_EXTENDED_MOUSE`) | `just_rts` |

The button/motion/cursor vectors are called from the IKBD mouse packet
handler in the exact order button → motion → cursor — see
[`_mouse-packet.md`](_mouse-packet.md).

### `vex_timv` extra output (`vdi_misc.c:100-112`)

Beyond the vector swap, `vex_timv` also sets **`INTOUT[0] = Tickcal()`** —
the number of milliseconds between Timer-C ticks (`Tickcal` = BIOS 0x06).
`CONTRL[4]` (`N_INTOUT`) is 1 (dispatcher pre-load). The swap runs with
interrupts masked (`set_sr(0x2700)`). The installed `tim_addr` is invoked
by `tick_int`, which also chains the old system timer vector
(`vdi_misc.c:78-90`).

### Verification criteria

- `vex_butv(new=P)` → `user_but` becomes `P`; `CONTRL[9..10]` holds the
  previous `user_but` (e.g. `just_rts` right after init).
- `vex_timv(new=P)` → `tim_addr` becomes `P`; `CONTRL[9..10]` = old
  `tim_addr`; `INTOUT[0]` = ms/tick from `Tickcal()`.
- Two successive `vex_curv` installs → the second call's `CONTRL[9..10]`
  returns the first call's pointer (chaining works).

---

## Edge cases discovered in source

- **`v_show_c` underflow is clamped, not wrapped** (`dis_cur:132-133`):
  extra shows never make `HIDE_CNT` negative, so an unbalanced show cannot
  "over-uncover" the cursor.
- **`v_show_c` reset flag collapses the hide depth** to 1 before showing
  (`linea_show_mouse:1127-1128`) — a force-show regardless of how deeply
  nested the hides were.
- **Out-of-range MFORM colour → pen 1**, silently
  (`validate_color_index:226-227`), not an error.
- **`vsc_form` ignores `mf_nplanes`** (`INTIN[2]`): the "reserved, must be
  1" field is never read by `set_mouse_form`.
- **`vex_*` return the raw 32-bit pointer** in `CONTRL[9..10]` even if the
  vector was never changed (returns the init default `just_rts`/`mov_cur`).

---

## Dependencies

### Requires

- **Mouse packet handler** feeding `GCURX`/`GCURY`/`MOUSE_BT`/`cur_ms_stat`
  and calling `user_but`/`user_mot`/`user_cur`
  ([`_mouse-packet.md`](_mouse-packet.md)).
- **Timer-C tick** for `vex_timv`/`tim_addr` (Phase-07 system timer).
- The workstation must be open (`CUR_WORK`) so `MAP_COL[]`/`numcolors`
  exist for `vsc_form`.

### Used by

- `v_dspcur`/`v_rmcur` (escape sub 18/19) call `v_show_c`/`v_hide_c`
  ([`../_escape.md`](../_escape.md)).
- The AES installs `user_but`/`user_mot` via `vex_butv`/`vex_motv` to drive
  `evnt_button`/`evnt_mouse`.

---

## Related data structures

- **MFORM (37-WORD)** — table above; struct `include/aesdefs.h:441-450`.
- **Mouse-packet / button-bit / `cur_ms_stat` layout** →
  [`_mouse-packet.md`](_mouse-packet.md).
- **Shared cursor/vector globals** (`HIDE_CNT`, `GCURX/Y`, `MOUSE_BT`,
  `user_but/mot/cur`, `tim_addr`, `mouse_cdb`, `mcs_ptr`) →
  [`../_state.md`](../_state.md#input-and-cursor-state).

---

## Cross-references

- **Atari Compendium / Profibuch:** cross-check of the MFORM layout, the
  hide-count semantics, and the `vex_*` ABI is **deferred** — no PDFs in
  `references/` (see [`../_discrepancies.md`](../_discrepancies.md)).
- **XBIOS:** `vsc_form` cursor delivery and the button/motion vectors
  depend on the IKBD mouse set up by `Initmous`
  ([`../../xbios/00-initmous.md`](../../xbios/00-initmous.md)); the
  keyboard/mouse vector base is `Kbdvbase`.

---

## Configuration / version notes

- `vex_wheelv` (op 134) and extended mouse buttons 3-5 exist only under
  `CONF_WITH_EXTENDED_MOUSE`; without it, opcode 134 is a dispatcher no-op
  and `MOUSE_BT` has only bits 0-1.
- `CONF_WITH_VDI_16BIT` selects a separate `cur_display16`/`cur_replace16`
  sprite path (TrueColor) — a **reference** rasterisation detail, invisible
  to the ABI.

## NewTOS implementation notes

- Cursor + button + position come from the **input subsystem** (USB-HID /
  IKBD), not the framebuffer. For QEMU bring-up the source is the
  virtio/USB-HID device; until wired, `vq_mouse` returns a parked cursor and
  zero buttons (documented limitation — NOT a stub returning fake motion).
- `vsc_form` installs the 37-WORD MFORM into the NewTOS `RasterBackend`
  sprite (`tos-api/src/vdi/mouse.rs`); the interleaved `Mcdb` layout is an
  EmuTOS detail — NewTOS may store the sprite however it likes, as long as
  the hotspot/colour constraint rules (mask `& 0x0f`, out-of-range → pen 1)
  are reproduced.
- The hide-count (`HIDE_CNT`) with underflow-clamp and reset-flag force-show
  is **contract** — reimplement exactly; nested AES hide/show relies on it.
- `vex_*` vectors are real function-pointer slots the IRQ/timer path calls;
  install/return semantics must be exact (programs chain the old vector).
  Pointer ABI is 32-bit for m68k callers; native callers use the NewTOS
  pointer width (see the trap pointer mapping).
- `vex_timv` ties into the system tick (Phase-07 timer); `Tickcal` (BIOS
  0x06) returns the tick period in ms for `INTOUT[0]`.
