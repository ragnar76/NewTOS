//! Memory performance benchmark (`bench` shell command).
//!
//! Measures sequential memory **read**, **write** and **copy** bandwidth
//! over a multi-megabyte buffer, timed with the ARM generic timer
//! (`CNTPCT_EL0` / `CNTFRQ_EL0`), and prints MB/s via GEMDOS `Cconout`.
//!
//! Note: in QEMU this reflects the TCG emulation speed, not hardware
//! bandwidth; on a real Pi 4 it reports the actual figures. The buffers are
//! sized to exceed the Cortex-A72's 1 MiB L2 cache so the numbers reflect
//! main memory rather than cache.
//!
//! # Why the buffers are allocated, not static
//!
//! The kernel image — `.text`, `.data` and **`.bss`** — lives at the bottom
//! of the Atari ST-RAM window (`$000000–$DFFFFF`), and the GEMDOS ST-RAM
//! pool is what is left above it, exactly as EmuTOS' `membot = end_os`
//! (`references/emutos/bios/biosmem.c:66-72`). Two 2-MiB scratch buffers in
//! `.bss` therefore cost every program 4 MiB of `Malloc`-able ST-RAM for as
//! long as the machine runs — and, because the linker script pads the image
//! to `ALIGN(2M)` twice, crossing a 2-MiB boundary moves `.dma` and the
//! kernel stack up and takes a further 2 MiB with it. A diagnostic scratch
//! buffer does not belong there, so both buffers come from the **TT-RAM**
//! pool while the command runs and go back on return.
//!
//! That also makes the measurement honest. As statics, `SRC` was provably
//! all-zero for the whole program, so the optimiser folded every load of it
//! to `0`, deleted the read loop entirely (the disassembly had two adjacent
//! `mrs CNTPCT_EL0` where 64 passes should have been) and collapsed the
//! 64-pass copy into a single `memset`. Pool memory reached through an
//! opaque allocator call cannot be constant-folded, so all three loops now
//! survive optimisation.

use core::hint::black_box;

use tos_api::bdos::mem::traps::{mfree, mxalloc};
use tos_types::{Long, MX_TTRAM};

use crate::arch::aarch64::mmu::VA_OFFSET;

/// Per-buffer size (2 MiB > A72 L2). Two buffers → 4 MiB of TT-RAM,
/// held only while the command runs.
const SIZE: usize = 2 * 1024 * 1024;
const WORDS: usize = SIZE / 8;
/// Passes per test, so the total transfer is large enough to time well.
const ITERS: u64 = 64;
/// Pattern written into the source buffer before the read/copy passes, so
/// the timed loops move real data rather than whatever the pool held.
const FILL: u64 = 0x0F1E_2D3C_4B5A_6978;

/// One TT-RAM pool block, addressed through its kernel high-half alias and
/// released on drop.
///
/// The pool hands out **physical** addresses (`tos-api/src/bdos/mem`), and
/// EL1 code must reach DRAM through `pa + VA_OFFSET` (ADR-010/ADR-014) —
/// the same conversion `aes::resource` does for its `Mxalloc`ed blocks.
struct PoolBuf {
    /// The address `Mfree` expects back: the pool's own `m_start`.
    raw: Long,
    /// Kernel high-half alias of `raw`, valid for [`WORDS`] `u64`s.
    words: *mut u64,
}

impl PoolBuf {
    /// Take one [`SIZE`]-byte block from the TT-RAM pool.
    ///
    /// `MX_TTRAM` is deliberate rather than `MX_PREFTTRAM`: falling back to
    /// ST-RAM would put the scratch buffer back into the window this whole
    /// arrangement keeps it out of. Returns `None` if the pool cannot serve
    /// the request.
    fn alloc() -> Option<Self> {
        // SAFETY: kernel context on the boot CPU; the shell command runs
        // with the implicit BDOS lock held, like the `Mxalloc` calls in
        // `aes::resource::rsrc_load` and the D6 selftests.
        let raw = unsafe { mxalloc(SIZE as Long, MX_TTRAM) };
        if raw <= 0 {
            return None;
        }
        // Pool blocks are 16-byte aligned (`PoolConfig::AARCH64`), so the
        // `u64` view is aligned. Every pool address is below
        // `TOS_MD_MAX_END`, i.e. a positive `Long`, so the widening is
        // value-preserving.
        let words = ((raw as u64) + VA_OFFSET) as *mut u64;
        Some(Self { raw, words })
    }

    /// The block as a `u64` slice.
    ///
    /// # Safety
    ///
    /// The caller must not hold two overlapping borrows; each [`PoolBuf`]
    /// owns a distinct pool block, so distinct buffers never alias.
    unsafe fn as_mut(&mut self) -> &mut [u64] {
        // SAFETY: `words` aliases `SIZE` live, exclusively owned pool bytes
        // mapped Normal-Cacheable by the kernel DRAM mapping.
        unsafe { core::slice::from_raw_parts_mut(self.words, WORDS) }
    }
}

impl Drop for PoolBuf {
    fn drop(&mut self) {
        // SAFETY: `raw` is on the pool's allocated list; see `alloc`.
        unsafe { mfree(self.raw) };
    }
}

/// Run all three benchmarks and print the results.
pub fn run() {
    puts(b"Memory benchmark (2 MiB buffer, 64 passes = 128 MiB each)\r\n");
    puts(b"  (QEMU shows emulation speed; real HW shows true bandwidth)\r\n\r\n");

    let (Some(mut src), Some(mut dst)) = (PoolBuf::alloc(), PoolBuf::alloc()) else {
        puts(b"  needs 2 x 2 MiB of free TT-RAM - not available\r\n");
        return;
    };
    // SAFETY: `src` and `dst` own distinct pool blocks, so the two
    // borrows do not alias.
    let src_w = unsafe { src.as_mut() };
    // SAFETY: see above.
    let dst_w = unsafe { dst.as_mut() };

    // Fill both buffers once (fault in / prime caches, and give the read
    // and copy passes real data) before timing.
    for w in src_w.iter_mut() {
        *w = FILL;
    }
    for w in dst_w.iter_mut() {
        *w = 0;
    }

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
