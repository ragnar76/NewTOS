# TT shifter group — XBIOS 0x50..0x57

> Phase 00 spec, **re-validated** against EmuTOS @ `1a7484ab` —
> `bios/screen.c:180-390` (`esetshift`, `egetshift`, `esetbank`, `esetcolor`,
> `esetpalette`, `egetpalette`, `esetgray`, `esetsmear`), read in full for
> this pass (previously marked "reference-level, not line-verified" — now
> line-verified; two corrections found, D-056/D-057). Cite-never-paste: no
> C bodies; constants/bitmasks as data.
>
> NewTOS profile enables `CONF_WITH_TT_SHIFTER`, so these eight functions
> are present in the dispatch table. The Atari TT shifter hardware does not
> exist on RPi4; semantics are emulated against the framebuffer.
>
> **Note on the build-profile mismatch:** EmuTOS's own `include/config.h:2151
> -2152` requires `CONF_WITH_ATARI_VIDEO` whenever `CONF_WITH_TT_SHIFTER` is
> set — no real EmuTOS build has the TT-shifter opcode table (this file)
> *and* the "no Atari video" runtime fallback that `02-physbase.md`,
> `04-getrez.md`, `05-setscreen.md`, `07-setcolor.md`, `25-vsync.md` extract
> for the NewTOS profile. NewTOS deliberately combines the *table shape* of
> the TT-shifter-enabled branch (this file — real `Eset*`/`Eget*` slots, not
> `xbios_unimpl`) with the *runtime behaviour* of the no-Atari-video branch
> (software shadows, no MMIO) for the other screen calls — a choice, not an
> EmuTOS-attested combination. See `docs/api/_discrepancies.md` D-058.

This group spec covers all eight TT-shifter calls in one file because they
share state, addressing, and the "no hardware present" fallback.

| ID    | Name        | EmuTOS func   | Native ABI (real return / source) | Args |
|------:|-------------|---------------|------------------------------------|------|
| 0x50  | EsetShift   | `esetshift`   | `WORD` (`screen.c:180`) | mode:WORD → WORD |
| 0x51  | EgetShift   | `egetshift`   | `WORD`, `xbios_w_v(81)` (`xbiosbind.h:59`) | — → WORD |
| 0x52  | EsetBank    | `esetbank`    | `WORD` (`screen.c:222`) | bank:WORD → WORD |
| 0x53  | EsetColor   | `esetcolor`   | `WORD`, `xbios_w_ww(83,a,b)` (`xbiosbind.h:60`) | index:WORD, color:WORD → WORD |
| 0x54  | EsetPalette | `esetpalette` | **`WORD`** (`screen.c:265`, not `void`) | index:WORD, count:WORD, rgb:UWORD* → WORD |
| 0x55  | EgetPalette | `egetpalette` | **`WORD`** (`screen.c:290`, not `void`) | index:WORD, count:WORD, rgb:UWORD* → WORD |
| 0x56  | EsetGray    | `esetgray`    | `WORD` (`screen.c:313`) | mode:WORD → WORD |
| 0x57  | EsetSmear   | `esetsmear`   | `WORD` (`screen.c:334`) | mode:WORD → WORD |

**Correction (D-057):** only **`EgetShift` (0x51)** and **`EsetColor` (0x53)**
have an actual binding macro in `include/xbiosbind.h` (`:59-60`); the ABI
shapes for the other six were previously attributed to that header but are
**not defined there** — they are read directly from the C prototypes in
`bios/screen.c`. More importantly, **`EsetPalette`/`EgetPalette` are declared
`WORD`-returning in EmuTOS**, not `void` as the Atari documentation and the
previous ABI table claimed: `bios/screen.c:260-263`'s comment states this
explicitly — *"This function is defined by Atari to return void; however, if
the TT shifter is not present, it should return the function number in a
WORD, which is the de facto TOS standard for unimplemented xbios functions.
Therefore internally we make it return a WORD."* Both return `0` on success
(`:281,306`) or the function number `0x54`/`0x55` when `!has_tt_shifter`
(`:270,295`) — the same D-008 pattern as the other six, just via a
value the m68k caller conventionally ignores (a `void`-declared caller
doesn't read D0, but the value is there and NewTOS's dispatcher must still
produce it, since the *contract* return channel exists even if most callers
don't consume it).

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

**Correction (D-056):** `TT_SHIFTER_BITMASK = 0x970f` (`bios/screen.h:33`),
**not** `0x0700` as previously stated. The mask covers bits 0-3 (colour bank,
read/written separately by `EsetBank` via `TT_SHIFTER+1`), bits 8-10
(resolution — the subset the table below documents), bit 12 and bit 15
(other shifter-state bits, not decoded further here — reference only, no
NewTOS contract depends on them beyond round-tripping through the mask).
`EsetShift`/`EgetShift` read/write the **whole** masked register
(`*resreg & TT_SHIFTER_BITMASK`, `screen.c:194-195,215`); `EsetGray`/
`EsetSmear` address a single byte of it (bit 4 / bit 7 of the low byte,
below). Within that full mask, resolution still sits at bits 8-10:

| `mode`  | Resolution |
|--------:|-----------|
| 0x0000  | TT low (320×480, 256 colours) |
| 0x0100  | TT medium (640×480, 16 colours) |
| 0x0200  | TT high (1280×960, mono) |
| 0x0400  | ST low (320×200, 16 colours) |
| 0x0500  | ST medium (640×200, 4 colours) |
| 0x0600  | ST high (640×400, mono) |

`EsetGray` toggles bit 4 and `EsetSmear` bit 7 of the **low byte** of
`TT_SHIFTER` (`screen.c:315,322-325` / `:336,343-346` — `old | 0x10` /
`old & 0xef` for gray, `old | 0x80` / `old & 0x7f` for smear); the return is
normalised to `0`/`1` (`(old&0x10)?1:0` / `(old&0x80)?1:0`), not the raw byte.

## TT palette (`TT_PALETTE_REGS` = `0xFFFF8400`, 256×UWORD)

`TT_PALETTE_BITMASK = 0x0FFF` — 4 bits/channel, `x RRRR GGGG BBBB`.

## Per-function semantics (line-verified against `bios/screen.c:180-349`)

- **EsetShift(mode)** — `screen.c:180-204`. **Always** writes (no `< 0` query
  guard, unlike the other setters): `vsync()`s first ("to avoid a possible
  resolution change in the middle of a screen display, we wait for a VBL
  (TOS3 does this too)", `:188-192`), then reads `oldmode = *resreg &
  TT_SHIFTER_BITMASK`, writes `*resreg = mode & TT_SHIFTER_BITMASK`, calls
  `vt52_init()` (cell dimensions may have changed), and returns `oldmode`.
  **There is no way to query without changing the register** — every call
  writes.
- **EgetShift()** — `screen.c:210-216`. Read the current shifter register
  (masked); no side effect.
- **EsetBank(bank)** — `screen.c:222-235`. In TT-low 256-colour mode select
  which 16-entry bank is active (byte at `TT_SHIFTER+1`, masked `& 0x0f`);
  `bank < 0` is a query (no write); returns the previous (masked) bank.
- **EsetColor(index, color)** — `screen.c:241-254`. Single palette write;
  `index &= 0xff`; `(WORD)color < 0` is a query (no write); returns the
  previous (masked `0x0FFF`) colour.
- **EsetPalette(index, count, rgb)** / **EgetPalette(...)** — `screen.c:265
  -307`. Bulk palette write/read; `index &= 0xff`; clamp `index+count` to
  ≤ 256 (silent truncation, `count = 256 - index`); **no query form** — always
  copies `count` words; returns `0` (or the function number if no TT
  shifter — D-057).
- **EsetGray(mode)** / **EsetSmear(mode)** — set/clear/query the grey /
  smear bit (`mode > 0` set, `mode == 0` clear, `mode < 0` query — no
  write); return the previous state as `0`/`1`.

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
