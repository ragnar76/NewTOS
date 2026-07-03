//! TOS BIOS — Trap 13 (`svc #13`).
//!
//! Spec: `docs/api/bios/`. Bring-up scope: the block-device functions
//! (`Rwabs`, `Mediach`, `Drvmap`) on top of the host-supplied
//! [`block::BlockDev`], plus the console character calls
//! (`Bconstat`/`Bconin`/`Bconout`/`Bcostat`, see [`chardev`]). The cookie
//! jar lands later.
//!
//! Native AArch64 ABI (as for GEMDOS): function id in `x8`, arguments in
//! `x0..`, result in `x0`.

pub mod block;
pub mod bpb;
pub mod chardev;
pub mod drive;
pub mod vectors;

pub use drive::mount;

use tos_types::error::{EREADF, ESECNF, EUNDEV, EWRITF, E_OK};
use tos_types::Long;

use crate::host;
use crate::trap::TrapArgs;

const SECTOR: usize = 512;

// BIOS function ids.
const BCONSTAT: u16 = 0x01;
const BCONIN: u16 = 0x02;
const BCONOUT: u16 = 0x03;
const RWABS: u16 = 0x04;
const SETEXC: u16 = 0x05;
const BCOSTAT: u16 = 0x08;
const TICKCAL: u16 = 0x06;
const GETBPB: u16 = 0x07;
const MEDIACH: u16 = 0x09;
const DRVMAP: u16 = 0x0A;

/// System-timer period in milliseconds (50 Hz → 20 ms), per EmuTOS.
const TICKCAL_MS: Long = 20;

/// BIOS Trap-13 dispatcher. Returns the full 64-bit register value:
/// most functions return a `Long`, but `Getbpb` returns a native
/// `BPB *` pointer, so the result is widened to `i64`.
pub fn dispatch(args: &TrapArgs) -> i64 {
    match args.func_id() {
        BCONSTAT => chardev::bconstat(args) as i64,
        BCONIN => chardev::bconin(args) as i64,
        BCONOUT => chardev::bconout(args) as i64,
        BCOSTAT => chardev::bcostat(args) as i64,
        RWABS => rwabs(args) as i64,
        SETEXC => vectors::setexc(args),
        TICKCAL => TICKCAL_MS as i64,
        GETBPB => bpb::getbpb(args.arg_u16(0)),
        MEDIACH => mediach(args) as i64,
        DRVMAP => drvmap() as i64,
        _ => tos_types::error::EINVFN as i64,
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

    // Resolve the drive to its partition (start LBA + length).
    let part = match drive::resolve(drive) {
        Some(p) => p,
        None => return EUNDEV,
    };
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
    // Partition-relative sector → absolute LBA, with a range check.
    if sector + numb as u64 > part.count as u64 {
        return ESECNF;
    }
    let lba = part.start_lba as u64 + sector;
    let len = numb * SECTOR;
    let write = r_w & 1 != 0;

    // SAFETY: `adr` is the caller's buffer for `len` bytes. EL1 self-test
    // passes kernel memory; EL0 user-pointer handling lands with the
    // process-aware BIOS path.
    let ok = unsafe {
        if write {
            blk.write_blocks(lba, core::slice::from_raw_parts(adr as *const u8, len))
        } else {
            blk.read_blocks(lba, core::slice::from_raw_parts_mut(adr as *mut u8, len))
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

/// `Mediach` (0x09) — media-change probe. `drv` is in `x0`
/// (`docs/api/bios/09-mediach.md` §"Trap interface").
///
/// The contract validates the drive first: an invalid / unmounted drive
/// returns `EUNDEV` (-15). A valid drive returns `0` (`MEDIANOCHANGE`) —
/// the Pi's SD card is treated as non-removable, so it never reports a
/// change (spec §"Pi / BCM2711 binding", `docs/HARDWARE-PROFILE.md` §3.4).
fn mediach(args: &TrapArgs) -> Long {
    let drive = args.arg_u16(0);
    match drive::resolve(drive) {
        Some(_) => 0,     // valid drive → non-removable → no change
        None => EUNDEV,   // out-of-range / unmounted drive
    }
}

/// `Drvmap` (0x0A) — bitmap of present drives (bit `n` = drive `n`),
/// built from the partition table.
fn drvmap() -> Long {
    drive::drvbits() as Long
}
