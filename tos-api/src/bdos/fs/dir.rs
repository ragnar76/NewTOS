//! Directory traversal (read-only).
//!
//! Mirrors the directory-scan core of EmuTOS `bdos/fsdir.c`. Walks a
//! FAT12/16 **root directory** entry by entry, yielding parsed 8.3
//! [`DirEntry`]s and skipping erased, long-file-name and volume-label
//! records. This is the shared infrastructure used both by the file
//! open path (`Fopen`, D4) and by `Fsfirst`/`Fsnext` (D5).
//!
//! On-disk entry layout: `docs/api/structures/ofd.md` (FCB, 32 bytes).
//! Sub-directory traversal (following a cluster chain rather than the
//! fixed root area) lands with `Dsetpath`.

use super::fat::Volume;
use crate::host;

const SECTOR: usize = 512;
const ENTRY: usize = 32;
const ENTRIES_PER_SECTOR: usize = SECTOR / ENTRY;

/// End-of-directory marker in the first name byte.
const END: u8 = 0x00;
/// Erased-entry marker in the first name byte.
const ERASED: u8 = 0xE5;

/// FAT attribute bits (`bdosdefs.h`).
/// Volume-label attribute.
pub const FA_VOL: u8 = 0x08;
/// Sub-directory attribute.
pub const FA_SUBDIR: u8 = 0x10;
/// Long-file-name records carry exactly this attribute pattern.
pub const FA_LFN: u8 = 0x0F;

/// A parsed 8.3 directory entry.
#[derive(Clone, Copy)]
pub struct DirEntry {
    /// Raw 11-byte FAT name (8 name + 3 ext, space-padded, uppercase).
    pub name: [u8; 11],
    /// `FA_*` attribute bits.
    pub attrib: u8,
    /// Packed FAT time.
    pub time: u16,
    /// Packed FAT date.
    pub date: u16,
    /// Starting cluster (low 16 bits; FAT12/16 has no high word).
    pub cluster: u32,
    /// File length in bytes.
    pub size: u32,
}

impl DirEntry {
    fn parse(b: &[u8]) -> DirEntry {
        let mut name = [0u8; 11];
        name.copy_from_slice(&b[0..11]);
        DirEntry {
            name,
            attrib: b[0x0B],
            time: u16::from_le_bytes([b[0x16], b[0x17]]),
            date: u16::from_le_bytes([b[0x18], b[0x19]]),
            cluster: u16::from_le_bytes([b[0x1A], b[0x1B]]) as u32,
            size: u32::from_le_bytes([b[0x1C], b[0x1D], b[0x1E], b[0x1F]]),
        }
    }
}

/// Iterator over the root directory of a [`Volume`]. Yields only real
/// 8.3 file/sub-directory entries (erased, LFN and volume-label records
/// are skipped). Reads one sector at a time; no sector cache yet (D2).
pub struct RootDir<'a> {
    vol: &'a Volume,
    /// Partition-relative sector currently buffered (`None` = not loaded).
    buf: [u8; SECTOR],
    rel_sector: u32,
    /// Index of the next entry within `buf`.
    idx: usize,
    /// Number of root-directory sectors already consumed.
    sectors_done: u32,
    /// `true` once the end-of-directory marker has been seen.
    done: bool,
}

impl<'a> RootDir<'a> {
    /// Begin a scan of `vol`'s root directory.
    pub fn new(vol: &'a Volume) -> RootDir<'a> {
        RootDir {
            vol,
            buf: [0u8; SECTOR],
            rel_sector: vol.root_sector(),
            idx: ENTRIES_PER_SECTOR, // force a load on first `next`
            sectors_done: 0,
            done: false,
        }
    }

    /// Read the buffered sector from the block device.
    fn load(&mut self) -> bool {
        match host::host().and_then(|h| h.block()) {
            Some(b) => b.read_blocks((self.vol.start_lba() + self.rel_sector) as u64, &mut self.buf),
            None => false,
        }
    }
}

impl Iterator for RootDir<'_> {
    type Item = DirEntry;

    fn next(&mut self) -> Option<DirEntry> {
        loop {
            if self.done {
                return None;
            }
            if self.idx >= ENTRIES_PER_SECTOR {
                // Advance to the next root-directory sector.
                if self.sectors_done >= self.vol.rdlen {
                    self.done = true;
                    return None;
                }
                if !self.load() {
                    self.done = true;
                    return None;
                }
                self.sectors_done += 1;
                self.rel_sector += 1;
                self.idx = 0;
            }
            let off = self.idx * ENTRY;
            self.idx += 1;
            let e = &self.buf[off..off + ENTRY];
            match e[0] {
                END => {
                    self.done = true;
                    return None;
                }
                ERASED => continue,
                _ => {}
            }
            // Skip long-file-name and volume-label records.
            if e[0x0B] == FA_LFN || e[0x0B] & FA_VOL != 0 {
                continue;
            }
            return Some(DirEntry::parse(e));
        }
    }
}

/// Convert a `name.ext` string to the 11-byte space-padded, uppercased
/// FAT short-name form (`"hello.txt"` → `b"HELLO   TXT"`). Returns
/// `None` if a component overflows 8 / 3 characters.
pub fn fcb_name(name: &str) -> Option<[u8; 11]> {
    let mut out = [b' '; 11];
    let bytes = name.as_bytes();
    let mut i = 0; // index into bytes
    let mut n = 0; // chars written to the 8-char name field
    while i < bytes.len() && bytes[i] != b'.' {
        if n >= 8 {
            return None;
        }
        out[n] = bytes[i].to_ascii_uppercase();
        n += 1;
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let mut x = 8;
        while i < bytes.len() {
            if x >= 11 {
                return None;
            }
            out[x] = bytes[i].to_ascii_uppercase();
            x += 1;
            i += 1;
        }
    }
    Some(out)
}

/// Find an 8.3 entry in the root directory by its FAT short name.
pub fn find_in_root(vol: &Volume, name11: &[u8; 11]) -> Option<DirEntry> {
    RootDir::new(vol).find(|e| &e.name == name11)
}
