//! `Getmpb` (BIOS 0x00) — report the free ST-RAM region as a Memory
//! Partition Block.
//!
//! Spec: `docs/api/bios/00-getmpb.md`. EmuTOS fills a single static
//! memory descriptor `themd` describing `[membot, memtop)` and points the
//! caller's MPB free list at it (`bios/biosmem.c:134`): `mp_mfl = &themd`,
//! `mp_mal = NULL`, `mp_rover` left untouched; `themd` gets `m_link = NULL`,
//! `m_start = membot`, `m_length = memtop - membot`, `m_own = NULL`.
//!
//! NewTOS seeds its allocator directly from the AArch64 memory map, so
//! `Getmpb` is a **read-only** report of the ST-RAM pool extent (from
//! [`crate::bdos::mem::init::stram_range`]). Unlike EmuTOS it does **not**
//! rewrite the live BDOS free list — the "second call destroys your
//! allocator state" quirk is explicitly rejected (spec "NewTOS
//! implementation notes"), so a second call just re-reports the same
//! extent and is harmless.

use core::cell::UnsafeCell;

use tos_types::{Long, MD, MPB};

use crate::bdos::mem::init::stram_range;
use crate::trap::TrapArgs;

/// The single static memory descriptor `Getmpb` reports, mirroring
/// EmuTOS' `themd` (`bios/biosmem.c:144`). Re-filled on each call from
/// the current ST-RAM extent; the caller's MPB free list points here.
struct ThemdCell(UnsafeCell<MD>);

// SAFETY: `THEMD` is only touched by the BIOS dispatcher while filling a
// caller's MPB. BIOS traps run on the boot/trap CPU under the same
// single-owner discipline as the BDOS `MpbCell`; the pointer fields make
// the auto `Sync` absent, so we assert it explicitly.
unsafe impl Sync for ThemdCell {}

static THEMD: ThemdCell = ThemdCell(UnsafeCell::new(MD {
    m_link: core::ptr::null_mut(),
    m_start: 0,
    m_length: 0,
    m_own: core::ptr::null_mut(),
}));

/// `Getmpb` (0x00) — fill the caller-supplied MPB with a one-descriptor
/// free list covering the ST-RAM pool. `getmpb` is `void` in EmuTOS; the
/// result register is undefined, so we return `0` and callers read the
/// populated `*mpb`.
pub fn getmpb(args: &TrapArgs) -> Long {
    let mpb = args.arg_u64(0) as *mut MPB;
    if mpb.is_null() {
        return 0;
    }
    let (start, end) = stram_range();
    let md = THEMD.0.get();

    // SAFETY: `md` is our stable static; `mpb` is the caller's writable
    // MPB (a kernel pointer at EL1, a user pointer once the process-aware
    // BIOS path lands — same direct-pointer contract as `Rwabs`). Single
    // boot/trap CPU, so no concurrent access to `THEMD`.
    unsafe {
        (*md).m_link = core::ptr::null_mut();
        (*md).m_start = start;
        (*md).m_length = end - start;
        (*md).m_own = core::ptr::null_mut();
        // Write only mp_mfl / mp_mal; mp_rover is preserved, exactly as
        // EmuTOS leaves the caller's value (spec §Outputs).
        (*mpb).mp_mfl = md;
        (*mpb).mp_mal = core::ptr::null_mut();
    }
    0
}
