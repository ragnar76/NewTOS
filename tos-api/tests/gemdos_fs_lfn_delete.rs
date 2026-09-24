//! `Fdelete` / `Ddelete` free the long-name slots of the entry they erase
//! (TASK/FS01).
//!
//! **Where this comes from.** `measurement-2026-09-23-desk24-desk25-desk26`
//! emptied a whole card from the desktop and then ran `fsck_msdos -n` on the
//! result: on the two images whose card had been emptied completely, and only
//! on those, `fsck_msdos` died with exit 133 instead of reporting a clean
//! filesystem (`serial/fsck.txt`). What was left behind were 35 directory
//! slots with `attrib == 0x0F` whose 8.3 entry had been erased — the long
//! names macOS had written for `.fseventsd`, `.Spotlight-V100` and every
//! `._*` companion. Marking exactly those slots `0xE5` in a copy of the same
//! image made it pass phases 1/2/3 with `rc=0`
//! (`method/lfn_orphan_probe.py --fix`).
//!
//! **What the entry's long-name slots are.** They sit immediately in front of
//! the 8.3 entry, in reverse order: ordinal 1 next to it, then 2, 3 …, and the
//! physically first slot of the chain carries `0x40` on top of its ordinal.
//! Every slot repeats a checksum over the eleven short-name bytes. Source:
//! FreeMiNT's FAT layer, `references/freemint/sys/fatfs.c:779-790` (the `LDIR`
//! record) and `:3690-3748` (`__readvfat`, which validates ordinal sequence
//! and checksum before accepting a chain).
//!
//! **What deleting is supposed to do.** FreeMiNT `unlink_cookie_i`
//! (`references/freemint/sys/fatfs.c:4246-4272`) marks the short entry `0xE5`
//! and then marks each of the `c->slots` VFAT slots in front of it `0xE5` as
//! well. EmuTOS cannot answer this question — classic TOS has no long names,
//! and EmuTOS only *skips* `FA_LFN` records when scanning
//! (`references/emutos/bdos/fsdir.c:326,1810`).
//!
//! NewTOS erased only the 8.3 entry, which is what accumulated the orphans.

mod fatvol;

use fatvol::*;

// ── fixture ───────────────────────────────────────────────────────────────

/// Checksum of an 8.3 name, computed independently of the implementation
/// under test: `sum = ror8(sum) + name[i]` over the eleven name bytes.
fn chksum(name11: &[u8; 11]) -> u8 {
    let mut sum: u8 = 0;
    for &c in name11 {
        sum = ((sum & 1) << 7 | (sum & 0xFE) >> 1).wrapping_add(c);
    }
    sum
}

/// One long-name slot: ordinal (`| 0x40` on the chain's first slot), the
/// short name's checksum, and up to 13 UCS-2 characters of the name.
fn lfn_slot(ord: u8, sum: u8, text: &str) -> [u8; 32] {
    let mut e = [0u8; 32];
    e[0] = ord;
    e[0x0B] = 0x0F; // FA_LFN
    e[0x0D] = sum;
    // Character positions inside the slot (`LDIR.name0_4`, `name5_10`,
    // `name11_12`), two bytes each.
    const POS: [usize; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
    let mut chars = text.chars();
    let mut filled = false;
    for &p in POS.iter() {
        match chars.next() {
            Some(c) => e[p..p + 2].copy_from_slice(&(c as u16).to_le_bytes()),
            None if !filled => {
                filled = true; // the terminating NUL, then 0xFFFF padding
            }
            None => e[p..p + 2].copy_from_slice(&0xFFFFu16.to_le_bytes()),
        }
    }
    e
}

/// One 8.3 entry.
fn short(name11: &[u8; 11], attr: u8, cluster: u16) -> [u8; 32] {
    let mut e = [0u8; 32];
    e[0..11].copy_from_slice(name11);
    e[0x0B] = attr;
    e[0x16..0x18].copy_from_slice(&0x8000u16.to_le_bytes());
    e[0x18..0x1A].copy_from_slice(&0x5901u16.to_le_bytes());
    e[0x1A..0x1C].copy_from_slice(&cluster.to_le_bytes());
    e
}

const FILE: u8 = 0x20;
const FOLDER: u8 = 0x10;

const LONGNA: &[u8; 11] = b"LONGNA~1TXT";
const LONGFO: &[u8; 11] = b"LONGFO~1   ";
const KEEPME: &[u8; 11] = b"KEEPME~1TXT";
const BADSUM: &[u8; 11] = b"BADSUM~1TXT";
const SUBLON: &[u8; 11] = b"SUBLON~1TXT";

/// Root-directory slot numbers of the planted fixture. Slots 0 and 1 are the
/// image's own `SUB` and `HELLO.TXT`.
mod root {
    pub const LONGNA_LFN2: usize = 2;
    pub const LONGNA_LFN1: usize = 3;
    pub const LONGNA: usize = 4;
    pub const LONGFO_LFN3: usize = 5;
    pub const LONGFO_LFN2: usize = 6;
    pub const LONGFO_LFN1: usize = 7;
    pub const LONGFO: usize = 8;
    pub const KEEPME_LFN1: usize = 9;
    pub const KEEPME: usize = 10;
    pub const BADSUM_LFN1: usize = 11;
    pub const BADSUM: usize = 12;
}

/// Slot numbers inside `C:\SUB` (cluster 2). Slots 0..2 are `.`, `..` and
/// `INNER.TXT`.
mod sub {
    pub const SUBLON_LFN1: usize = 3;
    pub const SUBLON: usize = 4;
}

/// The cluster the planted folder occupies.
const FOLDER_CLUSTER: u32 = 6;

/// Put the fixture into the raw image. Must run before the binary's first
/// GEMDOS call (the BDOS sector cache has no invalidate).
fn plant(host: &DiskHost) {
    // ── root directory ──
    let mut s = host.read_sector(root_lba());
    let put = |s: &mut [u8; SECTOR_BYTES], slot: usize, e: [u8; 32]| {
        s[slot * 32..slot * 32 + 32].copy_from_slice(&e);
    };
    let sum = chksum(LONGNA);
    put(&mut s, root::LONGNA_LFN2, lfn_slot(0x42, sum, "name.txt"));
    put(&mut s, root::LONGNA_LFN1, lfn_slot(0x01, sum, "a very long f"));
    put(&mut s, root::LONGNA, short(LONGNA, FILE, 0));

    let sum = chksum(LONGFO);
    put(&mut s, root::LONGFO_LFN3, lfn_slot(0x43, sum, "er"));
    put(&mut s, root::LONGFO_LFN2, lfn_slot(0x02, sum, "ng named fold"));
    put(&mut s, root::LONGFO_LFN1, lfn_slot(0x01, sum, "a very very l"));
    put(&mut s, root::LONGFO, short(LONGFO, FOLDER, FOLDER_CLUSTER as u16));

    let sum = chksum(KEEPME);
    put(&mut s, root::KEEPME_LFN1, lfn_slot(0x41, sum, "keep me.txt"));
    put(&mut s, root::KEEPME, short(KEEPME, FILE, 0));

    // A chain whose checksum matches no adjacent short name: already broken
    // before we touch it, and not ours to guess about. FreeMiNT's
    // `__readvfat` rejects it the same way (`fatfs.c:3716-3721`), so the
    // cookie it builds carries `slots == 0` and the unlink erases only the
    // 8.3 entry.
    put(
        &mut s,
        root::BADSUM_LFN1,
        lfn_slot(0x41, chksum(LONGNA) ^ 0xFF, "not mine.txt"),
    );
    put(&mut s, root::BADSUM, short(BADSUM, FILE, 0));
    host.write_sector(root_lba(), &s);

    // ── C:\SUB (cluster 2) ──
    let mut s = host.read_sector(cluster_lba(2));
    let sum = chksum(SUBLON);
    put(&mut s, sub::SUBLON_LFN1, lfn_slot(0x41, sum, "in a folder"));
    put(&mut s, sub::SUBLON, short(SUBLON, FILE, 0));
    host.write_sector(cluster_lba(2), &s);

    // ── the folder's own cluster: "." and ".." only ──
    let mut s = [0u8; SECTOR_BYTES];
    s[0..32].copy_from_slice(&short(b".          ", FOLDER, FOLDER_CLUSTER as u16));
    s[32..64].copy_from_slice(&short(b"..         ", FOLDER, 0));
    host.write_sector(cluster_lba(FOLDER_CLUSTER), &s);

    // ── and its FAT entry, in both copies ──
    for copy in 0..FAT_COPIES {
        let lba = fat_lba(copy);
        let mut f = host.read_sector(lba);
        let o = FOLDER_CLUSTER as usize * 2;
        f[o..o + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        host.write_sector(lba, &f);
    }
}

// ── readback ──────────────────────────────────────────────────────────────

/// First name byte of a directory slot, straight from the image.
fn head(sector: &[u8; SECTOR_BYTES], slot: usize) -> u8 {
    sector[slot * 32]
}

/// Attribute byte of a directory slot, straight from the image.
fn attr(sector: &[u8; SECTOR_BYTES], slot: usize) -> u8 {
    sector[slot * 32 + 0x0B]
}

/// Every slot of the root directory that still holds a live `attrib == 0x0F`
/// record whose 8.3 entry is gone — the orphan count `lfn_orphan_probe.py`
/// prints, and what `fsck_msdos` trips over.
fn orphan_lfn_slots(host: &DiskHost) -> Vec<usize> {
    let mut out = Vec::new();
    let mut slots: Vec<[u8; 32]> = Vec::new();
    'load: for sec in 0..32u32 {
        let s = host.read_sector(root_lba() + sec as u64);
        for i in 0..16 {
            let mut e = [0u8; 32];
            e.copy_from_slice(&s[i * 32..i * 32 + 32]);
            if e[0] == 0x00 {
                break 'load;
            }
            slots.push(e);
        }
    }
    for (i, e) in slots.iter().enumerate() {
        if e[0] == 0xE5 || e[0x0B] != 0x0F {
            continue;
        }
        // The chain belongs to the next record that is not a long-name slot.
        let mut j = i + 1;
        while j < slots.len() && slots[j][0x0B] == 0x0F {
            j += 1;
        }
        if j >= slots.len() || slots[j][0] == 0xE5 {
            out.push(i);
        }
    }
    out
}

// ── the test ──────────────────────────────────────────────────────────────

/// One test function on purpose: the fixture is planted behind the BDOS
/// sector cache, which has no invalidate, so it has to go in before the
/// binary's first GEMDOS call.
#[test]
fn deleting_an_entry_frees_its_long_name_slots() {
    let (_g, host) = fs();
    plant(host);

    // ── 1. Fdelete on a file with a two-slot long name ────────────────────
    let p = cpath("C:\\LONGNA~1.TXT");
    assert_eq!(gemdos1(FDELETE, p.as_ptr() as u64), E_OK);

    let s = host.read_sector(root_lba());
    assert_eq!(head(&s, root::LONGNA), 0xE5, "the 8.3 entry must be freed");
    assert_eq!(
        head(&s, root::LONGNA_LFN1),
        0xE5,
        "ordinal-1 long-name slot must be freed with it"
    );
    assert_eq!(
        head(&s, root::LONGNA_LFN2),
        0xE5,
        "the chain's first slot (0x40) must be freed with it"
    );
    // Only the first byte changes — FreeMiNT rewrites nothing else
    // (`fatfs.c:4266-4268`), and the 0x0F attribute is what keeps a scanner
    // from reading the slot as an 8.3 entry.
    assert_eq!(attr(&s, root::LONGNA_LFN1), 0x0F);

    // ── 2. the neighbouring live file keeps its chain ─────────────────────
    assert_eq!(head(&s, root::KEEPME), b'K', "KEEPME~1.TXT still lives");
    assert_eq!(
        head(&s, root::KEEPME_LFN1),
        0x41,
        "a live entry's long name must not be touched"
    );

    // ── 3. Ddelete on a folder with a three-slot long name ────────────────
    let p = cpath("C:\\LONGFO~1");
    assert_eq!(gemdos1(DDELETE, p.as_ptr() as u64), E_OK);

    let s = host.read_sector(root_lba());
    assert_eq!(head(&s, root::LONGFO), 0xE5, "the folder entry is freed");
    for slot in [root::LONGFO_LFN1, root::LONGFO_LFN2, root::LONGFO_LFN3] {
        assert_eq!(head(&s, slot), 0xE5, "long-name slot {slot} must be freed");
    }

    // ── 4. the same inside a sub-directory ────────────────────────────────
    let p = cpath("C:\\SUB\\SUBLON~1.TXT");
    assert_eq!(gemdos1(FDELETE, p.as_ptr() as u64), E_OK);

    let s = host.read_sector(cluster_lba(2));
    assert_eq!(head(&s, sub::SUBLON), 0xE5);
    assert_eq!(
        head(&s, sub::SUBLON_LFN1),
        0xE5,
        "a sub-directory's long-name slots are freed the same way"
    );

    // ── 5. a chain that does not belong is left alone ─────────────────────
    let p = cpath("C:\\BADSUM~1.TXT");
    assert_eq!(gemdos1(FDELETE, p.as_ptr() as u64), E_OK);

    let s = host.read_sector(root_lba());
    assert_eq!(head(&s, root::BADSUM), 0xE5);
    assert_eq!(
        head(&s, root::BADSUM_LFN1),
        0x41,
        "a chain whose checksum matches no short name is not ours to erase"
    );

    // ── 6. what the measurement counted ───────────────────────────────────
    // After deleting four of the five planted entries the directory holds
    // exactly one orphan long-name slot: the deliberately mismatched one from
    // step 5. Before the fix the same run left six: `[2, 3, 5, 6, 7, 11]`.
    assert_eq!(
        orphan_lfn_slots(host),
        vec![root::BADSUM_LFN1],
        "every freed entry took its long-name slots with it"
    );
}
