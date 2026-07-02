//! XBIOS screen group (Phase 10): `Physbase` / `Logbase` / `Getrez` /
//! `Setscreen` / `Setpalette` / `Setcolor`. Mirrors the API surface of
//! EmuTOS `bios/screen.c` on top of the host framebuffer
//! ([`crate::host::ScreenInfo`]).
//!
//! Spec: `docs/api/xbios/{02-physbase,03-logbase,04-getrez,05-setscreen,
//! 06-setpalette,07-setcolor}.md`.
//!
//! The logical / physical screen base and the resolution are kept as
//! software shadows (EmuTOS' `v_bas_ad` / `sshiftmod`). A shadow base of
//! `0` means "use the live hardware framebuffer". Setting the physical
//! base only updates the shadow — re-pointing the VideoCore scan-out at
//! an arbitrary buffer is a later refinement; software that double-
//! buffers by drawing into its own buffer and reading `Logbase` works.

use core::sync::atomic::{AtomicI64, AtomicU64, AtomicU8, Ordering};

use crate::host;
use crate::trap::TrapArgs;
use tos_types::Long;

/// Atari resolution codes (`include/biosdefs.h`). Our truecolor
/// framebuffer has no exact equivalent; `Getrez` returns a shadow whose
/// default reports the high-resolution code as a compatibility shim.
const ST_HIGH: i64 = 2;
const MIN_REZ: i16 = 0;
const MAX_REZ: i16 = 2;

/// Shadow logical / physical screen base (`0` = use the live FB).
static LOGBASE: AtomicU64 = AtomicU64::new(0);
static PHYSBASE: AtomicU64 = AtomicU64::new(0);
/// Shadow resolution code (`-1` = report the default).
static REZ: AtomicI64 = AtomicI64::new(-1);

/// Live hardware framebuffer base, or `0` if no display is up.
fn fb_base() -> u64 {
    host::host().and_then(|h| h.screen()).map_or(0, |s| s.base)
}

/// `Physbase` (0x02) — physical (displayed) screen base.
pub fn physbase() -> i64 {
    match PHYSBASE.load(Ordering::Relaxed) {
        0 => fb_base() as i64,
        p => p as i64,
    }
}

/// `Logbase` (0x03) — logical screen base (where drawing goes).
pub fn logbase() -> i64 {
    match LOGBASE.load(Ordering::Relaxed) {
        0 => fb_base() as i64,
        l => l as i64,
    }
}

/// `Getrez` (0x04) — current resolution code.
pub fn getrez() -> i64 {
    match REZ.load(Ordering::Relaxed) {
        r if r >= 0 => r,
        _ => ST_HIGH,
    }
}

/// `Setscreen` (0x05) — set logical/physical base and/or resolution.
/// `x0` = logLoc, `x1` = physLoc (each `> 0` to apply), `x2` = rez
/// (`-1` to leave unchanged), `x3` = videlmode (ignored — no Falcon).
pub fn setscreen(args: &TrapArgs) -> i64 {
    let log = args.arg_u64(0);
    let phys = args.arg_u64(1);
    let rez = args.arg_i16(2);

    if (log as i64) > 0 {
        LOGBASE.store(log, Ordering::Relaxed);
    }
    if (phys as i64) > 0 {
        PHYSBASE.store(phys, Ordering::Relaxed);
    }
    if rez == -1 {
        return 0;
    }
    if !(MIN_REZ..=MAX_REZ).contains(&rez) {
        return -1;
    }
    REZ.store(rez as i64, Ordering::Relaxed);
    0
}

/// `Setpalette` (0x06) — load a 16-entry ST palette. Our framebuffer is
/// truecolor (no hardware CLUT), so the palette is shadowed only; the
/// VDI keeps its own PEL→RGB map for indexed drawing. `x0` = palette ptr.
pub fn setpalette(args: &TrapArgs) -> i64 {
    let ptr = args.arg_u64(0);
    if ptr == 0 {
        return 0;
    }
    // SAFETY: caller-supplied 16-WORD palette (kernel pointer at EL1).
    let p = ptr as *const u16;
    let pal = palette();
    for (i, slot) in pal.iter_mut().enumerate() {
        *slot = unsafe { p.add(i).read() } & 0x0FFF; // ST: 0x0RGB, 3 bits/gun
    }
    0
}

/// `Setcolor` (0x07) — set or query one palette entry. `x0` = index
/// (0..15), `x1` = ST color value, or `-1` to query. Returns the old
/// value.
pub fn setcolor(args: &TrapArgs) -> i64 {
    let index = (args.arg_u16(0) & 0x0F) as usize;
    let value = args.arg_i16(1);
    let pal = palette();
    let old = pal[index] as Long as i64;
    if value >= 0 {
        pal[index] = (value as u16) & 0x0FFF;
    }
    old
}

// ── Cursconf (VT52 text-cursor config) ───────────────────────────────────
//
// `v_stat_0` flags (EmuTOS `vt52.h`): the cursor visible / flash-enable
// bits. NewTOS' GUI-oriented framebuffer console has no *blinking* text
// caret, so hide/show/flash track their state without a visible cursor —
// but the blink-rate get/set are honoured (a real, usable value), so this
// is a thin state model, not a stub.
const M_CFLASH: u8 = 0x02;
const M_CVIS: u8 = 0x10;
/// `v_stat_0` cursor bits; default = visible + flashing (EmuTOS).
static V_STAT_0: AtomicU8 = AtomicU8::new(M_CVIS | M_CFLASH);
/// `v_period` — cursor blink period in VBLs (EmuTOS default 30).
static V_PERIOD: AtomicU8 = AtomicU8::new(30);

/// `Cursconf` (0x15) — configure the VT52 text cursor (EmuTOS `cursconf`):
/// `function` 0 hide, 1 show, 2 disable flash, 3 enable flash, 4 set the
/// blink period (`operand` low byte, VBLs), 5 return the period; ≥ 6 no-op.
pub fn cursconf(args: &TrapArgs) -> i64 {
    let function = args.arg_i16(0);
    let operand = args.arg_i16(1);
    match function {
        0 => {
            V_STAT_0.fetch_and(!M_CVIS, Ordering::Relaxed);
        }
        1 => {
            V_STAT_0.fetch_or(M_CVIS, Ordering::Relaxed);
        }
        2 => {
            V_STAT_0.fetch_and(!M_CFLASH, Ordering::Relaxed);
        }
        3 => {
            V_STAT_0.fetch_or(M_CFLASH, Ordering::Relaxed);
        }
        4 => V_PERIOD.store(operand as u8, Ordering::Relaxed),
        5 => return V_PERIOD.load(Ordering::Relaxed) as i64,
        _ => {}
    }
    0
}

/// 16-entry ST palette shadow (0x0RGB, 3 bits per gun).
fn palette() -> &'static mut [u16; 16] {
    use core::cell::UnsafeCell;
    struct Pal(UnsafeCell<[u16; 16]>);
    // SAFETY: single-CPU, IRQ-off trap path.
    unsafe impl Sync for Pal {}
    static PAL: Pal = Pal(UnsafeCell::new([0u16; 16]));
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut *PAL.0.get() }
}
