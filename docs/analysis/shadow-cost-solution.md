# Making a soft elliptical shadow cheap on a 68030

**Implement fixed-point forward differences plus a small darkening lookup table first.** It preserves the current shadow model, removes floating point from the pixel loop, and works in the shared C source.

There is also a potentially cheaper formulation: because the output is a discrete palette-darkening level, compute the boundaries between levels and draw constant-level spans. Which wins depends on how many levels you use and the cost of generating those boundaries.

The timings below are estimates from the supplied measurements, not new hardware results.

## 1. The practical first implementation

On one row, write your existing affine offsets as

$$
d_a(n)=a+un,\qquad d_b(n)=b+vn,
$$

where $n=0$ is the first pixel you will actually draw. Then

$$
q_0=a^2+b^2,\qquad
\Delta q_0=2(au+bv)+(u^2+v^2),\qquad
\Delta^2q=2(u^2+v^2).
$$

The update is simply:

```c
q  += dq;
dq += ddq;
```

Store those three quantities in signed fixed point. Compute and convert the seeds outside the pixel loop—initially, even floating-point row setup is acceptable.

Use a table mapping squared distance directly to the **final palette increment**:

```c
/* q, dq, ddq: int32_t with F fractional bits.
   Covered span; table maps q to the current frame's darkening. */
for (int x = x0; x < x1; ++x, ++dst) {
    unsigned p = *dst;

    if (is_floor(p)) {
        int32_t qc = q < 0 ? 0 : q;
        unsigned i = (uint32_t)qc >> TABLE_SHIFT;
        if (i >= TABLE_SIZE)
            i = TABLE_SIZE - 1;

        unsigned level = shade[i];
        if (level)
            *dst = (uint8_t)(p + level);
    }

    q += dq;
    dq += ddq;
}
```

This is an implementation outline: retain your existing floor test and palette-range guarantees.

Build `shade[]` using the current coverage function and frame darkness. If coverage depends on radius rather than squared radius, absorb the square root into table construction. **There should be no per-pixel multiply, divide, float conversion, or height adjustment.**

Start with 256 entries; compare against the existing renderer before increasing it. Rebuild only when relevant shading parameters change, and avoid recreating the original expensive computation 256 times each frame: precompute the base falloff or fill the table from your coverage thresholds.

A fixed-point detail matters here: **rounding the second difference accumulates quadratically across a row.** With independently rounded seeds and scale $S=2^F$, a conservative error bound after $n$ steps is

$$
|e_q(n)|\leq
\frac{1+n+n(n-1)/2}{2S}.
$$

Choose precision against your actual maximum span width and threshold spacing. Check intermediate ranges, including the update after the last drawn pixel, to avoid signed overflow. Use wider arithmetic for setup if necessary; keep the pixel loop 32-bit. Reinitializing at each row limits drift.

## 2. Can anything beat that? Yes: darkening-level spans

If darkening decreases monotonically with $q$, each output level occupies a band between nested ellipses. You can compute where the level changes and draw runs with one constant increment.

For a row,

$$
q(n)=An^2+Bn+C.
$$

A coverage threshold $t$ intersects it at

$$
n_\pm=\frac{-B\pm\sqrt{B^2-4A(C-t)}}{2A}.
$$

Those crossings partition the row into runs. The inner loop becomes:

```c
for (; dst < end; ++dst) {
    unsigned p = *dst;
    if (is_floor(p))
        *dst = (uint8_t)(p + level);
}
```

No distance state and no coverage lookup.

This **preserves your existing quantized soft edge**, provided the threshold locations and pixel sampling match the current renderer. It does not turn the shadow into one hard ellipse.

The important qualifications:

- Draw disjoint runs, each pixel once. Repeatedly darkening nested filled ellipses adds unnecessary writes and can change the result.
- Boundary generation must be cheap enough. Several FPU square roots per row per level could consume the savings.
- Cache normalized boundaries, use a square-root table, or incrementally rasterize the threshold ellipses if setup proves expensive.
- Match the existing pixel-center convention and threshold inequalities to avoid one-pixel seams.

I would build this second **if the shadow has only a handful of darkening levels**. Fixed-point stepping is the simpler first change.

## 3. Changing size and darkness does not invalidate precomputation

Separate geometry from shading:

$$
q=\|M(x-c)\|^2,\qquad
\text{level}=L_h(q).
$$

Here $M$ handles size, aspect ratio, and orientation; $L_h$ handles the height-dependent falloff and darkness.

| Precomputed representation | What changes each frame | Main tradeoff |
|---|---|---|
| 1D falloff table indexed by $q$ | Geometry seeds and shading table | Small, works with the existing quadratic |
| Canonical 2D coverage texture | Texture-coordinate increments and darkness mapping | Larger working set; resampling can shimmer |
| Normalized threshold spans | Boundary scale/translation and threshold selection | Very cheap drawing; more setup logic |
| Cached masks for quantized heights | Mask selection and placement | More memory; possible visible stepping |

A canonical 2D mask works because your two normalized coordinates are already affine. Fixed-point texture coordinates advance by additions. However, it adds a texture fetch and introduces sampling choices; it is not automatically faster than a small 1D table.

For fixed orientation and aspect ratio, scaled normalized spans are especially attractive. If perspective also changes aspect ratio or orientation, a general transform still supports a canonical map, but simple uniform scaling is insufficient.

Keep darkness separate from mask coverage so size and opacity do not force a large collection of combined variants. Preserve fractional placement, or test carefully for motion shimmer.

## 4. The structural opportunity is copy-and-shade fusion

The background copy gives you a useful invariant: at the shadow stage, those pixels should still equal the immutable background.

Inside the shadow span, you can therefore do:

```c
unsigned p = *background++;
*dst++ = is_floor(p) ? p + level : p;
```

Arrange the room restoration so it copies the regions outside the current shadow and uses this combined operation inside it. This replaces:

- background read and destination write;
- destination read and optional second write;

with one background read and one destination write.

Retain restoration of the old shadow region and ensure overlapping dirty rectangles do not darken pixels twice. Reflection, ball, and text still follow afterward.

This is useful, but secondary to removing the arithmetic. “Written microseconds earlier” does not imply the pixels remain cached: the 68030 has only 256 bytes of data cache and uses write-through writes. A rectangle-sized pass readily exceeds that capacity. [Motorola MC68030 manual](https://www.nxp.com/docs/en/reference-manual/MC68030UM.pdf)

Also consider precomputed floor spans if the floor geometry is static and they eliminate the palette test without creating excessive fragmentation.

## 5. Expected payoff and implementation order

Your non-shadow frame cost is approximately

$$
174.29-111.76=62.53\text{ ms}.
$$

Holding everything else—including the measured wait—constant:

| New shadow cost | Total frame time | Frame rate |
|---:|---:|---:|
| 10 ms | 72.53 ms | 13.8 fps |
| 5 ms | 67.53 ms | 14.8 fps |
| 2 ms | 64.53 ms | 15.5 fps |
| 0 ms | 62.53 ms | 16.0 fps |

Actual retrace waiting can change with frame timing. These are budgeting scenarios, not predicted benchmark results.

For the October demo, I would implement in this order:

1. **Fixed-point quadratic stepping and a final-level LUT**, initially keeping the bounding box to isolate the arithmetic change.
2. Covered row spans, preserving fractional position and the existing coverage cutoff.
3. Constant-level runs if the palette has few shadow levels and setup can be amortized.
4. Fused background restoration only if further savings are needed.

Measure setup and drawing separately, inspect the compiled loop for FP instructions and helpers, and compare images at minimum/maximum size and darkness plus fractional center positions. The supplied brief is sufficient to choose the algorithm; the actual shadow function and palette rules are needed to make a faithful drop-in patch.
