# `Cursconf` — XBIOS 0x15

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/vt52.c:893` (`cursconf`).

## EmuTOS source

```c
WORD cursconf(WORD function, WORD operand)
{
    switch (function) {
    case 0:  cursor_off();              break;   /* hide */
    case 1:  cursor_on();               break;   /* show */
    case 2:  v_stat_0 &= ~M_CFLASH;     break;   /* disable flash */
    case 3:  v_stat_0 |=  M_CFLASH;     break;   /* enable flash */
    case 4:  v_period = LOBYTE(operand); break;  /* set blink rate */
    case 5:  return v_period;                    /* get blink rate */
    }
    return 0;
}
```

`v_stat_0` and `v_period` are line-A globals; `M_CFLASH` is bit 1
of `v_stat_0` (`vt52.h`). `cursor_on/off()` (file-local) update
`M_CVIS` (bit 4) and immediately repaint.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x15` |
| WORD args | 2 (WORD + WORD) |
| Stack frame | `+4: operand (W)`, `+2: function (W)`, `+0: 0x0015` |
| Vector | `xbios_vecs[21]` |

## Inputs

| Pos | Name      | Type | Range / meaning                                |
|-----|-----------|------|------------------------------------------------|
| 0   | `function` | WORD | 0..5 (sub-function selector)                  |
| 1   | `operand` | WORD | only used when `function == 4` (rate, low byte) |

### Sub-functions

| `function` | Action               | `operand` use | Returns       |
|-----------:|----------------------|---------------|---------------|
| 0          | hide cursor          | ignored       | 0             |
| 1          | show cursor          | ignored       | 0             |
| 2          | disable cursor flash | ignored       | 0             |
| 3          | enable cursor flash  | ignored       | 0             |
| 4          | set flash period     | period (low byte, in VBLs) | 0    |
| 5          | get flash period     | ignored       | current `v_period` |
| ≥ 6        | (no-op)              | ignored       | 0             |

`v_period` is in VBL units (50/60 Hz). Default is 30
(≈600 ms / 500 ms).

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended WORD) |

Sub-function-dependent — only function 5 returns useful data;
others return 0.

## Errors

None. Out-of-range `function` is a silent no-op.

## Edge cases

### 1. `operand` clamping

`v_period = LOBYTE(operand)` keeps only the low byte. A program
passing 0x0100 sets `v_period = 0` — instant flash. Programs
should pass values 0..255.

### 2. `v_period == 0` semantics

Cursor flashes at the highest possible rate (one toggle per VBL).
Practically invisible-fast cursor — undefined in Compendium but
real on hardware.

### 3. Hide vs disable-flash interaction

`cursor_off()` clears `M_CVIS` (visible bit). `function 2/3` only
touches `M_CFLASH`. So a hidden cursor with flash enabled stays
hidden — `M_CVIS` dominates.

### 4. Repaint timing

`cursor_on/off()` immediately writes to `v_bas_ad`. If the cursor
glyph crosses a screen-region boundary while VT52 init is in
progress, line-A may corrupt. EmuTOS does not guard against this.

### 5. `v_stat_0` is line-A territory

Programs that bypass `Cursconf` and write `v_stat_0` directly
must coordinate with the VBL cursor-flash handler — race-prone.

## Dependencies

- `v_stat_0`, `v_period`, `v_bas_ad`, `cursor_on/off` (line-A /
  vt52 globals).

Used by: VT52 escape sequences (`Esc e/f/p/q/r/y`), AES cursor
control, GEM mouse-vs-text-cursor coordination.

## Cross-references

- **Compendium:** "Cursconf" — agrees on sub-functions.
- **Profibuch:** "Cursconf (0x15)" — agrees.
- `docs/api/structures/osheader.md` — line-A pointer.

## Configuration

- `CONF_WITH_VT52` — required (NewTOS profile yes).

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: cursor flash is driven from the framebuffer compositor
at 60 Hz. `v_period` units stay VBL-equivalent for ABI compat.
