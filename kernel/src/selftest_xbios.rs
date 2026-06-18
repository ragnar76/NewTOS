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
