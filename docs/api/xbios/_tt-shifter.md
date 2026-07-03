# TT shifter group — XBIOS 0x50..0x57

> Phase 00 spec (reference-level — these eight are outside the 24-function
> "own-spec" set but present in the NewTOS TT-shifter profile). Source:
> EmuTOS @ `1a7484ab` — `bios/screen.c` (`esetshift`, `egetshift`,
> `esetbank`, `esetcolor`, `esetpalette`, `egetpalette`, `esetgray`,
> `esetsmear`). Cite-never-paste: no C bodies; constants/bitmasks as data.
>
> NewTOS profile enables `CONF_WITH_TT_SHIFTER`, so these eight functions
> are present in the dispatch table. The Atari TT shifter hardware does not
> exist on RPi4; semantics are emulated against the framebuffer.

This group spec covers all eight TT-shifter calls in one file because they
share state, addressing, and the "no hardware present" fallback.

| ID    | Name        | EmuTOS func   | Native ABI (`include/xbiosbind.h`) | Args |
|------:|-------------|---------------|------------------------------------|------|
| 0x50  | EsetShift   | `esetshift`   | `xbios_w_w`   | mode:WORD → WORD |
| 0x51  | EgetShift   | `egetshift`   | `xbios_w_v`   | — → WORD |
| 0x52  | EsetBank    | `esetbank`    | `xbios_w_w`   | bank:WORD → WORD |
| 0x53  | EsetColor   | `esetcolor`   | `xbios_w_ww`  | index:WORD, color:WORD → WORD |
| 0x54  | EsetPalette | `esetpalette` | `xbios_v_wwl` | index:WORD, count:WORD, rgb:UWORD* |
| 0x55  | EgetPalette | `egetpalette` | `xbios_v_wwl` | index:WORD, count:WORD, rgb:UWORD* |
| 0x56  | EsetGray    | `esetgray`    | `xbios_w_w`   | mode:WORD → WORD |
| 0x57  | EsetSmear   | `esetsmear`   | `xbios_w_w`   | mode:WORD → WORD |

Vector indices `0x50..0x57` = `xbios_vecs[80..87]`
(`bios/xbios.c:1391-1399`, gated by `CONF_WITH_TT_SHIFTER`).

## No-hardware fallback

Each function begins with a `has_tt_shifter` guard: when the TT shifter is
not present it returns early. This "returns the function number" behaviour
for the no-hardware case is the same D-008 pattern used elsewhere (an
early return that leaves the opcode-shaped value in D0). On the NewTOS
profile `has_tt_shifter` is reported **true** so the emulated path runs and
the D-008 fallback is not taken.

> **Correction (D-039):** an earlier draft's "Falcon overlap" note claimed a
> Falcon-detecting program calling `Vsetmode` (0x58+) "sees `EINVFN`". It
> does **not** — 0x58 is beyond `LAST_ENTRY (0x57)`, so the dispatcher
> returns the **function number** `0x58`, not `EINVFN`. See `_dispatcher.md
> §3`.

## TT shifter mode register (`TT_SHIFTER` = `0xFFFF8262`)

`TT_SHIFTER_BITMASK = 0x0700` (only bits 8..10 encode the resolution):

| `mode`  | Resolution |
|--------:|-----------|
| 0x0000  | TT low (320×480, 256 colours) |
| 0x0100  | TT medium (640×480, 16 colours) |
| 0x0200  | TT high (1280×960, mono) |
| 0x0400  | ST low (320×200, 16 colours) |
| 0x0500  | ST medium (640×200, 4 colours) |
| 0x0600  | ST high (640×400, mono) |

`EsetGray` toggles the grey-mode bit and `EsetSmear` the smear bit of the
same register (bit positions per `bios/screen.c`; treat as reference).

## TT palette (`TT_PALETTE_REGS` = `0xFFFF8400`, 256×UWORD)

`TT_PALETTE_BITMASK = 0x0FFF` — 4 bits/channel, `x RRRR GGGG BBBB`.

## Per-function semantics (reference)

- **EsetShift(mode)** — set TT resolution; returns the previous mode
  (masked). Re-initialises the VT52 because cell dimensions change.
- **EgetShift()** — read the current shifter register (masked); no side
  effect.
- **EsetBank(bank)** — in TT-low 256-colour mode select which 16-entry bank
  is active; `bank < 0` is a query; returns the previous bank.
- **EsetColor(index, color)** — single palette write; `index &= 0xff`;
  `color < 0` is a query; returns the previous (masked) colour.
- **EsetPalette(index, count, rgb)** / **EgetPalette(...)** — bulk
  palette write/read; clamp `index+count` to ≤ 256 (silent truncation).
- **EsetGray(mode)** / **EsetSmear(mode)** — set/clear/query the grey /
  smear bit; return the previous state.

> Behaviour details above (the exact `vsync`/`vt52_init` calls, the precise
> query-guard conditions, and the return of `0` vs. previous value for the
> bulk calls) are taken from the function comments and are **reference**;
> they were not line-verified in this pass because these opcodes are outside
> the 24-function contract set. Verify against `bios/screen.c` before
> implementing.

## Edge cases (reference)

1. **`index & 0xff` wraparound** — `index = 256` becomes 0; out-of-range
   indices silently overwrite earlier entries.
2. **`count` truncation** — `index+count > 256` is clamped, not an error.
3. **Masked read-modify-write** — the previous value is read masked and the
   new value written masked; high bits stay zero. NewTOS emulation must
   preserve this.

## NewTOS emulation strategy

Preserve the family as **ABI surface only** — the display is the BCM2711
framebuffer. NewTOS: (1) keep a software shadow of `TT_SHIFTER` +
`TT_PALETTE_REGS[256]`; (2) translate each change to a VideoCore mailbox
`set_palette` / framebuffer reconfigure; (3) report `has_tt_shifter = 1`
so programs get a working TT rather than the D-008 fallback.

## Dependencies

- `has_tt_shifter` (boot detection).
- `TT_SHIFTER`, `TT_PALETTE_REGS` addresses (reference).
- `vsync()`, `vt52_init()`.

Used by: TT-aware applications, TT-high-res scientific software, palette
demos.

## Cross-references

- **Compendium / Profibuch:** not present in `references/`; cross-check
  deferred.
- `docs/api/xbios/05-setscreen.md` — overlapping concerns at TT
  resolutions.
- `docs/api/xbios/06-setpalette.md`, `07-setcolor.md` — ST-era counterparts.
- `docs/api/xbios/_dispatcher.md §3` — the func-number (not `EINVFN`)
  return.

## Configuration

- `CONF_WITH_TT_SHIFTER` — enabled in the NewTOS profile.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: NewTOS exposes the TT shifter as ABI-only with a software shadow +
VideoCore back-end. `EgetShift() != <func-number>` lets ST applications find
a working TT, which maps better onto the Pi's native HD framebuffer than the
ST-only path.
