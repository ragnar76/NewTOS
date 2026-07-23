//! Phase 07 time/date + version self-test (D1, D2, D5).
//!
//! Drives `Tsetdate`/`Tsettime`/`Tgetdate`/`Tgettime`/`Sversion`
//! through the GEMDOS dispatcher (svc #1) and checks the clock source
//! (`CNTPCT_EL0`) actually ticks.

use crate::kprintln;

#[inline(always)]
fn svc0(fnid: u64) -> i64 {
    let r: i64;
    // SAFETY: svc #1, constant fn id, no argument.
    unsafe {
        core::arch::asm!("svc #1", in("x8") fnid, out("x0") r, options(nostack));
    }
    r
}

#[inline(always)]
fn svc1(fnid: u64, a0: u64) -> i64 {
    let r: i64;
    // SAFETY: svc #1, constant fn id + one argument.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") fnid,
            inout("x0") a0 => r,
            options(nostack),
        );
    }
    r
}

#[inline(always)]
fn cntpct() -> u64 {
    let c: u64;
    // SAFETY: read-only system-register read.
    unsafe {
        core::arch::asm!("mrs {}, cntpct_el0", out(reg) c, options(nomem, nostack, preserves_flags));
    }
    c
}

fn sversion() -> i64 {
    svc0(0x30)
}
fn tgetdate() -> u16 {
    svc0(0x2A) as u16
}
fn tgettime() -> u16 {
    svc0(0x2C) as u16
}
fn tsetdate(d: u16) -> i64 {
    svc1(0x2B, d as u64)
}
fn tsettime(t: u16) -> i64 {
    svc1(0x2D, t as u64)
}

/// Run the Phase 07 time/date/version self-test.
pub fn run_phase07_time_test() {
    kprintln!("");
    kprintln!("[time] === Phase 07 time/date + Sversion test ===");
    let mut ok = true;

    // D5 — Sversion.
    let v = sversion();
    let sv_ok = v == 0x3000;
    ok &= sv_ok;
    kprintln!(
        "[time]   Sversion = 0x{:04x}  {}",
        v as u16,
        if sv_ok { "OK" } else { "FAIL" },
    );

    // D2 — set then get round-trip. Target: 2026-06-16 12:34:56.
    let date = ((2026u16 - 1980) << 9) | (6 << 5) | 16; // 0x5CD0
    let time = (12u16 << 11) | (34 << 5) | (56 / 2); //    0x645C
    let sd = tsetdate(date);
    let st = tsettime(time);
    let gd = tgetdate();
    let gt = tgettime();

    let date_ok = sd == 0 && gd == date;
    let h = (gt >> 11) & 0x1F;
    let m = (gt >> 5) & 0x3F;
    let s2 = gt & 0x1F;
    // Date is exact; the seconds field may tick by 1 between set+get.
    let time_ok = st == 0 && h == 12 && m == 34 && (s2 == 28 || s2 == 29);
    ok &= date_ok && time_ok;

    let y = (gd >> 9) + 1980;
    let mo = (gd >> 5) & 0x0F;
    let dy = gd & 0x1F;
    kprintln!(
        "[time]   now: {}-{:02}-{:02} {:02}:{:02}:{:02}  (date {} time {})",
        y, mo, dy, h, m, s2 * 2,
        if date_ok { "OK" } else { "FAIL" },
        if time_ok { "OK" } else { "FAIL" },
    );

    // Out-of-range Tsetdate (month 13) must be rejected with ERROR(-1).
    let bad = tsetdate((46 << 9) | (13 << 5) | 1);
    let bad_ok = bad == -1;
    ok &= bad_ok;
    kprintln!(
        "[time]   Tsetdate(month=13) → {}  {}",
        bad,
        if bad_ok { "OK" } else { "FAIL" },
    );

    // D1/EC1 — the clock source (CNTPCT_EL0) advances.
    let c1 = cntpct();
    for _ in 0..2_000_000u32 {
        core::hint::spin_loop();
    }
    let c2 = cntpct();
    let tick_ok = c2 > c1;
    ok &= tick_ok;
    kprintln!(
        "[time]   CNTPCT advanced (+{})  {}",
        c2.wrapping_sub(c1),
        if tick_ok { "OK" } else { "FAIL" },
    );

    if ok {
        kprintln!("[time] === Phase 07 time PASS ===");
    } else {
        kprintln!("[time] === Phase 07 time FAIL ===");
    }
}
