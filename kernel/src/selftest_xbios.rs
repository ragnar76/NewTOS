//! Phase 10 XBIOS screen-group self-test: exercises Physbase / Logbase /
//! Getrez / Setscreen / Setcolor over the real `svc #14` trap path.

use crate::kprintln;

/// Run the XBIOS screen-group self-test (requires a framebuffer).
pub fn run() {
    if !crate::vdi_backend::available() {
        return;
    }
    let phys0 = physbase();
    let log0 = logbase();
    let rez = getrez();
    kprintln!(
        "[xbios] Physbase=0x{:x} Logbase=0x{:x} Getrez={} (expect base!=0, log==phys, rez=2)",
        phys0, log0, rez,
    );

    // Setscreen: redirect the logical base to a fake buffer, leave phys.
    let fake = 0x4200_0000u64;
    let r = setscreen(fake as i64, -1, -1);
    let log1 = logbase();
    let phys1 = physbase();
    kprintln!(
        "[xbios] Setscreen(log=0x{:x}) ret={} -> Logbase=0x{:x} (==fake) Physbase=0x{:x} (unchanged)",
        fake, r, log1, phys1,
    );
    // Restore the logical base to the hardware framebuffer.
    setscreen(log0, -1, -1);

    // Setscreen rez range check: invalid rez -> -1.
    let bad = setscreen(-1, -1, 9);
    kprintln!("[xbios] Setscreen(rez=9) -> {} (expect -1)", bad);

    // Setcolor: set entry 3, then query it back.
    let old = setcolor(3, 0x0700);
    let got = setcolor(3, -1);
    kprintln!(
        "[xbios] Setcolor(3,0x700) old={} ; query=0x{:x} (expect 0x700)",
        old, got,
    );

    // Phase 11: Random / Vsync / Kbdvbase / Tickcal.
    let r1 = random();
    let r2 = random();
    kprintln!(
        "[xbios] Random -> 0x{:06x}, 0x{:06x} ({}; both 24-bit)",
        r1, r2,
        if r1 != r2 && r1 >> 24 == 0 && r2 >> 24 == 0 { "OK" } else { "FAIL" },
    );
    let kb = kbdvbase();
    kprintln!("[xbios] Kbdvbase -> 0x{:x} ({})", kb, if kb != 0 { "OK" } else { "FAIL" });
    let tc = tickcal();
    kprintln!("[xbios] Tickcal -> {} (expect 20)", tc);
    // Vsync should block ~1 frame each and return (not hang).
    vsync();
    vsync();
    kprintln!("[xbios] Vsync x2 returned (did not hang) OK");

    // Settime/Gettime clock round-trip (shares the GEMDOS wall clock). Set
    // 2021-06-15 14:30:20 as packed date<<16 | time, read it straight back.
    let date = (((2021 - 1980) << 9) | (6 << 5) | 15) as u32; // 0x52CF
    let time = ((14 << 11) | (30 << 5) | (20 / 2)) as u32; // 0x73CA
    let packed = (date << 16) | time;
    settime(packed as i64);
    let got = gettime() as u32;
    kprintln!(
        "[xbios] Settime(2021-06-15 14:30:20) -> Gettime=0x{:08x} (expect 0x{:08x}) {}",
        got, packed,
        if got == packed { "OK" } else { "FAIL" },
    );
}

/// XBIOS `Random` (0x11): no args.
fn random() -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Random ABI.
    unsafe { core::arch::asm!("svc #14", in("x8") 0x11u64, out("x0") r, options(nostack)); }
    r
}

/// XBIOS `Settime` (0x16): x0=packed datetime (date<<16 | time). Void.
fn settime(packed: i64) {
    // SAFETY: svc #14, XBIOS Settime ABI.
    unsafe {
        core::arch::asm!("svc #14", in("x8") 0x16u64, in("x0") packed as u64, lateout("x0") _, options(nostack));
    }
}

/// XBIOS `Gettime` (0x17): no args. Returns the packed datetime.
fn gettime() -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Gettime ABI.
    unsafe { core::arch::asm!("svc #14", in("x8") 0x17u64, out("x0") r, options(nostack)); }
    r
}

/// XBIOS `Kbdvbase` (0x22): no args.
fn kbdvbase() -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Kbdvbase ABI.
    unsafe { core::arch::asm!("svc #14", in("x8") 0x22u64, out("x0") r, options(nostack)); }
    r
}

/// XBIOS `Vsync` (0x25): no args.
fn vsync() {
    // SAFETY: svc #14, XBIOS Vsync ABI.
    unsafe { core::arch::asm!("svc #14", in("x8") 0x25u64, lateout("x0") _, options(nostack)); }
}

/// BIOS `Tickcal` (0x06) via svc #13: no args.
fn tickcal() -> i64 {
    let r: i64;
    // SAFETY: svc #13, BIOS Tickcal ABI.
    unsafe { core::arch::asm!("svc #13", in("x8") 0x06u64, out("x0") r, options(nostack)); }
    r
}

/// XBIOS `Physbase` (0x02): no args.
fn physbase() -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Physbase ABI.
    unsafe { core::arch::asm!("svc #14", in("x8") 0x02u64, out("x0") r, options(nostack)); }
    r
}

/// XBIOS `Logbase` (0x03): no args.
fn logbase() -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Logbase ABI.
    unsafe { core::arch::asm!("svc #14", in("x8") 0x03u64, out("x0") r, options(nostack)); }
    r
}

/// XBIOS `Getrez` (0x04): no args.
fn getrez() -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Getrez ABI.
    unsafe { core::arch::asm!("svc #14", in("x8") 0x04u64, out("x0") r, options(nostack)); }
    r
}

/// XBIOS `Setscreen` (0x05): x0=logLoc, x1=physLoc, x2=rez, x3=videl.
fn setscreen(log: i64, phys: i64, rez: i16) -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Setscreen ABI.
    unsafe {
        core::arch::asm!("svc #14", in("x8") 0x05u64, inout("x0") log => r, in("x1") phys, in("x2") rez as u64, in("x3") 0u64, options(nostack));
    }
    r
}

/// XBIOS `Setcolor` (0x07): x0=index, x1=value (-1 to query). Old value.
fn setcolor(index: u16, value: i16) -> i64 {
    let r: i64;
    // SAFETY: svc #14, XBIOS Setcolor ABI.
    unsafe {
        core::arch::asm!("svc #14", in("x8") 0x07u64, inout("x0") index as u64 => r, in("x1") value as u64, options(nostack));
    }
    r
}
