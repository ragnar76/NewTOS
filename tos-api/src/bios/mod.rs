//! TOS BIOS — Trap 13 (`svc #13`).
//!
//! Spec: `docs/api/bios/`. Bring-up scope: the block-device functions
//! (`Rwabs`, `Mediach`, `Drvmap`) on top of the host-supplied
//! [`block::BlockDev`]. Console BIOS calls (`Bconin`/`Bconout`) and the
//! cookie jar land later.
//!
//! Native AArch64 ABI (as for GEMDOS): function id in `x8`, arguments in
//! `x0..`, result in `x0`.

pub mod block;

use tos_types::error::{EREADF, EUNDEV, EWRITF, E_OK};
use tos_types::Long;

use crate::host;
use crate::trap::TrapArgs;

const SECTOR: usize = 512;

// BIOS function ids.
const RWABS: u16 = 0x04;
const MEDIACH: u16 = 0x09;
const DRVMAP: u16 = 0x0A;

/// BIOS Trap-13 dispatcher.
pub fn dispatch(args: &TrapArgs) -> Long {
    match args.func_id() {
        RWABS => rwabs(args),
        MEDIACH => mediach(args),
        DRVMAP => drvmap(),
        _ => tos_types::error::EINVFN,
    }
}

/// `Rwabs` (0x04) — read/write absolute sectors.
///
/// `x0` = r_w (bit 0: 0 read / 1 write), `x1` = buffer, `x2` = count,
/// `x3` = first sector (or `-1` ⇒ use `x5`), `x4` = drive, `x5` = lfirst.
fn rwabs(args: &TrapArgs) -> Long {
    let r_w = args.arg_i16(0);
    let adr = args.arg_u64(1);
    let numb = args.arg_u16(2) as usize;
    let first = args.arg_i16(3);
    let drive = args.arg_u16(4);
    let lfirst = args.arg_i32(5);

    // Bring-up: a single block device on drive 0.
    if drive != 0 {
        return EUNDEV;
    }
    let blk = match host::host().and_then(|h| h.block()) {
        Some(b) => b,
        None => return EUNDEV,
    };
    if numb == 0 {
        return E_OK;
    }
    let sector = if first == -1 {
        lfirst as u32 as u64
    } else {
        first as u16 as u64
    };
    let len = numb * SECTOR;
    let write = r_w & 1 != 0;

    // SAFETY: `adr` is the caller's buffer for `len` bytes. EL1 self-test
    // passes kernel memory; EL0 user-pointer handling lands with the
    // process-aware BIOS path.
    let ok = unsafe {
        if write {
            blk.write_blocks(sector, core::slice::from_raw_parts(adr as *const u8, len))
        } else {
            blk.read_blocks(sector, core::slice::from_raw_parts_mut(adr as *mut u8, len))
        }
    };
    if ok {
        E_OK
    } else if write {
        EWRITF
    } else {
        EREADF
    }
}

/// `Mediach` (0x09) — media-change probe. The SD card is treated as
/// non-removable, so it never reports a change (`0` = unchanged).
fn mediach(_args: &TrapArgs) -> Long {
    0
}

/// `Drvmap` (0x0A) — bitmap of present drives. Bring-up: bit 0 = the SD
/// card if a block device is installed.
fn drvmap() -> Long {
    if host::host().and_then(|h| h.block()).is_some() {
        1
    } else {
        0
    }
}
