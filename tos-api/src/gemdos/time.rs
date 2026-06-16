//! GEMDOS time / date group (PHASE-07 D1 + D2).
//!
//! Specs: `docs/api/gemdos/time/{2a,2b,2c,2d}-*.md`.
//!
//! NewTOS has no battery RTC; the wall clock is a settable **base
//! epoch** (seconds since the TOS epoch 1980-01-01 00:00:00) plus the
//! monotonic seconds the host reports from the ARM generic timer
//! (`CNTPCT_EL0 / CNTFRQ_EL0`). `Tsetdate` / `Tsettime` recompute the
//! base so the clock reads the requested value; `Tgetdate` /
//! `Tgettime` add the monotonic delta back and pack to the FAT-style
//! WORD encodings.
//!
//! Calendar math uses EmuTOS's simplified leap rule (`bdos/time.c`):
//! every 4th year is a leap year — the 100/400 Gregorian exceptions
//! are dropped, which is exact over the representable 1980..2099 span
//! (2000 is a leap year and no century boundary falls inside).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::host::host;
use crate::trap::TrapArgs;
use tos_types::error::{ERROR, E_OK};
use tos_types::Long;

/// Wall-clock seconds since 1980-01-01 00:00:00 corresponding to
/// monotonic boot time 0. `now = BASE_SECS + monotonic`. Default `0`
/// ⇒ the system boots reading 1980-01-01 (the classic no-RTC default);
/// programs set it via `Tsetdate` / `Tsettime`.
static BASE_SECS: AtomicU64 = AtomicU64::new(0);

const SECS_PER_DAY: u64 = 86_400;

/// Days per month, 1-indexed via `month - 1` (February handled by
/// [`days_in_month`] for leap years).
const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

#[inline]
fn monotonic() -> u64 {
    host().map(|h| h.monotonic_seconds()).unwrap_or(0)
}

#[inline]
fn now_secs() -> u64 {
    BASE_SECS.load(Ordering::Acquire).wrapping_add(monotonic())
}

/// EmuTOS leap rule: every 4th year (valid for 1980..2099).
#[inline]
const fn is_leap(year: u32) -> bool {
    year.is_multiple_of(4)
}

#[inline]
fn days_in_month(year: u32, month: u32) -> u32 {
    if month == 2 && is_leap(year) {
        29
    } else {
        MONTH_DAYS[(month - 1) as usize]
    }
}

/// Broken-down calendar time.
struct Dt {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
}

/// Seconds-since-1980 → broken-down calendar time.
fn decompose(secs: u64) -> Dt {
    let tod = (secs % SECS_PER_DAY) as u32;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;

    let mut days = (secs / SECS_PER_DAY) as u32;
    let mut year = 1980u32;
    loop {
        let yd = if is_leap(year) { 366 } else { 365 };
        if days < yd {
            break;
        }
        days -= yd;
        year += 1;
    }
    let mut month = 1u32;
    loop {
        let md = days_in_month(year, month);
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    Dt { year, month, day: days + 1, hour, min, sec }
}

/// Broken-down calendar time → seconds-since-1980.
fn compose(year: u32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> u64 {
    let mut days: u64 = 0;
    for y in 1980..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    for m in 1..month {
        days += days_in_month(year, m) as u64;
    }
    days += (day - 1) as u64;
    days * SECS_PER_DAY + (hour as u64) * 3600 + (min as u64) * 60 + sec as u64
}

/// Pack to the GEMDOS date WORD: `[15..9]=year-1980, [8..5]=month,
/// [4..0]=day`.
#[inline]
fn pack_date(dt: &Dt) -> u16 {
    (((dt.year - 1980) as u16) << 9) | ((dt.month as u16) << 5) | (dt.day as u16)
}

/// Pack to the GEMDOS time WORD: `[15..11]=hour, [10..5]=minute,
/// [4..0]=second/2`.
#[inline]
fn pack_time(dt: &Dt) -> u16 {
    ((dt.hour as u16) << 11) | ((dt.min as u16) << 5) | ((dt.sec as u16) / 2)
}

/// `Tgetdate` (0x2A) — packed current date (zero-extended LONG).
pub fn tgetdate(_: &TrapArgs) -> Long {
    pack_date(&decompose(now_secs())) as Long
}

/// `Tgettime` (0x2C) — packed current time (zero-extended LONG).
pub fn tgettime(_: &TrapArgs) -> Long {
    pack_time(&decompose(now_secs())) as Long
}

/// `Tsetdate(d)` (0x2B) — set the date part of the wall clock. Returns
/// `E_OK`, or `ERROR` for an out-of-range field (EmuTOS `xsetdate`:
/// year offset > 119, month > 12, day > days-in-month). NewTOS also
/// rejects month/day `< 1` (avoids the EmuTOS day-0 quirk and the
/// underflow it would cause). See `docs/api/gemdos/time/2b-tsetdate.md`.
pub fn tsetdate(args: &TrapArgs) -> Long {
    let d = args.arg_u16(0);
    let year_off = (d >> 9) as u32;
    let month = ((d >> 5) & 0x0F) as u32;
    let day = (d & 0x1F) as u32;

    if year_off > 119 || !(1..=12).contains(&month) {
        return ERROR;
    }
    let year = 1980 + year_off;
    if day < 1 || day > days_in_month(year, month) {
        return ERROR;
    }

    let cur = decompose(now_secs());
    let target = compose(year, month, day, cur.hour, cur.min, cur.sec);
    BASE_SECS.store(target.wrapping_sub(monotonic()), Ordering::Release);
    E_OK
}

/// `Tsettime(t)` (0x2D) — set the time part of the wall clock. Returns
/// `E_OK`, or `ERROR` if the second/2 field ≥ 30 or the minute field
/// ≥ 60 (EmuTOS `xsettime`; the hour field is not validated, matching
/// EmuTOS). See `docs/api/gemdos/time/2d-tsettime.md`.
pub fn tsettime(args: &TrapArgs) -> Long {
    let t = args.arg_u16(0);
    let hour = (t >> 11) as u32; // not validated (EmuTOS parity)
    let min = ((t >> 5) & 0x3F) as u32;
    let dsec = (t & 0x1F) as u32; // second / 2

    if dsec >= 30 || min >= 60 {
        return ERROR;
    }

    let cur = decompose(now_secs());
    let target = compose(cur.year, cur.month, cur.day, hour, min, dsec * 2);
    BASE_SECS.store(target.wrapping_sub(monotonic()), Ordering::Release);
    E_OK
}
