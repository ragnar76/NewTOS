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
    /// The **card** rejected the command: its response carried one of the
    /// error bits of the card status word (SD Physical Layer Simplified
    /// Spec Ver. 9.10 Table 4-42 — out of range, address error, write
    /// protect violation, internal ECC failure, …; the full table is in
    /// `docs/hardware/emmc-sd.md` §5).
    ///
    /// Distinct from [`BlockError::Io`] on purpose: `Io` means the transfer
    /// itself broke (CRC, end bit, controller timeout), `Card` means the
    /// transfer was refused and never happened. Both are a failed access
    /// for the caller, but only the second one says the request was wrong.
    Card,
    /// The requested block is outside the address space this device can be
    /// asked about at all — the host-side counterpart of the card's
    /// `OUT_OF_RANGE`, for an address the command word cannot even carry
    /// (a byte-addressed SDSC card whose `lba × 512` does not fit the
    /// 32-bit argument, or a block number above `u32::MAX`).
    ///
    /// Reported rather than truncated: a wrapped address would silently
    /// hit a **different, valid** sector, which is the worst possible
    /// outcome for a write.
    OutOfRange,
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
