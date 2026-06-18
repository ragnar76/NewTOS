//! FAT12/16 table and cluster-chain reader (read-only).
//!
//! Mirrors EmuTOS `bdos/fsfat.c`. A [`Volume`] caches the per-drive
//! geometry derived from the BPB (see `docs/api/structures/bpb.md`) and
//! the partition's absolute start LBA, and exposes the two primitives the
//! rest of the file system needs:
//!
//! - [`Volume::fat_entry`] — follow the FAT to the next cluster.
//! - [`Volume::read_cluster`] — read a whole cluster's data sectors.
//!
//! All sector numbers in the BPB are **partition-relative**; this module
//! adds `start_lba` to reach absolute LBAs on the block device. No sector
//! cache yet (Phase 09 D2) — each access reads through the block device.

use crate::bios::bpb::{self, Bpb};
use crate::bios::drive;
use crate::host;

const SECTOR: usize = 512;
const B_16: u16 = 0x01;

/// FAT12 end-of-chain threshold (entries `>= 0xFF8` mark the last
/// cluster). FAT16 uses `0xFFF8`.
const EOC12: u32 = 0x0FF8;
const EOC16: u32 = 0xFFF8;

/// Mounted FAT volume geometry for one TOS drive.
#[derive(Clone, Copy)]
pub struct Volume {
    /// Absolute start LBA of the partition on the block device.
    start_lba: u32,
    /// Bytes per sector.
    pub recsiz: u32,
    /// Sectors per cluster.
    pub clsiz: u32,
    /// Bytes per cluster.
    pub clsizb: u32,
    /// Root-directory length in sectors.
    pub rdlen: u32,
    /// Partition-relative first sector of the last FAT (read source).
    fatrec: u32,
    /// Partition-relative first data sector (cluster 2).
    datrec: u32,
    /// Number of data clusters.
    pub numcl: u32,
    /// `true` = FAT16, `false` = FAT12.
    fat16: bool,
}

impl Volume {
    /// Mount the FAT volume on `drive`: resolve its partition and parse
    /// the BPB. Returns `None` for an absent drive or a non-FAT12/16
    /// boot record.
    pub fn mount(drive: u16) -> Option<Volume> {
        let part = drive::resolve(drive)?;
        let bpb: Bpb = bpb::bpb_for(drive)?;
        Some(Volume {
            start_lba: part.start_lba,
            recsiz: bpb.recsiz as u32,
            clsiz: bpb.clsiz as u32,
            clsizb: bpb.clsizb as u32,
            rdlen: bpb.rdlen as u32,
            fatrec: bpb.fatrec as u32,
            datrec: bpb.datrec as u32,
            numcl: bpb.numcl as u32,
            fat16: bpb.b_flags & B_16 != 0,
        })
    }

    /// Absolute start LBA of this volume's partition.
    pub fn start_lba(&self) -> u32 {
        self.start_lba
    }

    /// Partition-relative first sector of the root directory: it sits
    /// directly before the data area (`datrec - rdlen`).
    pub fn root_sector(&self) -> u32 {
        self.datrec - self.rdlen
    }

    /// Read one partition-relative sector into `buf`. Public entry for the
    /// directory search, which walks the fixed root area sector by sector.
    pub fn read_rel_sector(&self, rel_sector: u32, buf: &mut [u8; SECTOR]) -> bool {
        self.read_rel(rel_sector, buf)
    }

    /// Read one partition-relative sector into `buf` (must be 512 bytes).
    fn read_rel(&self, rel_sector: u32, buf: &mut [u8; SECTOR]) -> bool {
        match host::host().and_then(|h| h.block()) {
            Some(b) => b.read_blocks((self.start_lba + rel_sector) as u64, buf),
            None => false,
        }
    }

    /// `true` if `cluster` is a valid data cluster (2..numcl+2).
    pub fn is_data_cluster(&self, cluster: u32) -> bool {
        cluster >= 2 && cluster < self.numcl + 2
    }

    /// `true` if `entry` is an end-of-chain marker for this FAT width.
    pub fn is_eoc(&self, entry: u32) -> bool {
        entry >= if self.fat16 { EOC16 } else { EOC12 }
    }

    /// Read the FAT entry for `cluster` (the next cluster in the chain,
    /// or an end-of-chain marker). `None` on a read error.
    pub fn fat_entry(&self, cluster: u32) -> Option<u32> {
        if self.fat16 {
            let off = cluster * 2;
            let sec = self.fatrec + off / self.recsiz;
            let within = (off % self.recsiz) as usize;
            let mut b = [0u8; SECTOR];
            if !self.read_rel(sec, &mut b) {
                return None;
            }
            // A 2-byte LE entry at an even offset never crosses a sector.
            Some(u16::from_le_bytes([b[within], b[within + 1]]) as u32)
        } else {
            // FAT12: 12-bit packed; an entry can straddle two sectors, so
            // read the two bytes individually.
            let off = cluster + cluster / 2; // cluster * 1.5
            let lo = self.fat12_byte(off)?;
            let hi = self.fat12_byte(off + 1)?;
            let raw = (lo as u32) | ((hi as u32) << 8);
            Some(if cluster & 1 == 0 { raw & 0x0FFF } else { raw >> 4 })
        }
    }

    /// Read a single byte at partition-relative FAT byte offset `off`.
    fn fat12_byte(&self, off: u32) -> Option<u8> {
        let sec = self.fatrec + off / self.recsiz;
        let within = (off % self.recsiz) as usize;
        let mut b = [0u8; SECTOR];
        if !self.read_rel(sec, &mut b) {
            return None;
        }
        Some(b[within])
    }

    /// Partition-relative first sector of a data cluster.
    fn cluster_sector(&self, cluster: u32) -> u32 {
        self.datrec + (cluster - 2) * self.clsiz
    }

    /// Read one 512-byte sector (`sec_in_cluster` within `cluster`) into
    /// `buf`. Used by the read-side file API to copy a byte range without
    /// buffering a whole (possibly large) cluster on the stack.
    pub fn read_cluster_sector(
        &self,
        cluster: u32,
        sec_in_cluster: u32,
        buf: &mut [u8; SECTOR],
    ) -> bool {
        if !self.is_data_cluster(cluster) || sec_in_cluster >= self.clsiz {
            return false;
        }
        self.read_rel(self.cluster_sector(cluster) + sec_in_cluster, buf)
    }

    /// Read all `clsizb` bytes of `cluster` into `buf` (which must be at
    /// least `clsizb` long). `false` on an invalid cluster or read error.
    pub fn read_cluster(&self, cluster: u32, buf: &mut [u8]) -> bool {
        if !self.is_data_cluster(cluster) || buf.len() < self.clsizb as usize {
            return false;
        }
        let lba = (self.start_lba + self.cluster_sector(cluster)) as u64;
        match host::host().and_then(|h| h.block()) {
            Some(b) => b.read_blocks(lba, &mut buf[..self.clsizb as usize]),
            None => false,
        }
    }
}
