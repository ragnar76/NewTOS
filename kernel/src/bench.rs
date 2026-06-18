//! Memory performance benchmark (`bench` shell command).
//!
//! Measures sequential memory **read**, **write** and **copy** bandwidth
//! over a multi-megabyte buffer, timed with the ARM generic timer
//! (`CNTPCT_EL0` / `CNTFRQ_EL0`), and prints MB/s via GEMDOS `Cconout`.
//!
//! Note: in QEMU this reflects the TCG emulation speed, not hardware
//! bandwidth; on a real Pi 4 it reports the actual figures. The buffer is
//! sized to exceed the Cortex-A72's 1 MiB L2 cache so the numbers reflect
//! main memory rather than cache.

use core::cell::UnsafeCell;
use core::hint::black_box;

/// Per-buffer size (2 MiB > A72 L2). Two buffers → 4 MiB of BSS.
const SIZE: usize = 2 * 1024 * 1024;
const WORDS: usize = SIZE / 8;
/// Passes per test, so the total transfer is large enough to time well.
const ITERS: u64 = 64;

// 8-byte-aligned word buffers (a `[u8; SIZE]` cast to `*const u64` would
// fault the alignment check).
struct Buf(UnsafeCell<[u64; WORDS]>);
// SAFETY: accessed only from the single-CPU shell command path.
unsafe impl Sync for Buf {}
static SRC: Buf = Buf(UnsafeCell::new([0u64; WORDS]));
static DST: Buf = Buf(UnsafeCell::new([0u64; WORDS]));

/// Run all three benchmarks and print the results.
pub fn run() {
    puts(b"Memory benchmark (2 MiB buffer, 64 passes = 128 MiB each)\r\n");
    puts(b"  (QEMU shows emulation speed; real HW shows true bandwidth)\r\n\r\n");

    // SAFETY: single-threaded shell context; the two buffers are distinct.
    let src_w = unsafe { &*SRC.0.get() };
    let dst_w = unsafe { &mut *DST.0.get() };

    // Warm the buffers (fault in / prime caches once) before timing.
    for w in dst_w.iter_mut() {
        *w = 0;
    }
    black_box(src_w[0]);

    let total = SIZE as u64 * ITERS;

    // ── read ──
    let t0 = ticks();
    let mut acc = 0u64;
    for _ in 0..ITERS {
        for &w in src_w.iter() {
            acc = acc.wrapping_add(w);
        }
    }
    black_box(acc);
    report(b"Read ", total, ticks() - t0);

    // ── write ──
    let t0 = ticks();
    for i in 0..ITERS {
        let v = 0xA5A5_A5A5_A5A5_A5A5u64 ^ i;
        for w in dst_w.iter_mut() {
            *w = v;
        }
    }
    black_box(dst_w[0]);
    report(b"Write", total, ticks() - t0);

    // ── copy ──
    let t0 = ticks();
    for _ in 0..ITERS {
        dst_w.copy_from_slice(src_w);
    }
    black_box(dst_w.as_ptr());
    report(b"Copy ", total, ticks() - t0);
}

/// Print one result line: `<label>: <MB/s> MB/s`.
fn report(label: &[u8], bytes: u64, elapsed: u64) {
    let hz = freq();
    // MB/s = bytes * hz / elapsed / 1e6. bytes*hz fits u64 here.
    let mbps = bytes
        .saturating_mul(hz)
        .checked_div(elapsed)
        .unwrap_or(0)
        / 1_000_000;
    puts(b"  ");
    puts(label);
    puts(b": ");
    put_num(mbps);
    puts(b" MB/s\r\n");
}

// ── timing ────────────────────────────────────────────────────────────

fn ticks() -> u64 {
    let t: u64;
    // SAFETY: CNTPCT_EL0 is an EL1-readable monotonic counter.
    unsafe { core::arch::asm!("mrs {t}, cntpct_el0", t = out(reg) t, options(nomem, nostack)) };
    t
}
fn freq() -> u64 {
    let f: u64;
    // SAFETY: CNTFRQ_EL0 is EL1-readable.
    unsafe { core::arch::asm!("mrs {f}, cntfrq_el0", f = out(reg) f, options(nomem, nostack)) };
    f
}

// ── output (value-based GEMDOS Cconout) ─────────────────────────────────

fn puts(s: &[u8]) {
    for &b in s {
        cconout(b);
    }
}
fn put_num(mut v: u64) {
    let mut tmp = [0u8; 20];
    let mut i = tmp.len();
    loop {
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    puts(&tmp[i..]);
}
fn cconout(c: u8) {
    // SAFETY: svc #1, GEMDOS Cconout (0x02); char in x0.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x02u64, inout("x0") c as u64 => _, options(nostack));
    }
}
