//! `Getbpb` — parse a partition's FAT boot sector into a BIOS Parameter
//! Block. Spec: `docs/api/structures/bpb.md`, `docs/api/bios/07-getbpb.md`.
//!
//! Reduced 18-byte BPB (nine `UWORD`s). FAT12/16 only; FAT32 / exFAT
//! (`sectors_per_fat_16 == 0`) return `NULL`.

use core::cell::UnsafeCell;

use super::drive;
use crate::host;

/// BIOS Parameter Block (fixed Atari ABI, 18 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bpb {
    /// Sector size in bytes.
    pub recsiz: u16,
    /// Cluster size in sectors.
    pub clsiz: u16,
    /// Cluster size in bytes.
    pub clsizb: u16,
    /// Root-directory length in sectors.
    pub rdlen: u16,
    /// FAT size in sectors (one FAT).
    pub fsiz: u16,
    /// First sector of the last FAT.
    pub fatrec: u16,
    /// First data sector.
    pub datrec: u16,
    /// Number of data clusters.
    pub numcl: u16,
    /// Flags: `B_16` (0x01) = 16-bit FAT, `B_1FAT` (0x02) = single FAT.
    pub b_flags: u16,
}

impl Bpb {
    const ZERO: Bpb = Bpb {
        recsiz: 0,
        clsiz: 0,
        clsizb: 0,
        rdlen: 0,
        fsiz: 0,
        fatrec: 0,
        datrec: 0,
        numcl: 0,
        b_flags: 0,
    };
}

const B_16: u16 = 0x01;
const B_1FAT: u16 = 0x02;

struct BpbCell(UnsafeCell<[Bpb; 16]>);
// SAFETY: written and read only from the single-CPU, IRQ-off trap path.
unsafe impl Sync for BpbCell {}
static TABLE: BpbCell = BpbCell(UnsafeCell::new([Bpb::ZERO; 16]));

/// Parse a DOS/FAT boot sector into a [`Bpb`]. `part_sectors` is the
/// partition length (used when the in-sector total-sector fields are 0).
fn parse(sec: &[u8; 512], part_sectors: u32) -> Option<Bpb> {
    let recsiz = u16::from_le_bytes([sec[0x0B], sec[0x0C]]);
    let clsiz = sec[0x0D] as u16;
    let reserved = u16::from_le_bytes([sec[0x0E], sec[0x0F]]);
    let num_fats = sec[0x10] as u16;
    let root_entries = u16::from_le_bytes([sec[0x11], sec[0x12]]);
    let tot16 = u16::from_le_bytes([sec[0x13], sec[0x14]]);
    let fsiz16 = u16::from_le_bytes([sec[0x16], sec[0x17]]);
    let tot32 = u32::from_le_bytes([sec[0x20], sec[0x21], sec[0x22], sec[0x23]]);

    // Reject degenerate / FAT32 (16-bit FAT size 0) boot records.
    if recsiz == 0 || clsiz == 0 || num_fats == 0 || fsiz16 == 0 {
        return None;
    }

    let total = if tot16 != 0 {
        tot16 as u32
    } else if tot32 != 0 {
        tot32
    } else {
        part_sectors
    };

    let clsizb = recsiz.wrapping_mul(clsiz);
    let rdlen = (root_entries as u32 * 32).div_ceil(recsiz as u32) as u16;
    let fsiz = fsiz16;
    let fatrec = reserved + (num_fats - 1) * fsiz; // first sector of the last FAT
    let datrec = reserved + num_fats * fsiz + rdlen;
    let numcl = if total > datrec as u32 {
        ((total - datrec as u32) / clsiz as u32) as u16
    } else {
        0
    };

    let mut b_flags = 0;
    if numcl >= 4085 {
        b_flags |= B_16; // FAT16 (MS cluster-count threshold)
    }
    if num_fats == 1 {
        b_flags |= B_1FAT;
    }

    Some(Bpb { recsiz, clsiz, clsizb, rdlen, fsiz, fatrec, datrec, numcl, b_flags })
}

/// Extended FAT geometry: the 18-byte ABI [`Bpb`] plus the raw
/// `reserved` sector count and `num_fats`, which the write side needs to
/// address the first FAT and update every FAT copy.
pub struct Geometry {
    /// The ABI BPB.
    pub bpb: Bpb,
    /// Reserved sectors before the first FAT (partition-relative).
    pub reserved: u16,
    /// Number of FAT copies.
    pub num_fats: u8,
}

/// Read and parse a drive's boot sector into full geometry.
pub fn geometry(drive: u16) -> Option<Geometry> {
    if drive as usize >= 16 {
        return None;
    }
    let part = drive::resolve(drive)?;
    let blk = host::host().and_then(|h| h.block())?;
    let mut sec = [0u8; 512];
    if !blk.read_blocks(part.start_lba as u64, &mut sec) {
        return None;
    }
    let bpb = parse(&sec, part.count)?;
    Some(Geometry {
        bpb,
        reserved: u16::from_le_bytes([sec[0x0E], sec[0x0F]]),
        num_fats: sec[0x10],
    })
}

/// Read a drive's boot sector, parse its BPB, and cache it in the
/// per-drive static. Returns the parsed value (`None` on failure).
/// Shared by [`getbpb`] (the BIOS trap) and the BDOS FAT mount.
pub fn bpb_for(drive: u16) -> Option<Bpb> {
    if drive as usize >= 16 {
        return None;
    }
    let part = drive::resolve(drive)?;
    let blk = host::host().and_then(|h| h.block())?;
    let mut sec = [0u8; 512];
    if !blk.read_blocks(part.start_lba as u64, &mut sec) {
        return None;
    }
    let bpb = parse(&sec, part.count)?;
    // SAFETY: single trap path; one BPB slot per drive.
    let table = unsafe { &mut *TABLE.0.get() };
    table[drive as usize] = bpb;
    Some(bpb)
}

/// `Getbpb` (0x07) — read the drive's boot sector, parse its BPB into
/// the per-drive static, and return a pointer to it (`0` on failure).
/// The return is a **native 64-bit pointer** (the BIOS dispatch carries
/// `i64`).
pub fn getbpb(drive: u16) -> i64 {
    if bpb_for(drive).is_none() {
        return 0;
    }
    // SAFETY: single trap path; the slot was just written by `bpb_for`.
    let table = unsafe { &mut *TABLE.0.get() };
    &table[drive as usize] as *const Bpb as i64
}
