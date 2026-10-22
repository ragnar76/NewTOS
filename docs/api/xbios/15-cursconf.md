# `Cursconf` (XBIOS 0x15)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/vt52.c:893` (`cursconf`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the 6 subfunctions, the WORD return (only subfn 5
> returns the rate). Reference: the `v_stat_0`/`v_period` VT52 cursor state.

## EmuTOS source

**Primary location:** `bios/vt52.c:893` — `WORD cursconf(WORD function, WORD
operand)`, slot 0x15 of `xbios_vecs[]` (`bios/xbios.c:1311`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:**
- `bios/vt52.c` — `cursor_off()` / `cursor_on()` — hide/show via the
  `disab_cnt` nesting counter.

**Headers / type definitions used:**
- `bios/conout.h` — `M_CFLASH` (0x0001), `M_CSTATE` (0x0002), `M_CVIS`
  (0x0004); `v_stat_0`, `v_period`.
- `include/portab.h` — `LOBYTE(x)`.

**System variables / globals read:**
- `v_period` (returned by subfn 5).

**System variables / globals written:**
- `v_stat_0` (subfns 2/3, blink bit), `v_period` (subfn 4), `disab_cnt` /
  `v_stat_0` (subfns 0/1 via `cursor_off`/`cursor_on`).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Cursconf` configures the VT52 text cursor. The `function` selects one of six
sub-operations, `operand` is used only by subfunction 4:

- **0 hide** — `cursor_off()` (increments the `disab_cnt` nesting counter).
- **1 show** — `cursor_on()`.
- **2** — clear `M_CFLASH` in `v_stat_0`: the cursor stops flashing
  (steady). The source comment says "blinking cursor"; the code does the
  opposite (see edge case 2).
- **3** — set `M_CFLASH`: the cursor flashes. Comment: "not blinking".
- **4 set rate** — `v_period = LOBYTE(operand)` (blink interval in VBL ticks).
- **5 get rate** — `return v_period`.

Subfunctions 0-4 (and any unhandled `function`) return `0`; only subfunction
5 returns a value (the current blink rate). The blink itself runs in the VBL
`blink()` routine, which decrements a timer and toggles `M_CSTATE` when it
reaches `v_period`; `Cursconf` only sets the parameters/flags. `blink()`
flashes only while `M_CFLASH` is **set** (`bios/vt52.c:855-857`, and
`bios/conout.h:17`: "cursor flash 0:disabled 1:enabled"), so subfunction 2
makes the cursor **steady** and 3 makes it **blink** — the reverse of the
comments in `cursconf` and of the Atari documentation's names. Measured on
the reference (CON04, below).

`Cursconf` does not touch `conterm` (the bell/click/repeat byte).

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x15
- **Stack frame on entry (m68k):** `Cursconf(function, operand)` =
  `xbios_w_ww(21, a, b)` (`include/xbiosbind.h:39`):
  ```
  +4: operand  (word)
  +2: function (word)
   0: function_id (word) = 0x0015
  ```
- **Total parameter bytes:** 6 (W + W).
- **Native (AArch64) ABI:** ID `0x15` in `x8`; args in `x0..x1`; the WORD
  result **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name       | Type / size |
  |------------|-------------|------------|-------------|
  | +2         | `x0`        | `function` | `WORD` (16) — subfunction 0..5 |
  | +4         | `x1`        | `operand`  | `WORD` (16) — blink rate (subfn 4 only) |

---

## Inputs

| Position | Name       | Type   | Size | Source location   | Notes |
|----------|------------|--------|------|-------------------|-------|
| arg0     | `function` | `WORD` | 2    | `bios/vt52.c:895` | subfunction selector 0..5 |
| arg1     | `operand`  | `WORD` | 2    | `bios/vt52.c:909` | new blink rate for subfn 4 (`LOBYTE` used) |

### Subfunction table (`bios/vt52.c:895-913`)

| `function` | Action | Global changed | Return |
|-----------:|--------|----------------|--------|
| 0 | hide cursor | `cursor_off()` (`disab_cnt`++/`M_CVIS`) | 0 |
| 1 | show cursor | `cursor_on()` (`M_CVIS`) | 0 |
| 2 | flash off (steady) — commented "blinking" | `v_stat_0 &= ~M_CFLASH` | 0 |
| 3 | flash on (blinking) — commented "not blinking" | `v_stat_0 \|= M_CFLASH` | 0 |
| 4 | set blink rate | `v_period = LOBYTE(operand)` | 0 |
| 5 | get blink rate | (none) | `v_period` |
| other | (falls through) | (none) | 0 |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** WORD
- **Encoding:** `v_period` for subfunction 5; `0` for every other subfunction
  (and unhandled selectors).
- **Source location:** `bios/vt52.c:912,914`.

### Side effects on global state

- Per the subfunction table above (`v_stat_0`, `v_period`, `disab_cnt`).

### Side effects on hardware

- None directly; the VBL `blink()` renders the cursor per `v_stat_0`/`v_period`.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

An out-of-range `function` falls through the switch and returns `0`.

---

## Verification criteria

- **Get rate:** `Cursconf(5, 0)` returns the current `v_period` and changes
  nothing.
- **Set rate round-trips:** `Cursconf(4, 20)` then `Cursconf(5, 0)` returns
  `20`.
- **Hide/show:** `Cursconf(0, 0)` hides the cursor (returns 0); `Cursconf(1,
  0)` shows it.
- **Blink toggle:** `Cursconf(2, 0)` makes the cursor steady (clears
  `M_CFLASH`); `Cursconf(3, 0)` makes it blink (sets `M_CFLASH`). Both
  return 0.

---

## Edge cases discovered in source

1. **Only subfunction 5 returns a value.** Every other subfunction returns
   `0`, so a program cannot read back the visibility/blink flags — only the
   rate (`bios/vt52.c:911-914`).
2. **Subfunctions 2 and 3 do the opposite of their comments.**
   `function == 2` **clears** `M_CFLASH` and `function == 3` **sets** it
   (`bios/vt52.c:902-906`); `M_CFLASH` set means "flash enabled"
   (`conout.h:17`, `blink()` returns early without it, `vt52.c:855-857`,
   and `vt52_init` sets it for the blinking power-up cursor, `:932`). So 2
   gives a steady cursor and 3 a blinking one. *Correction 2026-10-22
   (CON04):* this entry said "flash bit set = flash disabled", which
   contradicts `blink()`. Measured on the pinned EmuTOS on Hatari Falcon
   (`measurements/measurement-2026-10-22-con04/`): after `ESC e` +
   `Cursconf(2,0)` six screenshots 0.3 s apart all show the cursor; with
   `Cursconf(3,0)` it alternates.
3. **Hide/show is nested via `disab_cnt`.** `cursor_off` increments a disable
   counter; balanced `cursor_on` calls are needed to re-show — subsystems can
   independently hide/show without fighting.
4. **`operand` is byte-truncated for the rate.** `v_period = LOBYTE(operand)`
   (`:909`) — only the low 8 bits set the rate.
5. **Does not touch `conterm`.** Cursor config and the bell/click/repeat byte
   are unrelated (`conterm` is read only in `bios/ikbd.c`/`bios/vt52.c`
   bell/keyboard paths).

---

## Dependencies

### Requires (must be initialized / functional)

- The VT52 cursor state (`v_stat_0`, `v_period`) and the VBL `blink()` routine.

### Used by

- Text-mode UIs that hide the cursor during redraws, setup tools adjusting
  the blink rate.

---

## Related data structures

- `docs/api/xbios/_state.md §4` — display/VT52 state.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the 6
  subfunctions; the "flash bit set = steady" sense is EmuTOS's.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- No `CONF_WITH_*` gate; the cursor is a VT52 software feature present on all
  builds.

---

## NewTOS implementation notes

(Empty during Phase 00.)

CON04 (2026-10-22): `Cursconf` drives the VT52 console's own state
(`tos-api/src/vdi/escape.rs` `cursconf`, called from
`tos-api/src/xbios/screen.rs`), the same `v_stat_0`/`disab_cnt`/`v_period`
that `ESC e`/`ESC f` change; the cursor is drawn (cell inversion) and blinks
from the 200 Hz timer (`docs/api/bios/_vt52.md` §6, decision Q5). Subfunction
semantics exactly as the EmuTOS code above, including 2 = steady.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (the Shifter-era VT52 cursor state) at
> the NewTOS framebuffer VT52 console (`fbcon`). The **contract is
> unchanged** — the 6 subfunctions, only subfn 5 returns the rate, the
> inverted `M_CFLASH` sense. Peripheral + driver: the framebuffer VT52
> console over the VideoCore buffer (`docs/HARDWARE-PROFILE.md` §2/§3.2).

- **The console's cursor (since CON04, 2026-10-22).** `cursconf` reaches
  the VT52 console state in `tos-api/src/vdi/escape.rs`: 0 `cursor_off`
  (`disab_cnt`++), 1 `cursor_on` (forced), 2 clear / 3 set `M_CFLASH`,
  4 `v_period = LOBYTE(operand)`, 5 return `v_period`, all else 0. The
  flag bits are EmuTOS' (`M_CFLASH 0x01`, `M_CVIS 0x04`); they are internal
  either way — only subfunction 5's rate is returned.
- **No Atari hardware dependency.** There is no Shifter or MFP-driven
  blink: the kernel's 200 Hz timer interrupt runs the blink step, one
  `v_period` unit = 1/60 s, default 30 = 0.5 s (`docs/COMPATIBILITY.md`,
  CON04 Q5).
- **1080p later.** Cursor config is geometry-independent.
