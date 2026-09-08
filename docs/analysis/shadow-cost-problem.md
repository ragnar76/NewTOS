# Problem statement: a soft elliptical shadow costs 1,478 cycles per pixel on a 68030

This is a self-contained brief for someone with no prior context. All figures are
measured on real hardware unless explicitly marked as inferred.

## 1. What the program is

A bouncing-ball demo, written in portable C, built from **one source file set for
three targets**:

| Target | CPU | Screen | Frame cost |
|---|---|---|---|
| Raspberry Pi 4, bare metal | AArch64, 1.5 GHz | 1920×1080, 32 bpp | **1.47 ms** |
| Atari ST + PiStorm + emulated ET4000 | emulated 68k | 1920×1072, 32 bpp | 22.9 ms |
| **Atari TT030 + NOVA (ATI Mach64)** | **68030 @ 32 MHz, 68882 FPU** | **640×480, 8 bpp** | **175 ms** |

The problem is the third machine. It manages **5.74 frames per second**.

A ball bounces inside a wireframe room drawn in perspective. It has a textured,
lit, spinning sphere; a soft elliptical shadow on the floor that grows and fades
with height; and a mirrored reflection.

## 2. The Atari TT030 target in detail

- **CPU** Motorola 68030 at 32 MHz. 256-byte instruction cache, 256-byte data
  cache, both enabled; TT-RAM is mapped cacheable.
- **FPU** Motorola 68882 coprocessor, 80-bit internal registers.
- **RAM** 8 MB ST-RAM (16-bit, contended with the video shifter) and 128 MB
  TT-RAM (32-bit, uncontended). **All the demo's buffers are confirmed in
  TT-RAM.**
- **Graphics** NOVA card (ATI Mach64) on the VME bus, driven through fVDI.
  640×480, 256 colours, chunky 8 bits per pixel, one byte per pixel.
- **Vertical retrace** measured at 59.26 Hz, so 16.87 ms per frame.
- **Single-buffered, and a page flip is structurally impossible**: fVDI hooks
  trap 14 and forces `Physbase()` and `Logbase()` to return the same address.
- **Compiler** `m68k-atari-mint-gcc 7.5.0`, `-m68020-60 -O2`, hardware FPU
  (`__M68881__`). Freestanding GEM application, no operating system beyond TOS.

## 3. How a frame is produced

1. Compose the frame into an **8-bit palette-index buffer in TT-RAM** (one byte
   per pixel, 640×480 = 300 KB).
2. Send it to the screen with **`vro_cpyfm`** (VDI raster copy, memory form to
   screen form), one call per dirty rectangle.

Only changed regions are touched. Measured: **2.10 rectangles per frame covering
24,327 pixels, which is 7.9 % of the screen.**

Composition is five passes over a rectangle, in this order:

| Pass | What it does | Pixels/frame |
|---|---|---|
| room | `memcpy` the untouched background back in | 19,469 |
| shadow | darken floor pixels inside an ellipse | 2,420 (bounding box) |
| reflect | mirrored ball below the contact line | ~170 |
| ball | textured lit sphere | 8,171 |
| text | a small readout, three 93×9 rectangles | small |

## 4. The measurement

Per frame, whole-run averages over 296 frames:

```
step (physics)   13.13 ms
compose         140.35 ms
blit             10.84 ms
wait              9.97 ms
                --------
                174.29 ms   ->  5.74 fps
```

Composition split by pass:

```
room      5.00 ms
shadow  111.76 ms      <-- 80 % of composition
reflect   0.88 ms
ball     10.73 ms
text     11.39 ms
```

Per-pixel cost, and this is the crux:

| Pass | µs per pixel | ≈ cycles at 32 MHz | relative |
|---|---|---|---|
| **shadow** | **46.2** | **1,478** | **1×** |
| ball | 1.31 | 42 | 35× cheaper |
| blit (crosses the VME bus!) | 0.446 | 14 | 104× cheaper |

**A shadow pixel costs 35 times a ball pixel, and 104 times what it costs to push
a pixel across the VME bus to the graphics card.**

## 5. Why: static analysis of the shipped m68k object

The ball's inner loop compiles to **pure integer code, zero floating point**:

```c
*dp++ = lut[*shp++ + tp[(unsigned)*op++ + du]];
```

Four loads, two adds, one store. A precomputed sphere map gives each covered
pixel its texel offset and light level; the spin is one added constant.

The shadow's inner loop is **59 instructions per bounding-box pixel, 39 of them
floating point**:

- 9 × `fmulx`
- 4 × `faddx`
- 3 × `fsubx`
- **17 × `fmoves`** — on the 68882 these are *format conversions*, not register moves
- 1 × `jsr` into `__fixxfsi` (float → int helper)

Across the whole object: **1,354 `fmove` against 624 floating-point compute
instructions** — 2.2 moves per operation. Also **123 `fdivx` and not a single
`fsqrt`**, because the square root is a four-step Newton iteration with a divide
in each step.

## 6. What the shadow actually computes

For every pixel in the ellipse's **bounding box**:

1. Two normalised offsets from the ellipse centre, `da` and `db`. Both are
   **affine in x** — they change by a constant per step along a row.
2. `q = da² + db²`, the squared normalised radius.
3. Compare `q` against coverage thresholds to get a darkening level.
4. If the underlying pixel is floor (a palette-range test) and the level is
   non-zero, add the level to the pixel's palette index.

Two structural inefficiencies are visible without measuring:

- The loop runs over the **bounding box**, not the ellipse. Pixels outside the
  ellipse still pay the full distance computation before being rejected. The
  ball, by contrast, has precomputed first and last covered column per row.
- Steps 1 and 2 recompute from scratch at every pixel, although `q` is
  **quadratic in x** along a row.

## 7. What has been ruled out, with evidence

Do not re-propose these.

- **ST-RAM contention.** Every buffer is confirmed in TT-RAM (the program prints
  its allocation classification against `phystop` and `ramtop`).
- **The ball, the texture, the sphere map.** Zero floating point, 7.6 % of the
  cost. Three separate optimisation ideas were aimed here before the split
  measurement existed.
- **The VME bus / graphics bandwidth.** The blit is 10.84 ms of a 174 ms frame.
  It is measured at 393.61 ns/px + 122.56 µs/call.
- **Double buffering / page flip.** Structurally impossible through this driver.
- **Hardware-accelerated blit from main memory.** The Mach64's blitter reads its
  operands from the card's own VRAM; it cannot read TT-RAM. Only screen-to-screen
  is accelerated.
- **Giving more work to the FPU.** Every crossing between integer and FP costs an
  `fmove`, and 44 % of the shadow loop's FP instructions already are conversions.

## 8. Fixes already identified and not yet built

- **Forward differences.** `q` is quadratic in x, so it can be advanced along a
  row with two additions (second difference). Expected to replace roughly ten FP
  compute instructions per pixel and remove the conversions with them.
- **`long double` instead of `float`.** The 68882's registers are 80-bit, so an
  extended value moves without conversion. Measured on the real shadow loop:
  float 59 FP instructions, **`double` 59 (no help)**, **`long double` 42 (−29 %)**.
- **`__builtin_sqrtf` with `-fno-math-errno`** yields one `fsqrts` and no libm
  dependency. This mostly affects start-up, not the frame.
- **Per-row column spans for the shadow**, as the ball already has, so rejected
  pixels are never visited.

## 9. Constraints on any answer

- **The same C source must build for all three targets.** m68k assembly would
  have to live in a separate platform file, not in the shared core.
- **Fixed-point integer is acceptable and probably desirable**; the output is an
  8-bit palette index, so precision requirements are low.
- The shadow must stay **soft-edged** — coverage varies smoothly, and it fades
  and grows with the ball's height. A hard-edged ellipse would be cheaper but is
  a visual regression.
- There is a fixed deadline: a demonstration on 9–10 October 2026.

## 10. The question

**How do you make a soft, distance-shaded elliptical shadow cheap on a 68030 with
a 68882?**

Related, in order of interest:

1. Is there a formulation better than forward differences for the quadratic — for
   example, working entirely in fixed point, or deriving the coverage from a
   precomputed table indexed by something cheap?
2. The ellipse changes size and darkness every frame as the ball rises and falls.
   Does that rule out a precomputed coverage map, or is there a parameterisation
   that survives the change — for instance, one shape scaled by a per-frame
   factor?
3. Anything structural we have missed: the shadow is drawn into the same buffer
   the background was just `memcpy`'d into, so it is a read-modify-write over
   pixels that were written microseconds earlier.

## Appendix: also slow, separate problem

Start-up takes **18.7 seconds** before the first bounce:

- the room's background image: **13.34 s** — about 71 antialiased glow lines,
  computed with a perpendicular distance and a halo per pixel, roughly 386,000
  band pixels at ~35 µs each
- the sphere map: **5.33 s** for 8,184 pixels, each needing several square roots
  and two arcsines

Both are dominated by the same floating-point cost, and both are one-time. The
plan is to compute them once and cache them to disk.
