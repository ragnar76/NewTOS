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

**Note what this case is and is not.** It can never "pass": the TT has 4
planes and is in palette mode, so its 15 is correct *for a TT* and no NewTOS
answer will equal it. The right NewTOS answer is 0 — neither value in the
comparison. The case is not an oracle here; it is a **detector**, and it
detected a real bug: NewTOS does not follow its own declared profile.

That distinction generalises. Wherever the two systems sit in structurally
different modes, the comparison cannot say what NewTOS should do, but a
difference is still worth reading.

### R3-2 — `DEV_TAB[39]` is 0

    case 145   intout[39]   NewTOS = 0   reference = 4096

`vdi/vdi_control.c:208`: `DEV_TAB[39] = get_palette();` — the RGB-precision
figure, 4096 on a TT. NewTOS reports 0.

This confirms by measurement **Mismatch #4**, already flagged by reasoning in
`docs/api/vdi/workstation/100-v_opnvwk.md`. Worth noting that the flag said
NewTOS reports the colour count there; it reports 0.

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

## A case-design mistake of mine

Cases 117 and 119 pair `vs_clip` with `vq_extnd(1)` to read the clip bounds
back. The bounds themselves **match** — `vs_clip` works. But `vq_extnd(1)`
returns 45 device words alongside the four that were the point, and their
profile differences drown the signal.

One case should measure one thing. A read-back has to be chosen so that what
comes back is mostly the thing under test. Round 4 should pair clipping with
something narrower, or accept that this pair can only ever be read by eye.
