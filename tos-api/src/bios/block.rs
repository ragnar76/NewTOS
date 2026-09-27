//! Block-device interface the kernel supplies to the BIOS layer.
//!
//! Mirrors the `RasterBackend` pattern: the kernel installs a block
//! device via the [`Host`](crate::host::Host); `Rwabs` routes through it.
//! Kept independent of the concrete SD/EMMC driver so `tos-api` does not
//! depend on the kernel.

use tos_types::Long;

/// What a block transfer returns: `Ok(())`, or the **TOS error code** of
/// the failure.
///
/// This is the shape EmuTOS' disk drivers have — `sd_rw`, `floppy_rw` and
/// `disk_rw` all return a LONG that is either 0 or a `gemerror.h` code
/// (`bios/sd.c:154`, `:187-190`), and `blkdev_rwabs` hands that code
/// straight back to the caller (`bios/blkdev.c:512-523`). Carrying the code
/// rather than a `bool` is what lets `Rwabs` distinguish `EWRITF` from
/// `EDRVNR`, and what lets the BDOS report a device error to the
/// application at all (EMMC audit finding 5,
/// `docs/api/hw/emmc-audit.md`).
pub type BlockResult = Result<(), Long>;

/// A 512-byte-sector block device.
///
/// `read_blocks`/`write_blocks` return `Ok(())` on success and the TOS
/// error code of the failure otherwise — `EREADF` (-11) / `EWRITF` (-10)
/// for a failed transfer, `EDRVNR` (-2) for a device that did not answer,
/// `EUNDEV` (-15) for a request the device cannot serve. The mapping from
/// the concrete driver's failures lives with the driver
/// (`docs/hardware/emmc-sd.md` §5).
pub trait BlockDev: Sync {
    /// Total 512-byte block count (0 if unknown).
    fn block_count(&self) -> u64;
    /// Read `buf.len()/512` blocks from `lba` into `buf`.
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> BlockResult;
    /// Write `buf.len()/512` blocks to `lba` from `buf`.
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> BlockResult;
}
