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
    /// Parse a 32-byte on-disk directory entry (FCB layout).
    pub fn parse(b: &[u8]) -> DirEntry {
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

/// Iterator over a directory of a [`Volume`]. `start_cluster == 0` is
/// the fixed root directory; otherwise a sub-directory's cluster chain.
/// Yields only real 8.3 file/sub-directory entries (erased, LFN and
/// volume-label records are skipped). Reads through the sector cache.
pub struct DirIter<'a> {
    vol: &'a Volume,
    start_cluster: u32,
    buf: [u8; SECTOR],
    /// Index of the next entry within `buf`.
    idx: usize,
    /// Number of directory sectors already consumed.
    sectors_done: u32,
    /// `true` once the end-of-directory marker has been seen.
    done: bool,
}

impl<'a> DirIter<'a> {
    /// Begin a scan of the directory whose first cluster is `start`
    /// (0 = root directory).
    pub fn new(vol: &'a Volume, start: u32) -> DirIter<'a> {
        DirIter {
            vol,
            start_cluster: start,
            buf: [0u8; SECTOR],
            idx: ENTRIES_PER_SECTOR, // force a load on first `next`
            sectors_done: 0,
            done: false,
        }
    }

    /// Begin a scan of `vol`'s root directory.
    pub fn root(vol: &'a Volume) -> DirIter<'a> {
        DirIter::new(vol, 0)
    }
}

impl Iterator for DirIter<'_> {
    type Item = DirEntry;

    fn next(&mut self) -> Option<DirEntry> {
        loop {
            if self.done {
                return None;
            }
            if self.idx >= ENTRIES_PER_SECTOR {
                // Advance to the next directory sector (chain-aware).
                let rel = match self.vol.dir_nth_sector(self.start_cluster, self.sectors_done) {
                    Some(r) => r,
                    None => {
                        self.done = true;
                        return None;
                    }
                };
                if !self.vol.read_rel_sector(rel, &mut self.buf) {
                    self.done = true;
                    return None;
                }
                self.sectors_done += 1;
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
    find_in_dir(vol, 0, name11)
}

/// Find an 8.3 entry in a directory (0 = root) by its FAT short name.
/// Sees files **and** sub-directories (volume labels / LFN records are
/// already skipped by [`DirIter`]).
pub fn find_in_dir(vol: &Volume, start: u32, name11: &[u8; 11]) -> Option<DirEntry> {
    DirIter::new(vol, start).find(|e| &e.name == name11)
}

/// Find an 8.3 **normal-file** entry by its FAT short name — like
/// [`find_in_dir`] but sub-directories are excluded, so a caller can never
/// resolve a folder (or label) as if it were a file. Mirrors EmuTOS'
/// `scan(dn, s, FA_NORM, …)`, whose `FA_NORM` mask omits `FA_SUBDIR`/`FA_VOL`
/// (`bdos/fs.h:195`); used by `Fopen` (`ixopen`) and `Fattrib` (`xchmod`) so
/// both report `EFILNF`/`EPTHNF` for a directory instead of operating on it.
/// Directory traversal (`Fsfirst`, `list_dir`, …) keeps using [`find_in_dir`]
/// / [`DirIter`] and still sees folders.
pub fn find_file_in_dir(vol: &Volume, start: u32, name11: &[u8; 11]) -> Option<DirEntry> {
    DirIter::new(vol, start).find(|e| &e.name == name11 && e.attrib & FA_SUBDIR == 0)
}

/// Map the partition-relative sector of directory slot `slot`.
fn slot_sector(vol: &Volume, start: u32, slot: u32) -> Option<u32> {
    vol.dir_nth_sector(start, slot / ENTRIES_PER_SECTOR as u32)
}

/// Locate an existing entry's raw slot index in directory `start`
/// (0 = root), for in-place updates / erase.
pub fn slot_of(vol: &Volume, start: u32, name11: &[u8; 11]) -> Option<u32> {
    let mut buf = [0u8; SECTOR];
    let mut loaded = u32::MAX;
    let mut slot = 0u32;
    loop {
        let sec_idx = slot / ENTRIES_PER_SECTOR as u32;
        if sec_idx != loaded {
            let rel = slot_sector(vol, start, slot)?;
            if !vol.read_rel_sector(rel, &mut buf) {
                return None;
            }
            loaded = sec_idx;
        }
        let off = (slot as usize % ENTRIES_PER_SECTOR) * ENTRY;
        let e = &buf[off..off + ENTRY];
        match e[0] {
            END => return None,
            ERASED => {}
            _ if e[0x0B] != FA_LFN && e[0x0B] & FA_VOL == 0 && e[0..11] == name11[..] => {
                return Some(slot)
            }
            _ => {}
        }
        slot += 1;
    }
}

/// Find a free slot (erased `0xE5` or end-of-directory `0x00`) in
/// directory `start` (0 = root). `None` if no free slot exists within
/// the directory's current sectors (the caller may grow a sub-directory).
pub fn find_free_slot(vol: &Volume, start: u32) -> Option<u32> {
    let mut buf = [0u8; SECTOR];
    let mut loaded = u32::MAX;
    let mut slot = 0u32;
    loop {
        let sec_idx = slot / ENTRIES_PER_SECTOR as u32;
        if sec_idx != loaded {
            let rel = slot_sector(vol, start, slot)?;
            if !vol.read_rel_sector(rel, &mut buf) {
                return None;
            }
            loaded = sec_idx;
        }
        let b0 = buf[(slot as usize % ENTRIES_PER_SECTOR) * ENTRY];
        if b0 == END || b0 == ERASED {
            return Some(slot);
        }
        slot += 1;
    }
}

/// Find a free slot in directory `start` (0 = root), **growing a non-root
/// directory by one cluster** when it is full. The fixed-size root directory
/// still returns `None` on exhaustion.
///
/// Mirrors the EmuTOS `ixcreat` grow loop (`fsopnclo.c:172-183`): when
/// `scan` finds no free slot it appends a cluster with `nextcl(fd,1)`,
/// zeroes it with `dirinit(dn)` and rescans; only the root (`!fd->o_dnode`)
/// cannot grow and returns `EACCDN`. Used by `Fcreate`, `Dcreate` and the
/// cross-directory `Frename` so a sub-directory can hold more than one
/// cluster's worth of entries.
pub fn find_free_slot_grow(vol: &Volume, start: u32) -> Option<u32> {
    if let Some(slot) = find_free_slot(vol, start) {
        return Some(slot);
    }
    if start == 0 {
        return None; // the root directory has a fixed size and cannot grow
    }
    grow_dir(vol, start)
}

/// Grow a non-root directory by one cluster: allocate a fresh FAT cluster,
/// link it to the end of the directory's cluster chain, zero every sector
/// (so all its entries read as end-of-directory `0x00`), and return the
/// slot index of the first entry in the new cluster. `None` on allocation
/// or write failure. Mirrors EmuTOS `nextcl(fd,1)` + `dirinit(dn)`
/// (`fsopnclo.c:178-181`).
fn grow_dir(vol: &Volume, start: u32) -> Option<u32> {
    // Walk to the last cluster of the chain, counting the clusters so far.
    let mut last = start;
    let mut count = 1u32;
    loop {
        match vol.fat_entry(last) {
            Some(n) if !vol.is_eoc(n) && vol.is_data_cluster(n) => {
                last = n;
                count += 1;
            }
            _ => break,
        }
    }
    // Allocate the new cluster (marked EOC by `alloc_cluster`) and splice
    // it onto the chain: previous last cluster now points at it.
    let new = vol.alloc_cluster()?;
    if !vol.set_fat_entry(last, new) {
        return None;
    }
    // Zero every sector of the new cluster: all entries become the
    // end-of-directory marker (empty slots).
    let zero = [0u8; SECTOR];
    for s in 0..vol.clsiz {
        if !vol.write_cluster_sector(new, s, &zero) {
            return None;
        }
    }
    // The first slot of the new cluster follows every slot that already
    // existed: `count` clusters × `clsiz` sectors × entries-per-sector.
    Some(count * vol.clsiz * ENTRIES_PER_SECTOR as u32)
}

/// Read the raw 32-byte directory entry at `slot` in directory `start`.
pub fn read_slot(vol: &Volume, start: u32, slot: u32, out: &mut [u8; ENTRY]) -> bool {
    let rel = match slot_sector(vol, start, slot) {
        Some(r) => r,
        None => return false,
    };
    let mut buf = [0u8; SECTOR];
    if !vol.read_rel_sector(rel, &mut buf) {
        return false;
    }
    let off = (slot as usize % ENTRIES_PER_SECTOR) * ENTRY;
    out.copy_from_slice(&buf[off..off + ENTRY]);
    true
}

/// Write a raw 32-byte directory entry at `slot` in directory `start`
/// (read-modify-write of the containing sector, through the cache).
pub fn write_slot(vol: &Volume, start: u32, slot: u32, entry: &[u8; ENTRY]) -> bool {
    let rel = match slot_sector(vol, start, slot) {
        Some(r) => r,
        None => return false,
    };
    let mut buf = [0u8; SECTOR];
    if !vol.read_rel_sector(rel, &mut buf) {
        return false;
    }
    let off = (slot as usize % ENTRIES_PER_SECTOR) * ENTRY;
    buf[off..off + ENTRY].copy_from_slice(entry);
    vol.write_rel_sector(rel, &buf)
}

// ── long-file-name slots ──────────────────────────────────────────────────
//
// A VFAT long name is stored in extra 32-byte slots carrying `attrib ==
// FA_LFN` that sit **immediately in front of** the 8.3 entry they name, in
// reverse order: the slot next to the 8.3 entry holds ordinal 1, the one
// before it ordinal 2, and the physically first slot of the chain has bit
// `0x40` set on top of its ordinal. Every slot of a chain repeats a checksum
// over the eleven short-name bytes, so a chain can be told apart from a
// stale one that happens to precede the same slot.
//
// Source: FreeMiNT's FAT layer, `references/freemint/sys/fatfs.c:779-790`
// (the `LDIR` record: `head` = ordinal + `0x40` end-of-name flag at offset 0,
// `attr` at 11, `chksum` at 13) and `:3690-3748` (`__readvfat`: walk the
// ordinals down to 1, every slot must repeat the same checksum, and the
// checksum must match the short entry). FreeMiNT notes the layout is
// "informations ... from Microsoft" (`:813`). EmuTOS does **not** answer this
// question: classic TOS has no long names at all, and EmuTOS only *skips*
// `FA_LFN` records when scanning (`references/emutos/bdos/fsdir.c:326,1810`).

/// Offset of the ordinal byte (`LDIR.head`) in a long-name slot.
const LFN_ORD: usize = 0x00;
/// Offset of the short-name checksum in a long-name slot.
const LFN_CHKSUM: usize = 0x0D;
/// `LDIR.head` bit marking the physically first slot of a chain.
const LFN_LAST: u8 = 0x40;
/// `LDIR.head` bits holding the ordinal (`fatfs.c:781`, "bit 0..4").
const LFN_ORD_MASK: u8 = 0x1F;
/// Upper bound for the backward walk — the ordinal field is five bits wide,
/// so no well-formed chain can be longer. A corrupt chain cannot make the
/// walk run away past it.
const LFN_MAX_SLOTS: u8 = LFN_ORD_MASK;

/// Checksum of an 8.3 name as every long-name slot of that entry repeats it:
/// `sum = ror8(sum) + name[i]` over the eleven short-name bytes
/// (FreeMiNT `__readvfat`, `references/freemint/sys/fatfs.c:3743-3748`).
pub fn lfn_checksum(name11: &[u8; 11]) -> u8 {
    let mut sum = 0u8;
    for &c in name11.iter() {
        sum = sum.rotate_right(1).wrapping_add(c);
    }
    sum
}

/// Free the directory entry at `slot` **together with its long-name slots**.
///
/// Marks the 8.3 entry erased (`0xE5` in the first name byte) and then walks
/// backwards over the `FA_LFN` slots in front of it, marking each of them
/// erased as well. This is FreeMiNT's `unlink_cookie_i`
/// (`references/freemint/sys/fatfs.c:4246-4272`), which erases the short
/// entry first and only then the `c->slots` VFAT slots that precede it —
/// same order here, so an interrupted delete leaves harmless orphans rather
/// than a live entry with a truncated name chain.
///
/// A slot is only taken to belong to this entry when it is a well-formed
/// continuation: not already erased, `attrib == FA_LFN`, its checksum equal
/// to the short name's, and its ordinal the next one expected (1, 2, 3 …).
/// The walk stops at the slot carrying `LFN_LAST`, and refuses to touch a
/// chain that does not match — an already-orphaned or malformed chain in
/// front of the entry is left exactly as it was.
///
/// Returns whether the 8.3 entry itself was freed.
///
/// NewTOS never *writes* long-name slots (GEMDOS names are 8.3), but every
/// card written by a foreign system carries them, and leaving them behind on
/// delete accumulates orphans that make `fsck_msdos` fail (TASK/FS01).
pub fn erase_slot_with_lfn(vol: &Volume, start: u32, slot: u32) -> bool {
    let mut e = [0u8; ENTRY];
    if !read_slot(vol, start, slot, &mut e) {
        return false;
    }
    let mut name11 = [0u8; 11];
    name11.copy_from_slice(&e[0..11]);
    let sum = lfn_checksum(&name11);

    // The 8.3 entry first (FreeMiNT order).
    e[0] = ERASED;
    if !write_slot(vol, start, slot, &e) {
        return false;
    }

    // Then its long-name chain.
    erase_lfn_chain(vol, start, slot, sum);
    true
}

/// Erase the long-name chain in front of directory slot `slot` whose slots
/// carry the short-name checksum `sum` — the chain of the 8.3 name the slot
/// held when `sum` was computed. Walks backwards from `slot - 1`, ordinal 1
/// upwards, with the same well-formedness rules as [`erase_slot_with_lfn`]
/// (live `FA_LFN` slot, matching checksum, expected ordinal, stop at
/// `LFN_LAST`); a chain that does not match is left alone. The slot itself
/// is not touched.
///
/// Used on its own by the same-directory `Frename`, which overwrites the 8.3
/// name in place (EmuTOS `xrename`, `bdos/fsdir.c:1076-1083`): the old chain
/// no longer matches the new name and would otherwise be left as an orphan
/// (TASK/FS18, the FS01 rule).
pub fn erase_lfn_chain(vol: &Volume, start: u32, slot: u32, sum: u8) {
    let mut ord = 1u8;
    let mut k = slot;
    let mut l = [0u8; ENTRY];
    while k > 0 && ord <= LFN_MAX_SLOTS {
        k -= 1;
        if !read_slot(vol, start, k, &mut l) {
            break;
        }
        let head = l[LFN_ORD];
        if head == ERASED || head == END || l[0x0B] != FA_LFN {
            break; // not a live long-name slot
        }
        if l[LFN_CHKSUM] != sum || head & LFN_ORD_MASK != ord {
            break; // belongs to some other (or a broken) name
        }
        l[LFN_ORD] = ERASED;
        if !write_slot(vol, start, k, &l) {
            break;
        }
        if head & LFN_LAST != 0 {
            break; // physically first slot of the chain
        }
        ord += 1;
    }
}

/// Build a 32-byte FCB directory entry (name, attribute, time/date,
/// start cluster, size).
pub fn build_fcb(name11: &[u8; 11], attr: u8, cluster: u32, size: u32, time: u16, date: u16) -> [u8; ENTRY] {
    let mut e = [0u8; ENTRY];
    e[0..11].copy_from_slice(name11);
    e[0x0B] = attr;
    e[0x16..0x18].copy_from_slice(&time.to_le_bytes());
    e[0x18..0x1A].copy_from_slice(&date.to_le_bytes());
    e[0x1A..0x1C].copy_from_slice(&((cluster & 0xFFFF) as u16).to_le_bytes());
    e[0x1C..0x20].copy_from_slice(&size.to_le_bytes());
    e
}
