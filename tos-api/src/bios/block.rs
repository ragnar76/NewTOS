//! Block-device interface the kernel supplies to the BIOS layer.
//!
//! Mirrors the `RasterBackend` pattern: the kernel installs a block
//! device via the [`Host`](crate::host::Host); `Rwabs` routes through it.
//! Kept independent of the concrete SD/EMMC driver so `tos-api` does not
//! depend on the kernel.

/// A 512-byte-sector block device. `read`/`write` return `true` on
/// success.
pub trait BlockDev: Sync {
    /// Total 512-byte block count (0 if unknown).
    fn block_count(&self) -> u64;
    /// Read `buf.len()/512` blocks from `lba` into `buf`.
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> bool;
    /// Write `buf.len()/512` blocks to `lba` from `buf`.
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> bool;
}
