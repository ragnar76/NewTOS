//! TASK/FS18 — `Frename` and the long-name slots of the renamed entry.
//!
//! The same-directory rename now overwrites the 8.3 name in place
//! (`references/emutos/bdos/fsdir.c:1076-1083`). A VFAT long-name chain in
//! front of that entry carries a checksum over the *old* short name
//! (FreeMiNT `references/freemint/sys/fatfs.c:3743-3748`), so after the
//! rename it would no longer belong to anything — an orphan that makes
//! `fsck_msdos` fail (TASK/FS01). The chain is erased with the old name, as
//! `Fdelete` does (`dir::erase_slot_with_lfn`); a cross-directory move erases
//! the old entry together with its chain the same way. EmuTOS has no long
//! names and only skips such slots (`fsdir.c:326,1810`), so nothing a TOS
//! program can see changes.
//!
//! One test only: the fixture is planted into the raw image before the
//! binary's first GEMDOS call (the BDOS sector cache has no invalidate).

mod fatvol;

use fatvol::*;

const FRENAME: u16 = 0x56;

fn chksum(name11: &[u8; 11]) -> u8 {
    let mut sum: u8 = 0;
    for &c in name11 {
        sum = ((sum & 1) << 7 | (sum & 0xFE) >> 1).wrapping_add(c);
    }
    sum
}

fn lfn_slot(ord: u8, sum: u8) -> [u8; 32] {
    let mut e = [0u8; 32];
    e[0] = ord;
    e[1] = b'x';
    e[0x0B] = 0x0F;
    e[0x0D] = sum;
    e
}

fn short(name11: &[u8; 11]) -> [u8; 32] {
    let mut e = [0u8; 32];
    e[0..11].copy_from_slice(name11);
    e[0x0B] = 0x20;
    e[0x16..0x18].copy_from_slice(&0x8000u16.to_le_bytes());
    e[0x18..0x1A].copy_from_slice(&0x5901u16.to_le_bytes());
    e
}

const LONGNA: &[u8; 11] = b"LONGNA~1TXT";
const MOVEME: &[u8; 11] = b"MOVEME~1TXT";

fn frename(src: &str, dst: &str) -> i32 {
    let s = cpath(src);
    let d = cpath(dst);
    call(FRENAME, [0, s.as_ptr() as u64, d.as_ptr() as u64, 0, 0, 0, 0, 0])
}

fn slot(s: &[u8; SECTOR_BYTES], i: usize) -> &[u8] {
    &s[i * 32..i * 32 + 32]
}

#[test]
fn rename_erases_the_old_long_name_chain() {
    let (_g, host) = fs();
    // Root slots 0/1 are the image's SUB and HELLO.TXT.
    let mut s = host.read_sector(root_lba());
    let mut put = |i: usize, e: [u8; 32]| s[i * 32..i * 32 + 32].copy_from_slice(&e);
    put(2, lfn_slot(0x42, chksum(LONGNA)));
    put(3, lfn_slot(0x01, chksum(LONGNA)));
    put(4, short(LONGNA));
    put(5, lfn_slot(0x41, chksum(MOVEME)));
    put(6, short(MOVEME));
    host.write_sector(root_lba(), &s);

    // Same directory: name overwritten in slot 4, its chain (2, 3) erased.
    assert_eq!(frename("C:\\LONGNA~1.TXT", "C:\\RENAMED.TXT"), E_OK);
    let s = host.read_sector(root_lba());
    assert_eq!(&slot(&s, 4)[0..11], b"RENAMED TXT");
    assert_eq!(slot(&s, 4)[0x0B], 0x20);
    assert_eq!(slot(&s, 2)[0], 0xE5, "LFN slot 2 erased");
    assert_eq!(slot(&s, 3)[0], 0xE5, "LFN slot 3 erased");

    // Across directories: old entry (6) and its chain (5) erased.
    assert_eq!(frename("C:\\MOVEME~1.TXT", "C:\\SUB\\MOVED.TXT"), E_OK);
    let s = host.read_sector(root_lba());
    assert_eq!(slot(&s, 6)[0], 0xE5, "old 8.3 entry erased");
    assert_eq!(slot(&s, 5)[0], 0xE5, "its LFN slot erased");
    // Untouched neighbours.
    assert_eq!(&slot(&s, 0)[0..11], b"SUB        ");
    assert_eq!(&slot(&s, 1)[0..11], b"HELLO   TXT");
    assert_eq!(&slot(&s, 4)[0..11], b"RENAMED TXT");
}
