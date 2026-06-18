//! GEMDOS drive calls: `Dgetdrv` / `Dsetdrv` / `Dfree`. Mirrors EmuTOS
//! `bdos/fsmain.c` (`xgetdrv`/`xsetdrv`) and `bdos/fsfat.c` (`xgetfree`).
//!
//! Spec: `docs/api/gemdos/drive/{0e-dsetdrv,19-dgetdrv}.md`,
//! `docs/api/gemdos/directory/36-dfree.md`.
//!
//! The current drive is a global stand-in for the per-process
//! `p_curdrv` (single foreground caller for now), defaulting to `C:`
//! (the first hard-disk drive). This is also the implicit drive used by
//! `Fopen`/`Fsfirst` when a path carries no `X:` prefix.

use core::sync::atomic::{AtomicU16, Ordering};

use super::fat::Volume;
use crate::bios::drive as bios_drive;
use crate::trap::TrapArgs;
use tos_types::error::{ERROR, E_OK};
use tos_types::Long;

const BLKDEVNUM: u16 = 16;
/// Default current drive: `C:` (drive index 2; A:/B: are floppy slots).
const DEFAULT_DRIVE: u16 = 2;

static CUR_DRIVE: AtomicU16 = AtomicU16::new(DEFAULT_DRIVE);

/// The current drive index (0 = A:). Used by the path parsers when a
/// path has no explicit `X:` prefix.
pub fn current_drive() -> u16 {
    CUR_DRIVE.load(Ordering::Relaxed)
}

/// `Dgetdrv` (0x19) — return the current drive (0 = A:).
pub fn dgetdrv(_args: &TrapArgs) -> Long {
    CUR_DRIVE.load(Ordering::Relaxed) as Long
}

/// `Dsetdrv` (0x0E) — set the current drive (0-based), return the BIOS
/// drive bitmap. Out-of-range indices are silently ignored (no existence
/// check), per the historical Atari behaviour.
pub fn dsetdrv(args: &TrapArgs) -> Long {
    let drv = args.arg_u16(0);
    if drv < BLKDEVNUM {
        CUR_DRIVE.store(drv, Ordering::Relaxed);
    }
    bios_drive::drvbits() as Long
}

/// `Dfree` (0x36) — fill a 4-LONG buffer with free/total clusters,
/// sector size and cluster size. `drv`: 0 = current, 1 = A:, 2 = B:, …
/// (note the `+1` offset, unlike `Dsetdrv`). Returns `ERROR` (-1) if the
/// drive cannot be mounted.
pub fn dfree(args: &TrapArgs) -> Long {
    let buf = args.arg_u64(0);
    let drv = args.arg_u16(1);
    let index = if drv == 0 {
        current_drive()
    } else {
        drv - 1
    };

    let vol = match Volume::mount(index) {
        Some(v) => v,
        None => return ERROR,
    };
    let free = vol.count_free_clusters();

    // 4 LONGs: free clusters, total clusters, bytes/sector, sectors/cluster.
    let out = [free, vol.numcl, vol.recsiz, vol.clsiz];
    // SAFETY: `buf` is the caller's 16-byte buffer (kernel buffer at EL1).
    let p = buf as *mut u8;
    for (i, v) in out.iter().enumerate() {
        unsafe {
            p.add(i * 4).copy_from(v.to_le_bytes().as_ptr(), 4);
        }
    }
    E_OK
}
