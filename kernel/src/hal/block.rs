//! Block device HAL trait.
//!
//! A 512-byte-sector storage device, board-independent. The bring-up
//! implementation is the RPi4 microSD card on the EMMC2 controller
//! (`drivers::bcm2711::emmc`); the BIOS `Rwabs` layer (Phase 08) and the
//! filesystem (Phase 09) sit on top.

/// Fixed logical sector size (TOS/FAT and SD both use 512 bytes).
pub const SECTOR_SIZE: usize = 512;

/// A block (sector) I/O error.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BlockError {
    /// No card / device not initialised.
    NotReady,
    /// A command or transfer timed out.
    Timeout,
    /// The controller reported a transfer error (CRC, etc.).
    Io,
    /// Operation not supported by this device.
    Unsupported,
}

/// A 512-byte-block storage device.
pub trait BlockDevice: Sync {
    /// Total number of addressable 512-byte blocks, or 0 if unknown.
    fn block_count(&self) -> u64;

    /// Read `buf.len() / 512` consecutive blocks starting at logical
    /// block address `lba` into `buf`. `buf.len()` must be a multiple of
    /// [`SECTOR_SIZE`].
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError>;

    /// Write `buf.len() / 512` consecutive blocks starting at `lba`.
    /// Default: unsupported (read-only bring-up).
    fn write_blocks(&self, _lba: u64, _buf: &[u8]) -> Result<(), BlockError> {
        Err(BlockError::Unsupported)
    }
}
