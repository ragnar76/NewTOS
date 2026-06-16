# TT shifter group — XBIOS 0x50..0x57

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:180..354` (`esetshift`, `egetshift`, `esetbank`,
> `esetcolor`, `esetpalette`, `egetpalette`, `esetgray`, `esetsmear`).
>
> NewTOS profile enables `CONF_WITH_TT_SHIFTER`, so these eight
> functions are present in the dispatch table. The Atari TT shifter
> hardware does not exist on RPi4; semantics are emulated against
> the framebuffer.

This group spec covers all eight TT-shifter calls in one file
because they share state, addressing, and the "no hardware →
return function number" pattern.

| ID    | Name        | EmuTOS func    | Args                                  |
|------:|-------------|----------------|---------------------------------------|
| 0x50  | EsetShift   | `esetshift`    | mode:WORD                             |
| 0x51  | EgetShift   | `egetshift`    | —                                     |
| 0x52  | EsetBank    | `esetbank`     | bank:WORD                             |
| 0x53  | EsetColor   | `esetcolor`    | index:WORD, color:UWORD                |
| 0x54  | EsetPalette | `esetpalette`  | index:WORD, count:WORD, rgb:UWORD *    |
| 0x55  | EgetPalette | `egetpalette`  | index:WORD, count:WORD, rgb:UWORD *    |
| 0x56  | EsetGray    | `esetgray`     | mode:WORD                             |
| 0x57  | EsetSmear   | `esetsmear`    | mode:WORD                             |

## Common pattern

Every function starts with:

```c
if (!has_tt_shifter)
    return 0xNN;        /* function number — see D-008 */
```

Then accesses MMIO at `TT_SHIFTER` (`0xFFFF8262`) and
`TT_PALETTE_REGS` (`0xFFFF8400`, 256 × UWORD).

## EmuTOS source (representative excerpts)

```c
WORD esetshift(WORD mode)
{
    if (!has_tt_shifter) return 0x50;
    vsync();                                /* avoid mid-frame change */
    oldmode = *resreg & TT_SHIFTER_BITMASK;
    *resreg = mode & TT_SHIFTER_BITMASK;
    vt52_init();
    return oldmode;
}

WORD esetcolor(WORD index, UWORD color)
{
    if (!has_tt_shifter) return 0x53;
    index &= 0xff;
    oldcolor = TT_PALETTE_REGS[index] & TT_PALETTE_BITMASK;
    if ((WORD)color >= 0)                   /* color < 0 = query */
        TT_PALETTE_REGS[index] = color & TT_PALETTE_BITMASK;
    return oldcolor;
}
```

Same pattern, varying register / bit field, for the other six.

## Trap interface (common)

Standard XBIOS. Stack frames per function — see Compendium for the
exact byte layout. Vector indices `0x50..0x57` correspond to
`xbios_vecs[80..87]`.

## TT shifter mode (resreg `0xFF8262`)

Low 8 bits encode resolution × interlace × sync source:

| `mode`  | Resolution                             |
|--------:|----------------------------------------|
| 0x0000  | TT low (320 × 480, 256 colours)         |
| 0x0100  | TT medium (640 × 480, 16 colours)       |
| 0x0200  | TT high (1280 × 960, 2 colours mono)    |
| 0x0400  | ST low (320 × 200, 16 colours)          |
| 0x0500  | ST medium (640 × 200, 4 colours)        |
| 0x0600  | ST high (640 × 400, 2 colours)          |

`TT_SHIFTER_BITMASK = 0x0700` (only bits 8..10 are valid).

## TT palette format

256 entries of UWORD at `0xFF8400`. Format `0x0FFF` (4 bits per
channel, `xRRRRGGGGBBBB`). Mask `TT_PALETTE_BITMASK = 0x0FFF`.

## Per-function semantics

### `EsetShift(mode)` (0x50)

Set TT resolution. `vsync()` first to avoid mid-frame plane shift.
Re-runs `vt52_init()` because cell dimensions change. Returns
previous mode.

### `EgetShift()` (0x51)

Read current TT shifter register (no side effects). Returns
masked value.

### `EsetBank(bank)` (0x52)

In TT low-res 256-colour mode, only 16 of the 256 palette entries
are visible at once; `bank` selects which 16. `bank == -1` is the
documented query (any negative value is treated as query because
of `if (bank >= 0)` write guard). Returns previous bank (low 4
bits).

### `EsetColor(index, color)` (0x53)

Single-entry palette write. `color < 0` is query (`(WORD)color`
checks signed). Returns previous color value.

### `EsetPalette(index, count, rgb)` (0x54)

Bulk palette write. Clamps `index+count` to `≤ 256`. Always
returns 0 on success (atypical — the Atari spec says void; EmuTOS
returns WORD because that's what `xbios_unimpl` would return on
no-hardware fallback).

### `EgetPalette(index, count, rgb)` (0x55)

Bulk palette read. Same clamping. Returns 0.

### `EsetGray(mode)` (0x56)

Toggle TT-shifter grey-mode bit (bit 4 of `*shiftreg`).
- `mode > 0` → set bit 4.
- `mode == 0` → clear bit 4.
- `mode < 0` → query only.

Returns 1 if previously enabled, else 0.

### `EsetSmear(mode)` (0x57)

Same pattern as `EsetGray`, but bit 7 of `*shiftreg` (TT high-res
"smear" anti-aliasing).

## Outputs

Per-function — see semantics above.

## Errors

None at the XBIOS level. `has_tt_shifter == 0` returns the
function number (D-008 pattern).

## Edge cases

### 1. Vsync before shift change

Only `EsetShift` waits — the other seven write registers without
synchronisation. Programs changing palette in the middle of a
frame see partial updates.

### 2. `index & 0xff` — silent wraparound

`EsetColor`/`EsetPalette`/`EgetPalette` mask the index to 8 bits.
`index = 256` becomes `index = 0`. Programs passing out-of-range
indices silently overwrite earlier entries.

### 3. `count` over-run protection

`if ((index+count) > 256) count = 256 - index;` — silently
truncates rather than erroring. Programs reading `egetpalette(250,
100, buf)` get only 6 entries written.

### 4. Bit-mask preservation

Read-modify-write reads the **masked** previous value and writes
the **masked** new value, leaving high bits zero. NewTOS emulation
must respect this.

### 5. Falcon overlap

XBIOS `Vsetmode`/`Vsetrgb` (Falcon Videl) live at higher numbers
(0x58+). They are not in the NewTOS dispatch table
(LAST_ENTRY=0x57). Programs Falcon-detecting via `Vsetmode` see
EINVFN; programs TT-detecting via `EsetShift` get a real answer.

## NewTOS emulation strategy

The TT-shifter family is preserved as **ABI surface only** — the
underlying display hardware is the BCM2711 framebuffer. NewTOS:

1. Maintains a software shadow of `TT_SHIFTER` and
   `TT_PALETTE_REGS` (256 × UWORD).
2. Translates the shadow on every change to a VideoCore mailbox
   `set_palette` call and (for `EsetShift`) a framebuffer
   re-config.
3. Reports `has_tt_shifter = 1` so programs do not see the
   "function number" D-008 path.

## Dependencies

- `has_tt_shifter` (boot detection).
- `TT_SHIFTER`, `TT_PALETTE_REGS` MMIO addresses.
- `vsync()`, `vt52_init()`.

Used by: TT-aware applications, scientific software designed for
TT high-res, palette-heavy demos.

## Cross-references

- **Compendium:** all eight functions documented; agrees.
- **Profibuch:** "Eset*-Familie" — agrees, lists palette format.
- `docs/api/xbios/05-setscreen.md` — overlapping concerns at TT
  resolutions.
- `docs/api/xbios/06-setpalette.md`, `07-setcolor.md` — ST-era
  counterparts.

## Configuration

- `CONF_WITH_TT_SHIFTER` — enabled in NewTOS.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: NewTOS exposes the TT shifter as ABI-only with software
shadow + VideoCore back-end. ST applications that detect TT via
`EgetShift() != 0x51` find a working TT — preferable to the older
ST-only XBIOS path because TT modes match the Pi's native HD
framebuffer better.
