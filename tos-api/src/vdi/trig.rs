//! Integer sine/cosine for VDI curve flattening (`#![no_std]`, no libm).
//!
//! Data-level port of EmuTOS' quarter-period sine table + `Isin`/`Icos`
//! (`vdi/vdi_gdp.c:48-116`), so VDI arc/circle/ellipse flattening is
//! **pixel-identical** to TOS (AUD-V3-009). The previous Bhaskara-I
//! approximation was accurate to ~0.16 % but not bit-exact against the
//! table, so cardinal-adjacent arc points could differ by 1 px.
//!
//! Angles are in **tenths of a degree**. `isin`/`icos` take a single
//! quadrant (0..900); the quadrant reduction and the on-axis snap live in
//! the GDP `clc_pts` caller (`super::gdp`). Results are Q16 (the table is
//! normalised to 0..65536); scale a Q16 sine by a radius with
//! [`umul_shift`], which divides by 65536.

/// `HALFPI` in tenths of a degree (90.0°) — EmuTOS `vdi_gdp.c:28`.
pub(super) const HALFPI: i16 = 900;

/// The largest angle (tenths) that has a table entry: 89.6°
/// (`MAX_TABLE_ANGLE`, `vdi_gdp.c:36`). By construction the table has no
/// exact-90° entry, which `clc_pts` special-cases via the on-axis snap.
pub(super) const MAX_TABLE_ANGLE: i16 = 896;

/// Sines of 0..89.6° in 0.8° steps, normalised to 0..65536 (Q16). EmuTOS
/// `sin_tbl` (`vdi_gdp.c:57-73`); `SINE_TABLE_SIZE` = MAX_TABLE_ANGLE/8+1
/// = 113 entries. The final entry (89.6°) is < 65536 on purpose so the
/// scale has no exact-1.0 value to return.
static SIN_TBL: [u16; 113] = [
    0, 915, 1830, 2744, 3658, 4572, 5484, 6395, 7305, 8214, 9121, 10026, 10929, 11831, 12729,
    13626, 14519, 15410, 16298, 17183, 18064, 18942, 19816, 20686, 21553, 22415, 23272, 24125,
    24974, 25817, 26656, 27489, 28317, 29140, 29956, 30767, 31572, 32371, 33163, 33949, 34729,
    35501, 36267, 37026, 37777, 38521, 39258, 39986, 40708, 41421, 42126, 42823, 43511, 44191,
    44862, 45525, 46179, 46824, 47459, 48086, 48703, 49310, 49908, 50496, 51075, 51643, 52201,
    52750, 53287, 53815, 54332, 54838, 55334, 55819, 56293, 56756, 57208, 57649, 58078, 58497,
    58903, 59299, 59683, 60055, 60415, 60764, 61101, 61426, 61739, 62040, 62328, 62605, 62870,
    63122, 63362, 63589, 63804, 64007, 64197, 64375, 64540, 64693, 64833, 64960, 65075, 65177,
    65266, 65343, 65407, 65458, 65496, 65522, 65534,
];

/// `Isin` (`vdi_gdp.c:92`): Q16 sine of `angle` (tenths, one quadrant) by
/// table lookup plus linear interpolation across the 0.8° cell. The caller
/// (`clc_pts`) only ever passes angles in `[HALFPI-MAX_TABLE_ANGLE,
/// MAX_TABLE_ANGLE]` = `[4, 896]`, so `index` stays in `0..=112` and the
/// `index + 1` interpolation term is reached only for `index <= 111`
/// (angle 896 has remainder 0 and takes no interpolation).
pub(super) fn isin(angle: i16) -> u16 {
    let index = (angle >> 3) as usize;
    let remainder = (angle & 7) as i32;
    let mut tmpsin = SIN_TBL[index] as i32;
    if remainder != 0 {
        // add interpolation
        tmpsin += ((SIN_TBL[index + 1] as i32 - tmpsin) * remainder) >> 3;
    }
    tmpsin as u16
}

/// `Icos` (`vdi_gdp.c:113`): `Isin(HALFPI - angle)`.
pub(super) fn icos(angle: i16) -> u16 {
    isin(HALFPI - angle)
}

/// `umul_shift` (`intmath.h:73`): `(m1 * m2 + 32768) / 65536` with a 32-bit
/// intermediate — scales a Q16 sine/cosine by a radius, rounding to nearest.
pub(super) fn umul_shift(m1: u16, m2: u16) -> u16 {
    (((m1 as u32) * (m2 as u32) + 32768) / 65536) as u16
}
