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

use core::cell::UnsafeCell;

use crate::bios::bpb::{self, Bpb};
use crate::bios::drive;

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
    /// Partition-relative first sector of the first FAT.
    fat1: u32,
    /// Number of FAT copies (updated together on write).
    num_fats: u32,
    /// FAT size in sectors (one copy).
    fsiz: u32,
    /// Partition-relative first data sector (cluster 2).
    datrec: u32,
    /// Number of data clusters.
    pub numcl: u32,
    /// `true` = FAT16, `false` = FAT12.
    fat16: bool,
}

/// Per-drive logged-in geometry — EmuTOS's `drvtbl[]` / `drvsel`
/// (`bdos/fsdrive.c:59,94-105`) in the shape this file system needs.
///
/// `ckdrv` reads a drive's boot sector through `Getbpb` **once**, when the
/// drive is not yet in `drvsel`, and keeps the derived media descriptor from
/// then on; every later BDOS call on a logged-in drive touches no sector to
/// find its geometry. [`Volume::mount`] is our `ckdrv`, so it caches the same
/// way. Without this every path-resolving GEMDOS call — `Fsfirst`, each
/// `Fsnext`, `Fopen`, `Dgetpath`, `Dsetpath` — pays one physical boot-sector
/// read, which on the Pi 4's single-block PIO SD path is ~512 us apiece
/// (measured: `tosdump/reference/TERADESK-GDTRACE-r1.BIN`, where `Dgetpath`,
/// whose only device access is this mount, means 512.5 us while the calls that
/// touch no device mean 0).
struct Mounted(UnsafeCell<[Option<Volume>; 16]>);
// SAFETY: written and read only from the single-CPU, IRQ-off trap path.
unsafe impl Sync for Mounted {}
static MOUNTED: Mounted = Mounted(UnsafeCell::new([None; 16]));

/// Log every drive out, so the next [`Volume::mount`] re-reads its boot
/// sector. EmuTOS clears the drive's `drvsel` bit and frees its `DMD` when the
/// media changes (`bdos/bdosmain.c:456-482`); NewTOS has one moment with the
/// same meaning — the BIOS re-scanning the partition table
/// (`crate::bios::mount`), which can move or replace every partition.
pub fn log_out_all() {
    // SAFETY: single trap path; no outstanding borrow of the table.
    let table = unsafe { &mut *MOUNTED.0.get() };
    *table = [None; 16];
}

impl Volume {
    /// Mount the FAT volume on `drive`: resolve its partition and parse
    /// the BPB. Returns `None` for an absent drive or a non-FAT12/16
    /// boot record.
    ///
    /// This is EmuTOS `ckdrv` (`bdos/fsdrive.c:77-105`): a drive that is
    /// already logged in is answered from [`MOUNTED`] without touching the
    /// block device; only the first call for a drive — and any call for a
    /// drive that failed to log in, exactly as `ckdrv` retries `Getbpb` while
    /// the `drvsel` bit stays clear — reads its boot sector.
    pub fn mount(drive: u16) -> Option<Volume> {
        if drive as usize >= 16 {
            return None;
        }
        // SAFETY: single trap path; no outstanding borrow of the table.
        let table = unsafe { &mut *MOUNTED.0.get() };
        if let Some(v) = table[drive as usize] {
            return Some(v);
        }
        let v = Self::log_in(drive)?;
        table[drive as usize] = Some(v);
        Some(v)
    }

    /// Read and parse the drive's boot sector into a [`Volume`] — EmuTOS
    /// `Getbpb` + `log_media` (`bdos/fsdrive.c:96-104`).
    fn log_in(drive: u16) -> Option<Volume> {
        let part = drive::resolve(drive)?;
        let geo = bpb::geometry(drive)?;
        let bpb: Bpb = geo.bpb;
        Some(Volume {
            start_lba: part.start_lba,
            recsiz: bpb.recsiz as u32,
            clsiz: bpb.clsiz as u32,
            clsizb: bpb.clsizb as u32,
            rdlen: bpb.rdlen as u32,
            fatrec: bpb.fatrec as u32,
            fat1: geo.reserved as u32,
            num_fats: geo.num_fats as u32,
            fsiz: bpb.fsiz as u32,
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

    /// Map the `n`-th sector of a directory to a partition-relative
    /// sector. `start_cluster == 0` denotes the fixed root directory;
    /// otherwise the sub-directory's cluster chain is followed. `None`
    /// once the directory's sectors are exhausted.
    pub fn dir_nth_sector(&self, start_cluster: u32, n: u32) -> Option<u32> {
        if start_cluster == 0 {
            // Fixed root directory.
            if n < self.rdlen {
                Some(self.root_sector() + n)
            } else {
                None
            }
        } else {
            // Sub-directory: walk the cluster chain to the n-th sector.
            let cl_index = n / self.clsiz;
            let sec_in_cl = n % self.clsiz;
            let mut cl = start_cluster;
            for _ in 0..cl_index {
                match self.fat_entry(cl) {
                    Some(x) if !self.is_eoc(x) && self.is_data_cluster(x) => cl = x,
                    _ => return None,
                }
            }
            if !self.is_data_cluster(cl) {
                return None;
            }
            Some(self.cluster_sector(cl) + sec_in_cl)
        }
    }

    /// Number of clusters per directory cluster (used by the directory
    /// grow path); the root directory does not grow.
    pub fn dir_max_sectors(&self, start_cluster: u32) -> u32 {
        if start_cluster == 0 {
            self.rdlen
        } else {
            // Sub-directories grow on demand; report a generous bound for
            // a full scan (chain walking stops at end-of-chain anyway).
            self.numcl * self.clsiz
        }
    }

    /// Read one partition-relative sector into `buf`. Public entry for the
    /// directory search, which walks the fixed root area sector by sector.
    pub fn read_rel_sector(&self, rel_sector: u32, buf: &mut [u8; SECTOR]) -> bool {
        self.read_rel(rel_sector, buf)
    }

    /// Write one partition-relative sector from `buf` through the cache.
    /// Public entry for the directory write helpers.
    pub fn write_rel_sector(&self, rel_sector: u32, buf: &[u8; SECTOR]) -> bool {
        self.write_rel(rel_sector, buf)
    }

    /// Read one partition-relative sector into `buf` (must be 512 bytes),
    /// through the sector cache.
    fn read_rel(&self, rel_sector: u32, buf: &mut [u8; SECTOR]) -> bool {
        super::cache::read((self.start_lba + rel_sector) as u64, buf)
    }

    /// Write one partition-relative sector from `buf` through the cache.
    fn write_rel(&self, rel_sector: u32, buf: &[u8; SECTOR]) -> bool {
        super::cache::write((self.start_lba + rel_sector) as u64, buf)
    }

    /// `true` if FAT writes are supported on this volume (FAT16 only for
    /// now; FAT12 packed-entry writes land with floppy support).
    pub fn is_writable(&self) -> bool {
        self.fat16
    }

    /// FAT16 end-of-chain marker written for a newly allocated cluster.
    pub const EOC: u32 = 0xFFFF;

    /// Write a FAT16 entry in **every** FAT copy. Returns `false` on a
    /// non-FAT16 volume or a read/write error.
    pub fn set_fat_entry(&self, cluster: u32, value: u32) -> bool {
        if !self.fat16 {
            return false;
        }
        // F8 hardening: reject an out-of-range cluster before the
        // `cluster * 2` offset math (see `fat_entry`); a defined `false`
        // (write failed) instead of an overflow panic / FAT-sector wrap.
        if !self.is_data_cluster(cluster) {
            return false;
        }
        let off = cluster * 2;
        let sec_in_fat = off / self.recsiz;
        let within = (off % self.recsiz) as usize;
        let mut buf = [0u8; SECTOR];
        for f in 0..self.num_fats {
            let sec = self.fat1 + f * self.fsiz + sec_in_fat;
            if !self.read_rel(sec, &mut buf) {
                return false;
            }
            buf[within..within + 2].copy_from_slice(&(value as u16).to_le_bytes());
            if !self.write_rel(sec, &buf) {
                return false;
            }
        }
        true
    }

    /// Allocate the first free cluster: mark it end-of-chain and return
    /// its number. `None` if the volume is full or not writable.
    pub fn alloc_cluster(&self) -> Option<u32> {
        if !self.fat16 {
            return None;
        }
        let last = self.numcl + 2;
        let eps = self.recsiz / 2;
        let nsec = (last * 2).div_ceil(self.recsiz);
        let mut buf = [0u8; SECTOR];
        for s in 0..nsec {
            if !self.read_rel(self.fatrec + s, &mut buf) {
                return None;
            }
            for i in 0..eps {
                let cl = s * eps + i;
                if cl < 2 {
                    continue;
                }
                if cl >= last {
                    return None;
                }
                let b = (i * 2) as usize;
                if u16::from_le_bytes([buf[b], buf[b + 1]]) == 0 && self.set_fat_entry(cl, Self::EOC)
                {
                    return Some(cl);
                }
            }
        }
        None
    }

    /// Free a whole cluster chain starting at `start` (sets each entry to
    /// the free marker `0`).
    pub fn free_chain(&self, start: u32) -> bool {
        let mut cl = start;
        while self.is_data_cluster(cl) {
            let next = match self.fat_entry(cl) {
                Some(n) => n,
                None => return false,
            };
            if !self.set_fat_entry(cl, 0) {
                return false;
            }
            if self.is_eoc(next) || !self.is_data_cluster(next) {
                break;
            }
            cl = next;
        }
        true
    }

    /// Write one data sector (`sec_in_cluster` within `cluster`) from
    /// `buf` through the cache.
    pub fn write_cluster_sector(
        &self,
        cluster: u32,
        sec_in_cluster: u32,
        buf: &[u8; SECTOR],
    ) -> bool {
        if !self.is_data_cluster(cluster) || sec_in_cluster >= self.clsiz {
            return false;
        }
        self.write_rel(self.cluster_sector(cluster) + sec_in_cluster, buf)
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
    /// or an end-of-chain marker). `None` on a read error **or** an
    /// out-of-range cluster.
    ///
    /// The bound check (F8 hardening) rejects any `cluster` outside the
    /// partition's valid data range `2..=numcl+1` **before** the
    /// `cluster * 2` (FAT16) / `cluster + cluster/2` (FAT12) offset math,
    /// so a corrupt directory entry or an uninitialised DTA cluster number
    /// yields a defined `None` (treated as end-of-chain / error by every
    /// caller) instead of a debug overflow panic or a release wrap into an
    /// unrelated FAT sector. No behaviour change for valid clusters.
    pub fn fat_entry(&self, cluster: u32) -> Option<u32> {
        if !self.is_data_cluster(cluster) {
            return None;
        }
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

    /// Count free (zero) clusters in the FAT, over the data-cluster range
    /// `2..numcl+2`. FAT16 scans the FAT sectors directly (EmuTOS
    /// `countfree16`); FAT12 falls back to per-entry decoding.
    pub fn count_free_clusters(&self) -> u32 {
        let last = self.numcl + 2;
        let mut free = 0u32;
        if self.fat16 {
            let eps = self.recsiz / 2; // 16-bit entries per sector
            let nsec = (last * 2).div_ceil(self.recsiz);
            let mut sbuf = [0u8; SECTOR];
            for s in 0..nsec {
                if !self.read_rel(self.fatrec + s, &mut sbuf) {
                    break;
                }
                for i in 0..eps {
                    let cl = s * eps + i;
                    if cl < 2 {
                        continue;
                    }
                    if cl >= last {
                        return free;
                    }
                    let b = (i * 2) as usize;
                    if u16::from_le_bytes([sbuf[b], sbuf[b + 1]]) == 0 {
                        free += 1;
                    }
                }
            }
        } else {
            for c in 2..last {
                if matches!(self.fat_entry(c), Some(0)) {
                    free += 1;
                }
            }
        }
        free
    }

    /// Read all `clsizb` bytes of `cluster` into `buf` (which must be at
    /// least `clsizb` long), sector by sector through the cache. `false`
    /// on an invalid cluster or read error.
    pub fn read_cluster(&self, cluster: u32, buf: &mut [u8]) -> bool {
        if !self.is_data_cluster(cluster) || buf.len() < self.clsizb as usize {
            return false;
        }
        let first = self.cluster_sector(cluster);
        let mut s = [0u8; SECTOR];
        for i in 0..self.clsiz {
            if !self.read_rel(first + i, &mut s) {
                return false;
            }
            let off = (i as usize) * SECTOR;
            buf[off..off + SECTOR].copy_from_slice(&s);
        }
        true
    }
}
