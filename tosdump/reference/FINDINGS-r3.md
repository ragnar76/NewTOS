# Round 3 findings — preludes, and what they exposed

`cargo test -p tos-api --test vdi_reference` → **145 of 158 pass, 13 fail**
(rounds 1–3 together; round 3 contributes 5 of the failures).

Measured on the same Atari TT, same EmuTOS, same 640×480×16 as rounds 1 and 2
— `DEV_TAB` and `SIZ_TAB` in the three logs are identical, so the rounds are
comparable. Case table `cases/vdi-r3.tsv`, CRC `0C06A9C7`, 38 cases and 34
preludes.

## What the preludes bought

`vst_font` (21) now **passes**. Round 2 could not have caught its defect: the
runner resets before every case, so a case could only read a setter's echo,
and an echo proves the echo. Setting a non-default value in a prelude and
reading it back afterwards is what proves the effect.

That is the whole point of the round, and it worked on the first case it was
built for.

## Genuine defects

Only one. Three of the five differences turned out to follow from the colour
profile or, in R3-2's case, to be correct behaviour that the first reading of
this document called a defect.

### R3-1 — `v_get_pixel` (105) takes the palette path in truecolor mode

    case 140   intout[0]   NewTOS = 1   reference = 15
    case 142   intout[0]   NewTOS = 1   reference = 15

`vdi/vdi_fill.c:1104-1113`:

```c
if (TRUECOLOR_MODE) {
    INTOUT[0] = 0;
    INTOUT[1] = pel;
    return;
}
INTOUT[0] = pel;
INTOUT[1] = REV_MAP_COL[pel];
```

`TRUECOLOR_MODE` is `v_planes > 8` (`vdi/vdi_defs.h:22`). NewTOS declares 16
planes, so it is in truecolor mode and `INTOUT[0]` must be **0**. It returns 1.

**FIXED.** `intout[0]` is now 0 and `intout[1]` carries the raw 16-bit screen
word, matching `pixelread`'s truecolor path (`vdi_fill.c:670-690`). It reads
`0x0000` over the black-filled box and `0xFFDF` over the cleared background,
so it really reads the screen rather than returning a constant — and `0xFFDF`
is `(31<<11)|(31<<6)|31`, exactly EmuTOS's pseudo-palette white with the green
LSB clear (`vdi_col.c:601-614`).

**The suite count went DOWN, from 145 passed to 143.** Round-2 cases 140 and
141 (`v_get_pixel` on the cleared background) had been passing **by
coincidence**: NewTOS's palette path answered `(0, 0)` and a palette-mode TT
answers `(0, 0)` too, because `MAP_COL[0]` and `REV_MAP_COL[0]` are both 0.
On the truecolor path `intout[1]` becomes the screen word, so they now differ.
They belong in the same category as the two cases below, and they were
green for the wrong reason.

That is the right outcome, and worth stating plainly: a suite whose number
only ever rises is a suite being optimised for its number.

**Note what this case is and is not.** It can never "pass": the TT has 4
planes and is in palette mode, so its 15 is correct *for a TT* and no NewTOS
answer will equal it. The right NewTOS answer is 0 — neither value in the
comparison. The case is not an oracle here; it is a **detector**, and it
detected a real bug: NewTOS does not follow its own declared profile.

That distinction generalises. Wherever the two systems sit in structurally
different modes, the comparison cannot say what NewTOS should do, but a
difference is still worth reading.

### R3-2 — WITHDRAWN: `DEV_TAB[39] = 0` is correct for this profile

    case 145   intout[39]   NewTOS = 0   reference = 4096

First written as a genuine defect. It is not, and the check that showed it
was made before briefing the fix — a brief on the first reading would have had
an agent "repair" correct code.

`vdi/vdi_control.c:208` sets `DEV_TAB[39] = get_palette()`, and
`bios/screen.c:838-847` says:

```c
/* we return the same values as Atari TOS 4.04 */
if (has_videl) {
    WORD mode = vsetmode(-1);
    if ((mode&VIDEL_COMPAT) || ((mode&VIDEL_BPPMASK) == VIDEL_4BPP))
        return 4096;
    return 0;
}
```

On a Falcon `get_palette()` returns **0** unless the mode is compatibility or
4 bits per plane. The reference's 4096 comes from the separate `sshiftmod`
path for ST and TT colour modes (`:850-852`). NewTOS presents a 16-bpp
truecolor Falcon workstation, so 0 is what EmuTOS itself would report.

This also **closes Mismatch #4** in
`docs/api/vdi/workstation/100-v_opnvwk.md`, which flagged by reasoning that
NewTOS reported the colour count in that slot. It now reports 0, which is
right.

## A decision, not a defect

### R3-3 — `MAX_VERTICES`: 512 against the reference's 1024

    case 117   intout[14]   NewTOS = 512   reference = 1024

`include/config.h:1654` sets 1024 by default; the 512 variants are the small
targets (`:389,446,549,561`). The reference is a `make 512` build and reports
1024.

Not a bug as long as NewTOS genuinely clamps `ptsin` at 512 — reporting a
limit you do not honour would be worse than reporting a lower one. The
question is whether NewTOS should raise its limit to match the declared target
profile.

## Not defects — the colour profile again

`INQ_TAB[4]` (planes, 16 against 4) and `INQ_TAB[5]` (0 against 1) both follow
from the 16-plane Falcon profile. `vdi/vdi_control.c:210-213`:

```c
INQ_TAB[4] = v_planes;
if ((v_planes == 16) || (get_monitor_type() == MON_MONO))
    INQ_TAB[5] = 0;
else INQ_TAB[5] = 1;
```

NewTOS reports 0 because it has 16 planes, which is exactly what EmuTOS would
report on the same hardware. Same family as **R2-8**, and `DEV_TAB[13]` in case
145 likewise.

## Two further defects, found while fixing R3-1 and deliberately left alone

Both are the same family — NewTOS is self-consistent under truecolor but does
not use EmuTOS's scheme — and both are separate opcodes, so they are separate
work.

### R3-4 — `v_clrwk` (3) clears to the wrong word

`vdi/vdi_control.c:608-618`: under `TRUECOLOR_MODE` EmuTOS clears with
`memset(v_bas_ad, 0xff, size)`, i.e. **`0xFFFF`**. NewTOS fills with PEL 0,
which resolves to `0xFFDF`. The difference is exactly `OVERLAY_BIT` (`0x0020`,
`vdi_fill.c:26`), the green LSB.

### R3-5 — `v_contourfill` (103) searches through a PEL round trip

EmuTOS has a separate 16-bit seed fill under truecolor that masks the overlay
bit out of both the search colour and each screen pixel
(`vdi_fill.c:702,709,726,733`). NewTOS searches via `read_pixel`, which is a
lossy mapping back to the nearest CLUT index (`backend.rs:162-165`).

**The three fit together.** EmuTOS clears to `0xFFFF`, its pseudo-palette white
is `0xFFDF`, and the contour fill masks `OVERLAY_BIT` precisely so those two
compare equal. NewTOS clears to `0xFFDF` and searches by PEL — internally
consistent, but a different scheme, and it will diverge wherever a program
mixes the two.

## A case-design mistake of mine

Cases 117 and 119 pair `vs_clip` with `vq_extnd(1)` to read the clip bounds
back. The bounds themselves **match** — `vs_clip` works. But `vq_extnd(1)`
returns 45 device words alongside the four that were the point, and their
profile differences drown the signal.

One case should measure one thing. A read-back has to be chosen so that what
comes back is mostly the thing under test. Round 4 should pair clipping with
something narrower, or accept that this pair can only ever be read by eye.
