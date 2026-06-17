//! BIOS drive table — maps TOS drive letters to partitions on the block
//! device (the SD card).
//!
//! TOS numbers drives `A:`=0, `B:`=1, `C:`=2, … Floppies would be A:/B:
//! (we have none), so the SD card's partitions are assigned from **C:**
//! upward. The MBR (sector 0) is parsed at [`mount`]; `Rwabs` then maps
//! `(drive, sector)` to an absolute LBA via [`resolve`].

use core::cell::UnsafeCell;

use crate::host;

const MAX_DRIVES: usize = 16;
/// First hard-disk drive letter (`C:`); A:/B: are reserved for floppies.
const FIRST_HD: usize = 2;

/// A mounted partition: its absolute start sector, length, and MBR type.
#[derive(Clone, Copy)]
pub struct Partition {
    /// Absolute starting LBA on the block device.
    pub start_lba: u32,
    /// Length in 512-byte sectors.
    pub count: u32,
    /// MBR partition type byte (e.g. 0x06 FAT16, 0x0B/0x0C FAT32).
    pub kind: u8,
}

struct TableCell(UnsafeCell<[Option<Partition>; MAX_DRIVES]>);
// SAFETY: filled once at boot (`mount`) and read from the single-CPU,
// IRQ-off trap path (`Rwabs`); never concurrently.
unsafe impl Sync for TableCell {}

static TABLE: TableCell = TableCell(UnsafeCell::new([None; MAX_DRIVES]));

/// MBR partition type bytes that denote a container/extended partition
/// (not a directly-mountable filesystem) — skipped in bring-up.
fn is_extended(kind: u8) -> bool {
    matches!(kind, 0x05 | 0x0F | 0x85)
}

/// Parse the MBR on the installed block device and (re)build the drive
/// table. Returns the number of drives mounted. A device with no MBR
/// partitions (a "super-floppy" FAT image) is mounted whole as `C:`.
pub fn mount() -> usize {
    let blk = match host::host().and_then(|h| h.block()) {
        Some(b) => b,
        None => return 0,
    };
    let mut sec0 = [0u8; 512];
    if !blk.read_blocks(0, &mut sec0) {
        return 0;
    }

    // SAFETY: boot CPU, single-threaded.
    let table = unsafe { &mut *TABLE.0.get() };
    *table = [None; MAX_DRIVES];

    let mut drive = FIRST_HD;
    let mut mounted = 0;

    if sec0[510] == 0x55 && sec0[511] == 0xAA {
        // Four primary partition entries at offset 0x1BE, 16 bytes each.
        let mut any = false;
        for i in 0..4 {
            let e = 0x1BE + i * 16;
            let kind = sec0[e + 4];
            let start = u32::from_le_bytes([sec0[e + 8], sec0[e + 9], sec0[e + 10], sec0[e + 11]]);
            let count = u32::from_le_bytes([sec0[e + 12], sec0[e + 13], sec0[e + 14], sec0[e + 15]]);
            if kind != 0 && count != 0 && !is_extended(kind) {
                any = true;
                if drive < MAX_DRIVES {
                    table[drive] = Some(Partition { start_lba: start, count, kind });
                    drive += 1;
                    mounted += 1;
                }
            }
        }
        if any {
            return mounted;
        }
    }

    // No partitions → the whole device is one volume on C:.
    let count = blk.block_count().min(u32::MAX as u64) as u32;
    table[FIRST_HD] = Some(Partition { start_lba: 0, count, kind: 0 });
    1
}

/// Resolve a TOS drive number to its partition.
pub fn resolve(drive: u16) -> Option<Partition> {
    if drive as usize >= MAX_DRIVES {
        return None;
    }
    // SAFETY: single trap path; table is only mutated at boot.
    unsafe { (*TABLE.0.get())[drive as usize] }
}

/// Bitmap of present drives (bit `n` = drive `n`), for `Drvmap`.
pub fn drvbits() -> u32 {
    let mut bits = 0u32;
    // SAFETY: single trap path.
    let table = unsafe { &*TABLE.0.get() };
    for (i, p) in table.iter().enumerate() {
        if p.is_some() {
            bits |= 1 << i;
        }
    }
    bits
}
