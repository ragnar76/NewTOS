//! XBIOS system group (Phase 11): `Random` / `Vsync` / `Kbdvbase`.
//! Mirrors EmuTOS `bios/xbios.c` (`random`/`kbdvbase`) and
//! `bios/screen.c` (`vsync`).
//!
//! Spec: `docs/api/xbios/{11-random,25-vsync,22-kbdvbase}.md`.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::host;

/// `Random` (0x11) — 24-bit pseudo-random via the EmuTOS LCG. Seeded
/// from the host frame clock on first use.
pub fn random() -> i64 {
    static RSEED: AtomicU32 = AtomicU32::new(0);
    let mut s = RSEED.load(Ordering::Relaxed);
    if s == 0 {
        let fc = host::host().map_or(1, |h| h.frame_clock());
        s = ((fc << 16) as u32).wrapping_add(fc as u32);
        if s == 0 {
            s = 1;
        }
    }
    // EmuTOS: rseed = rseed * 3141592621 + 1; return (rseed >> 8) & 0xFFFFFF.
    s = s.wrapping_mul(3_141_592_621).wrapping_add(1);
    RSEED.store(s, Ordering::Relaxed);
    ((s >> 8) & 0x00FF_FFFF) as i64
}

/// `Vsync` (0x25) — block until the next frame. Spins on the host frame
/// clock (~60 Hz, derived from the hardware timer); ±1 frame accuracy.
pub fn vsync() -> i64 {
    if let Some(h) = host::host() {
        let a = h.frame_clock();
        while h.frame_clock() == a {
            core::hint::spin_loop();
        }
    }
    0
}

/// `Blitmode` (0x40) — get/set the BLiTTER mode, returning the status
/// word. The Pi/BCM2711 has no Atari BLiTTER, so the contract answer is
/// `0x0000` (bit 1 = "present" clear → absent), for both the `-1` query
/// and any set request — there is nothing to enable. This is a real
/// status value, not an error: a blitter-presence probe correctly sees
/// "absent" and falls back to CPU/VideoCore blits.
///
/// Spec: `docs/api/xbios/40-blitmode.md` §"Pi / BCM2711 binding";
/// `docs/HARDWARE-PROFILE.md` §3.9.
pub fn blitmode() -> i64 {
    0x0000
}

/// IKBD vector table (Atari `struct kbdvecs`, 9 handler pointers).
/// Programs read it via `Kbdvbase` and may install their own handlers.
/// Native pointer width (not the m68k 4-byte layout); the IKBD-event
/// dispatch through these vectors lands with the process/AES model.
#[repr(C)]
#[derive(Default)]
struct KbdVecs {
    midivec: usize,
    vkbderr: usize,
    vmiderr: usize,
    statvec: usize,
    mousevec: usize,
    clockvec: usize,
    joyvec: usize,
    midisys: usize,
    ikbdsys: usize,
}

struct KbdVecsCell(UnsafeCell<KbdVecs>);
// SAFETY: single-CPU, IRQ-off trap path; programs poke it cooperatively.
unsafe impl Sync for KbdVecsCell {}
static KBDVECS: KbdVecsCell = KbdVecsCell(UnsafeCell::new(KbdVecs {
    midivec: 0,
    vkbderr: 0,
    vmiderr: 0,
    statvec: 0,
    mousevec: 0,
    clockvec: 0,
    joyvec: 0,
    midisys: 0,
    ikbdsys: 0,
}));

/// `Kbdvbase` (0x22) — pointer to the IKBD vector table.
pub fn kbdvbase() -> i64 {
    KBDVECS.0.get() as i64
}
