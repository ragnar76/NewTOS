//! Integer sine/cosine for VDI curve flattening (`#![no_std]`, no libm).
//!
//! Angles are in **tenths of a degree** (0..3600, CCW), matching the VDI
//! arc/rotation convention. Results are Q15 fixed point in
//! `[-32767, 32767]` (i.e. value × 32767). Uses Bhaskara I's sine
//! approximation, whose worst-case error (~0.16 %) is far below one pixel
//! for screen-sized radii — and it is exact at 0/30/90/150/180°.

const Q15: i64 = 32767;

/// `sin(angle)` in Q15, `angle` in tenths of a degree.
pub fn sin(angle: i32) -> i32 {
    let t = angle.rem_euclid(3600);
    // Reduce to [0,1800] (the positive half); the upper half is negated.
    let (t, neg) = if t >= 1800 { (t - 1800, true) } else { (t, false) };
    // Bhaskara I: sin(d°) ≈ 4·d(180−d) / (40500 − d(180−d)), here scaled
    // for tenths via P = t(1800−t) so d(180−d) = P/100.
    let p = (t as i64) * (1800 - t as i64);
    let val = (4 * p * Q15) / (4_050_000 - p);
    if neg {
        -(val as i32)
    } else {
        val as i32
    }
}

/// `cos(angle)` in Q15 (`cos x = sin(x + 90°)`), `angle` in tenths.
pub fn cos(angle: i32) -> i32 {
    sin(angle + 900)
}

/// `(sin, cos)` in Q15.
pub fn sincos(angle: i32) -> (i32, i32) {
    (sin(angle), cos(angle))
}
