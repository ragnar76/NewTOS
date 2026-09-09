# An ordered dither on a constant value is a halftone screen

A design decision, not a bug hunt: the cause is found and measured, two fixes are
built and measured, and the question is which to take. Self-contained for a
reader with no prior context. Everything below is measured unless marked.

Companion image: `cmp32.png` — the ball's shaded underside at 4×, three strips:
as shipped, variant A, variant B.

## 1. What draws the ball

A bouncing-ball demo in portable C, one source built for a Raspberry Pi 4 (bare
metal, 1920×1080), an Atari TT030 with a NOVA card (640×480, 256 colours) and an
Atari ST with a PiStorm (1920×1072, truecolour). The defect appears on **all
three**, identically.

The ball is a textured, lit sphere. It is not shaded per frame. At start-up a
**sphere map** is built: for every covered screen pixel it stores the texel it
samples and a **light level**. Per frame the inner loop is then

```c
*dp++ = lut[*shp++ + tp[(unsigned)*op++ + du]];
```

four loads, two adds, one store, no arithmetic. The spin is one added constant.

There are `BALL_LEVELS = 48` light levels. Because 48 levels have to represent a
continuous gradient, the level is **ordered-dithered** with an 8×8 Bayer matrix,
normalised to [0,1):

```c
dith8[y][x] = ((float)m8[y][x] + 0.5f) / 64.0f;
```

and applied when the sphere map is built:

```c
sh = (int)(lit * (float)(BALL_LEVELS - 1) + dith8[sy & 7][sx & 7]);
```

## 2. The lighting, verbatim

```c
dif = fx*lx + fy*ly + nz*lz;      /* N·L, diffuse            */
if (dif < 0.0f) dif = 0.0f;       /* <-- the clamp that matters */

sp = fx*hx + fy*hy + nz*hz;       /* N·H, specular           */
if (sp < 0.0f) sp = 0.0f;
sp *= sp; sp *= sp; sp *= sp;
sp *= sp; sp *= sp; sp *= sp;     /* ^64, a tight highlight  */

lit = LIT_TOP * (LIT_AMB + (1.0f - LIT_AMB) * dif) + LIT_SPEC * sp;
if (lit > 1.0f) lit = 1.0f;
```

with `LIT_AMB = 0.12f` (how lit the terminator still is), `LIT_TOP = 0.85f` (the
share of the ramp the diffuse term owns) and `LIT_SPEC = 0.15f`.

## 3. The defect, to the decimal

Behind the terminator `dif` clamps to zero and `sp` is zero with it. So `lit` is
**exactly constant over the whole unlit underside**:

```
lit = LIT_TOP · LIT_AMB = 0.85 · 0.12 = 0.102
0.102 · 47 = 4.794          integer part 4, fraction 0.794
```

The Bayer dither raises exactly the cells whose threshold is ≥ 0.206 — 51 of 64,
**79.7 %** — and leaves 13 of 64, **20.3 %**, one level lower. Measured on the
real sphere map for a 102-pixel ball, 8184 covered pixels:

```
dif clamped to 0                        1391 px = 17.0 % of the ball
dif AND sp both 0  ->  lit is CONSTANT   889 px = 10.9 %
   of those, level 4:  181 px (20.4 %)
   of those, level 5:  708 px (79.6 %)   ... and nothing else
```

Predicted 79.7/20.3, measured 79.6/20.4.

**An ordered dither is correct on average and conspicuous in structure when the
value it dithers does not vary.** Everywhere else on the ball `lit` changes from
pixel to pixel, the local density of raised cells changes with it, and the same
dither reads as a smooth ramp. On the cap it has nothing to ride on, so it prints
a fixed 8×8 lattice.

The user's own description, seeing it on the Pi: *"looks like missing
antialiasing."*

Measured on the rendered image, lower hemisphere, fraction of pixels exactly
8×8-periodic:

| | palette, 640×480 | truecolour, 1920×1072 |
|---|---|---|
| as shipped | 14.28 % | **25.30 %** |

Truecolour is worse because every level gets its own RGB, so every dither step is
visible; a 256-colour palette collapses adjacent levels in short colour ramps and
hides part of it.

## 4. Two fixes, both built and measured

**A — suppress the dither where `lit` is exactly constant.** Round instead:
`sh = (int)(lit * 47 + 0.5f)` on those pixels.

**B — let the ambient fall off behind the terminator instead of clamping it.** A
two-piece linear map of `N·L ∈ [-1,1]` onto `[0,1]`, with the knee at
`(0, LIT_AMB)`; the ambient falls linearly from the terminator to zero at the
anti-pole. **No new constant:**

```c
lit = LIT_TOP * (dif >= 0.0f ? LIT_AMB + (1.0f - LIT_AMB) * dif
                             : LIT_AMB * (1.0f + dif))
      + LIT_SPEC * sp;
```

(with the `dif` clamp removed so it can go negative; written as a ternary
*inside* the `LIT_TOP` multiplication rather than via a temporary, so the
expression stays bit-identical on the 68882, where an intermediate `float`
would introduce a rounding that does not exist today)

Results, same metric:

| | palette, 640×480 | truecolour, 1920×1072 |
|---|---|---|
| as shipped | 14.28 % | 25.30 % |
| **A** | 12.13 % | 24.30 % |
| **B** | **0.38 %** | **13.85 %** |

**A barely helps, and the reason is measurable:** it can only reach the 10.9 %
where `lit` is *exactly* constant, while the visible lattice extends further into
the band where `lit` varies so slowly that consecutive pixels stay on the same
side of the same threshold.

In the image, A also replaces the screen with a **flat plateau** — the cap looks
like a dark patch painted on, and the ball stops reading as round. B produces a
genuine gradient: the cap darkens toward the limb and the stipple varies with it.

**Neither touches the lit side.** Verified by writing out the sphere map from
both builds together with a mask of the sign of `dif`:

```
sphere-map cells with dif > 0 (lit side): 6793, levels changed: 0
```

B changes 9199 pixels on the whole 1920×1072 image, all inside the ball, maximum
channel deviation 24/255. A changes 1615 pixels, maximum 6/255.

Under B the cap spans levels 1–5 (1.6 / 16.2 / 43.1 / 33.9 / 5.3 %) instead of
two levels in a 20/80 split. **No pixel reaches level 0**, so the texture stays
readable everywhere.

## 5. What is open, stated rather than glossed

The 13.85 % residual periodicity under B at truecolour is at the limit of this
metric: "equal to the pixel 8 away" is also true of a *healthy* dither on a very
gentle ramp. That the palette machine simultaneously falls to 0.38 % argues the
remainder is no longer a lattice, but the metric cannot prove it; the image is
the better argument there.

## 6. The same mechanism, second site, unmeasured

The floor shadow's coverage is `1 − q²`, which is flat in the middle, so its
value is nearly constant in the shadow's core and its 4×4 dither is the same
halftone screen. This has **not** been measured. It is raised here because it
bears on whether this is a one-off or a pattern in how the demo dithers.

## 7. Constraints on any answer

- One C source builds for all three machines; a fix must work in all of them.
- The sphere map is built once at start-up, so cost there is nearly free —
  5.3 s on the TT already, 20 ms on the Pi. Per-frame cost must not rise: the
  inner loop is four loads, two adds, one store and no arithmetic, and that is
  the property that makes the demo viable on a 32 MHz 68030.
- 48 light levels, 8×8 Bayer, a 256-entry palette on two machines and truecolour
  on the third.
- The ball must stay readable: its texture carries lettering.

## 8. The questions

1. **A or B — or something neither of us thought of?** B is the recommendation:
   it removes the cause rather than the symptom, leaves the lit side provably
   untouched, and makes the ball rounder rather than merely less wrong. Its price
   is that the dark side gets visibly darker, up to 24/255, on all three
   machines.
2. Is a two-piece linear ambient falloff the right model, or is there a standard
   formulation that behaves better at the terminator?
3. Is there a dithering choice that is robust against a constant input in
   general — so that the shadow's flat core, and any future flat region, cannot
   reproduce this?
4. B leaves 13.85 % residual periodicity at truecolour. Is that inherent to an
   8×8 Bayer on a gentle gradient, or is there something better at this level
   count?
