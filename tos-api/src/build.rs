//! The build stamp — NewTOS's counterpart of EmuTOS's generated `obj/header.h`.
//!
//! EmuTOS runs `tools/mkheader.awk` at build time and gets `OS_DATE` (BCD
//! `MMDDYYYY`), `OS_DOSDATE` (GEMDOS-packed) and `COPYRIGHT_YEAR`
//! (`references/emutos/obj/header.h`). Those are not decoration: `os_date` and
//! `os_dosdate` sit in the OSHEADER that programs read, so the value has to
//! come from the build rather than from a literal.
//!
//! [`build.rs`](../../build.rs) emits the date and the short revision; this
//! module turns the date into the two packed forms at compile time and hands
//! all of it to its readers — the OSHEADER writer in
//! [`crate::bdos::process::loader`] and, through [`BUILD_DATE`] /
//! [`BUILD_REV`], the kernel's boot screen.
//!
//! `SOURCE_DATE_EPOCH` pins the date when it is set, so a reproducible build
//! stays reproducible.

/// The build date as ISO `YYYY-MM-DD`.
pub const BUILD_DATE: &str = env!("NEWTOS_BUILD_DATE");

/// The short commit the build came from, or `"unknown"` outside a work tree.
pub const BUILD_REV: &str = env!("NEWTOS_BUILD_REV");

/// `BUILD_DATE` and `BUILD_REV` as one line, `YYYY-MM-DD (rev)`.
pub const BUILD_STAMP: &str = concat!(env!("NEWTOS_BUILD_DATE"), " (", env!("NEWTOS_BUILD_REV"), ")");

/// The build year, as the four digits of [`BUILD_DATE`].
pub const BUILD_YEAR: u16 = parse4(0);

/// The build month, 1-12.
pub const BUILD_MONTH: u8 = parse2(5) as u8;

/// The build day, 1-31.
pub const BUILD_DAY: u8 = parse2(8) as u8;

/// OSHEADER `os_date` (+0x18): the build date in BCD `MMDDYYYY`, the shape
/// `mkheader.awk` writes (`obj/header.h`: `#define OS_DATE 0x08292026` for
/// 2026-08-29).
pub const OS_DATE: u32 = (bcd2(BUILD_MONTH) as u32) << 24
    | (bcd2(BUILD_DAY) as u32) << 16
    | (bcd2((BUILD_YEAR / 100) as u8) as u32) << 8
    | bcd2((BUILD_YEAR % 100) as u8) as u32;

/// OSHEADER `os_dosdate` (+0x1E): the same date GEMDOS-packed,
/// `(year - 1980) << 9 | month << 5 | day` (`obj/header.h`: `23837` for
/// 2026-08-29).
pub const OS_DOSDATE: u16 =
    ((BUILD_YEAR - 1980) << 9) | ((BUILD_MONTH as u16) << 5) | BUILD_DAY as u16;

/// Two decimal digits of [`BUILD_DATE`] starting at `at`.
const fn parse2(at: usize) -> u16 {
    let b = BUILD_DATE.as_bytes();
    (b[at] - b'0') as u16 * 10 + (b[at + 1] - b'0') as u16
}

/// Four decimal digits of [`BUILD_DATE`] starting at `at`.
const fn parse4(at: usize) -> u16 {
    parse2(at) * 100 + parse2(at + 2)
}

/// One byte as two BCD digits (`29` → `0x29`). Values above 99 cannot occur:
/// every caller passes a month, a day or half a year.
const fn bcd2(v: u8) -> u8 {
    ((v / 10) << 4) | (v % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The packing is EmuTOS's, checked against the one generated header we
    /// have: `obj/header.h` states `OS_DATE 0x08292026` and `OS_DOSDATE 23837`
    /// for a build made on 2026-08-29.
    #[test]
    fn packs_like_mkheader() {
        assert_eq!(bcd2(8), 0x08);
        assert_eq!(bcd2(29), 0x29);
        assert_eq!(bcd2(20), 0x20);
        assert_eq!(bcd2(26), 0x26);
        let os_date: u32 = (bcd2(8) as u32) << 24
            | (bcd2(29) as u32) << 16
            | (bcd2(20) as u32) << 8
            | bcd2(26) as u32;
        assert_eq!(os_date, 0x0829_2026);
        let dosdate: u16 = ((2026u16 - 1980) << 9) | (8u16 << 5) | 29u16;
        assert_eq!(dosdate, 23_837);
    }

    /// The stamp this build actually carries has to be a usable date, not a
    /// placeholder: a real month, a real day, and a year GEMDOS can pack (its
    /// epoch is 1980). No upper or lower bound beyond that — `SOURCE_DATE_EPOCH`
    /// may legitimately pin an older date for a reproducible build.
    #[test]
    fn this_build_is_stamped() {
        assert_eq!(BUILD_DATE.len(), 10);
        assert!(BUILD_YEAR >= 1980, "build year {BUILD_YEAR} predates the GEMDOS epoch");
        assert!((1..=12).contains(&BUILD_MONTH), "month {BUILD_MONTH}");
        assert!((1..=31).contains(&BUILD_DAY), "day {BUILD_DAY}");
        // The GEMDOS packing must round-trip to the same three fields.
        assert_eq!(OS_DOSDATE >> 9, BUILD_YEAR - 1980);
        assert_eq!((OS_DOSDATE >> 5) & 0x0f, BUILD_MONTH as u16);
        assert_eq!(OS_DOSDATE & 0x1f, BUILD_DAY as u16);
    }
}
