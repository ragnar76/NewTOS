//! TASK/FS18 + TASK/FS14 — `Frename` as EmuTOS `xrename` does it
//! (`references/emutos/bdos/fsdir.c:895-1101`).
//!
//! * **Same directory** (`strtcl1 == strtcl2`): only the eleven name bytes of
//!   the existing FCB are overwritten, in place (`fsdir.c:1076-1083`). The
//!   entry keeps its slot, so the directory order stays, and an open file's
//!   OFD — which `ixclose` writes back through `o_dirfil`/`o_dirbyt`
//!   (`bdos/fsopnclo.c:530-549`) — still names the live entry. NewTOS moved
//!   the entry to a free slot and erased the old one, so the close of a file
//!   renamed while open wrote its start cluster and length into the erased
//!   slot: length 0 and leaked clusters (FS17 point 3,
//!   `docs/analysis/open-tickets-review-2026-10-19.md`).
//! * **Across directories**: a new entry is made with `ixcreat(p2,att)`
//!   (`fsdir.c:1014`); *any* `ixcreat` failure — an empty name or one that
//!   starts with a dot (`fsopnclo.c:117-118`), a full root (`:174-176`) — is
//!   answered with `EPTHNF` (`fsdir.c:1017-1018`, TASK/FS14). The new entry
//!   is closed through `xclose` → `ixclose(fd,0)` with `O_DIRTY`, which ORs
//!   `FA_ARCHIVE` into a moved *file's* attribute (`fsopnclo.c:541-545`);
//!   time/date, start cluster and length are the old ones (`fsdir.c:1029-1035`).
//! * **Error order**: the target-exists check comes first (`fsdir.c:912-913`),
//!   then the source (`:915-944`), then the target directory (`:975-976`).
//!
//! NewTOS deviation for an *open* file moved across directories
//! (`docs/COMPATIBILITY.md` "Sanctioned deviations", FS18): EmuTOS leaves
//! the OFD on the erased old slot, so a later write + close loses the data
//! the same way; NewTOS moves the OFD with the entry.

mod fatvol;

use fatvol::*;
use tos_api::bdos::process;
use tos_types::BasePage;

const FRENAME: u16 = 0x56;
const FATTRIB: u16 = 0x43;
const FSEEK: u16 = 0x42;
const EACCDN: i32 = -36;

/// A process basepage in host memory, made `run` with [`Pd::run`].
struct Pd(*mut BasePage);
impl Pd {
    fn new() -> Pd {
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        // SAFETY: fresh box.
        unsafe { (*bp).p_uft = [-1, -1, -2, -3, -1, -1] };
        Pd(bp)
    }
    fn run(&self) {
        process::set_current(self.0);
    }
}
impl Drop for Pd {
    fn drop(&mut self) {
        if process::current() == self.0 {
            process::set_current(core::ptr::null_mut());
        }
        // SAFETY: nothing names the box any more.
        drop(unsafe { Box::from_raw(self.0 as *mut [u8; 256]) });
    }
}

fn fcreate(name: &str, attr: u64) -> i32 {
    let p = cpath(name);
    gemdos2(FCREATE, p.as_ptr() as u64, attr)
}
fn fopen(name: &str, mode: u64) -> i32 {
    let p = cpath(name);
    gemdos2(FOPEN, p.as_ptr() as u64, mode)
}
fn fdelete(name: &str) -> i32 {
    let p = cpath(name);
    gemdos1(FDELETE, p.as_ptr() as u64)
}
fn dcreate(name: &str) -> i32 {
    let p = cpath(name);
    gemdos1(DCREATE, p.as_ptr() as u64)
}
fn fwrite(h: i32, data: &[u8]) -> i32 {
    call(FWRITE, [h as u16 as u64, data.len() as u64, data.as_ptr() as u64, 0, 0, 0, 0, 0])
}
fn fread(h: i32, buf: &mut [u8]) -> i32 {
    call(FREAD, [h as u16 as u64, buf.len() as u64, buf.as_mut_ptr() as u64, 0, 0, 0, 0, 0])
}
fn fclose(h: i32) -> i32 {
    gemdos1(FCLOSE, h as u16 as u64)
}
fn fseek(off: i32, h: i32, mode: u16) -> i32 {
    call(FSEEK, [off as u32 as u64, h as u16 as u64, mode as u64, 0, 0, 0, 0, 0])
}
fn fattrib(name: &str, wrt: u64, attr: u64) -> i32 {
    let p = cpath(name);
    call(FATTRIB, [p.as_ptr() as u64, wrt, attr, 0, 0, 0, 0, 0])
}
fn frename(src: &str, dst: &str) -> i32 {
    let s = cpath(src);
    let d = cpath(dst);
    call(FRENAME, [0, s.as_ptr() as u64, d.as_ptr() as u64, 0, 0, 0, 0, 0])
}

/// `(name, attr, time, date, length)` of every entry `Fsfirst`/`Fsnext` list.
fn listing(glob: &str) -> Vec<(String, u8, u16, u16, u32)> {
    let mut dta = Dta::new();
    gemdos1(FSETDTA, dta.ptr());
    let g = cpath(glob);
    let mut out = Vec::new();
    // FA_ANY | FA_SUBDIR: files, hidden/system and folders.
    let mut r = gemdos2(FSFIRST, g.as_ptr() as u64, FA_ANY);
    while r == E_OK {
        // SAFETY: the DTA is 44 bytes; d_attrib 21, d_time 22, d_date 24, d_length 26.
        let raw = unsafe { core::slice::from_raw_parts(dta.ptr() as *const u8, 44) };
        let w = |o: usize| u16::from_le_bytes([raw[o], raw[o + 1]]);
        let len = u32::from_le_bytes([raw[26], raw[27], raw[28], raw[29]]);
        out.push((dta.name(), raw[21], w(22), w(24), len));
        r = call(FSNEXT, [0; 8]);
    }
    out
}

fn names(glob: &str) -> Vec<String> {
    listing(glob).into_iter().map(|e| e.0).collect()
}

/// A file's contents through `Fopen` + `Fread` (`None` if it cannot be opened).
fn contents(name: &str) -> Option<Vec<u8>> {
    let h = fopen(name, 0);
    if h < 0 {
        return None;
    }
    let len = fseek(0, h, 2);
    fseek(0, h, 0);
    let mut buf = vec![0u8; len.max(0) as usize];
    let n = fread(h, &mut buf);
    fclose(h);
    assert_eq!(n, len, "Fread of {name}");
    Some(buf)
}

fn pattern(n: usize, seed: u8) -> Vec<u8> {
    (0..n).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

// ── a tiny fsck over the raw image ────────────────────────────────────────

const ROOT_SLOTS: u32 = 512;
const FIRST_CL: u32 = 2;
const END_CL: u32 = 5002; // clusters 2..5001
const SECTOR_CL: usize = 512; // one sector per cluster in the fixture

fn fat(host: &DiskHost) -> Vec<u16> {
    let mut v = Vec::new();
    for s in 0..20u64 {
        let sec = host.read_sector(fat_lba(0) + s);
        for e in sec.chunks(2) {
            v.push(u16::from_le_bytes([e[0], e[1]]));
        }
    }
    v
}

fn chain(fat: &[u16], start: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut c = start;
    while (FIRST_CL..END_CL).contains(&c) && out.len() < END_CL as usize {
        out.push(c);
        c = u32::from(fat[c as usize]);
    }
    out
}

/// Every problem `fsck_msdos` would report for this FAT16 fixture: a file
/// whose chain length does not match its size, a cluster reached twice, an
/// allocated cluster no entry reaches (lost), a long-name slot whose 8.3
/// entry is gone.
fn fsck(host: &DiskHost) -> Vec<String> {
    let fat = fat(host);
    let mut used = vec![false; END_CL as usize];
    let mut problems = Vec::new();
    // (path, directory start cluster: 0 = root)
    let mut dirs: Vec<(String, u32)> = vec![("C:".into(), 0)];
    while let Some((path, start)) = dirs.pop() {
        let mut slots: Vec<[u8; 32]> = Vec::new();
        let sectors: Vec<u64> = if start == 0 {
            (0..ROOT_SLOTS as u64 / 16).map(|s| root_lba() + s).collect()
        } else {
            chain(&fat, start).into_iter().map(cluster_lba).collect()
        };
        'load: for lba in sectors {
            let s = host.read_sector(lba);
            for i in 0..16 {
                let mut e = [0u8; 32];
                e.copy_from_slice(&s[i * 32..i * 32 + 32]);
                if e[0] == 0 {
                    break 'load;
                }
                slots.push(e);
            }
        }
        for (i, e) in slots.iter().enumerate() {
            if e[0] == 0xE5 {
                continue;
            }
            let name = String::from_utf8_lossy(&e[0..11]).into_owned();
            if e[0x0B] == 0x0F {
                let mut j = i + 1;
                while j < slots.len() && slots[j][0x0B] == 0x0F && slots[j][0] != 0xE5 {
                    j += 1;
                }
                if j >= slots.len() || slots[j][0] == 0xE5 || slots[j][0x0B] == 0x0F {
                    problems.push(format!("{path}: orphan long-name slot {i}"));
                }
                continue;
            }
            if e[0x0B] & 0x08 != 0 || e[0] == b'.' {
                continue;
            }
            let cl = u32::from(u16::from_le_bytes([e[0x1A], e[0x1B]]));
            let size = u32::from_le_bytes([e[0x1C], e[0x1D], e[0x1E], e[0x1F]]);
            let ch = chain(&fat, cl);
            for &c in &ch {
                if used[c as usize] {
                    problems.push(format!("{path}\\{name}: cluster {c} cross-linked"));
                }
                used[c as usize] = true;
            }
            if e[0x0B] & 0x10 != 0 {
                dirs.push((format!("{path}\\{}", name.trim_end()), cl));
            } else {
                let need = (size as usize).div_ceil(SECTOR_CL);
                if need != ch.len() {
                    problems.push(format!("{path}\\{name}: size {size} but {} clusters", ch.len()));
                }
            }
        }
    }
    let lost = (FIRST_CL..END_CL).filter(|&c| fat[c as usize] != 0 && !used[c as usize]).count();
    if lost != 0 {
        problems.push(format!("{lost} lost clusters"));
    }
    problems
}

// ── FS18: same directory ──────────────────────────────────────────────────

/// The FS18 case: an open file renamed in its own directory, then written
/// and closed, keeps every byte, and the volume stays consistent.
#[test]
fn same_dir_rename_of_open_file_keeps_its_data() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS18A"), E_OK);
    let h = fcreate("C:\\FS18A\\D.TXT", 0);
    assert!(h >= 6, "Fcreate -> {h}");
    assert_eq!(frename("C:\\FS18A\\D.TXT", "C:\\FS18A\\E.TXT"), E_OK);
    let data = pattern(3000, 7);
    assert_eq!(fwrite(h, &data), 3000);
    assert_eq!(fclose(h), E_OK);
    assert_eq!(names("C:\\FS18A\\*.*"), [".", "..", "E.TXT"]);
    assert_eq!(contents("C:\\FS18A\\E.TXT").as_deref(), Some(&data[..]));
    assert_eq!(fsck(host), Vec::<String>::new());
    assert_eq!(fdelete("C:\\FS18A\\E.TXT"), E_OK);
    assert_eq!(fsck(host), Vec::<String>::new());
}

/// A file that already holds data, renamed while open and then extended.
#[test]
fn same_dir_rename_of_open_file_then_extend() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS18X"), E_OK);
    let h = fcreate("C:\\FS18X\\OLD.TXT", 0);
    let first = pattern(700, 1);
    assert_eq!(fwrite(h, &first), 700);
    assert_eq!(fclose(h), E_OK);
    let h = fopen("C:\\FS18X\\OLD.TXT", 2);
    assert!(h >= 6, "Fopen -> {h}");
    assert_eq!(frename("C:\\FS18X\\OLD.TXT", "C:\\FS18X\\NEW.TXT"), E_OK);
    assert_eq!(fseek(0, h, 2), 700);
    let more = pattern(1500, 9);
    assert_eq!(fwrite(h, &more), 1500);
    assert_eq!(fclose(h), E_OK);
    let mut want = first.clone();
    want.extend_from_slice(&more);
    assert_eq!(contents("C:\\FS18X\\NEW.TXT").as_deref(), Some(&want[..]));
    assert_eq!(contents("C:\\FS18X\\OLD.TXT"), None);
    assert_eq!(fsck(host), Vec::<String>::new());
}

/// EmuTOS overwrites the name in place: A, B, then A -> C lists C, B.
#[test]
fn same_dir_rename_keeps_directory_order() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS18O"), E_OK);
    for n in ["A", "B"] {
        let h = fcreate(&format!("C:\\FS18O\\{n}.TXT"), 0);
        assert_eq!(fclose(h), E_OK);
    }
    assert_eq!(frename("C:\\FS18O\\A.TXT", "C:\\FS18O\\C.TXT"), E_OK);
    assert_eq!(names("C:\\FS18O\\*.*"), [".", "..", "C.TXT", "B.TXT"]);
}

/// The same-directory rename touches only the name: attribute (no archive
/// bit added), time/date, cluster and length stay.
#[test]
fn same_dir_rename_keeps_attribute_and_stamp() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS18S"), E_OK);
    let h = fcreate("C:\\FS18S\\A.TXT", 0);
    assert_eq!(fwrite(h, b"stamp"), 5);
    assert_eq!(fclose(h), E_OK);
    assert_eq!(fattrib("C:\\FS18S\\A.TXT", 1, 0x02), 0x02); // hidden, no archive
    let before = listing("C:\\FS18S\\A.TXT");
    assert_eq!(frename("C:\\FS18S\\A.TXT", "C:\\FS18S\\B.TXT"), E_OK);
    let after = listing("C:\\FS18S\\B.TXT");
    assert_eq!(before.len(), 1);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].1, 0x02, "attribute");
    assert_eq!((after[0].2, after[0].3, after[0].4), (before[0].2, before[0].3, before[0].4));
}

// ── FS18: across directories ─────────────────────────────────────────────

/// An open file moved to another directory, then written and closed: the
/// data lands in the moved entry (NewTOS deviation, see the module docs), the
/// old directory holds nothing, the volume stays consistent.
#[test]
fn cross_dir_rename_of_open_file_follows_the_entry() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS18B"), E_OK);
    assert_eq!(dcreate("C:\\FS18C"), E_OK);
    let h = fcreate("C:\\FS18B\\D.TXT", 0);
    assert!(h >= 6, "Fcreate -> {h}");
    assert_eq!(frename("C:\\FS18B\\D.TXT", "C:\\FS18C\\E.TXT"), E_OK);
    let data = pattern(3000, 3);
    assert_eq!(fwrite(h, &data), 3000);
    assert_eq!(fclose(h), E_OK);
    assert_eq!(names("C:\\FS18B\\*.*"), [".", ".."]);
    assert_eq!(names("C:\\FS18C\\*.*"), [".", "..", "E.TXT"]);
    assert_eq!(contents("C:\\FS18C\\E.TXT").as_deref(), Some(&data[..]));
    assert_eq!(fsck(host), Vec::<String>::new());
    assert_eq!(fdelete("C:\\FS18C\\E.TXT"), E_OK);
    assert_eq!(fsck(host), Vec::<String>::new());
}

/// A moved file keeps time/date, cluster and length and gains `FA_ARCHIVE`
/// (`xclose` → `ixclose(fd,0)` with `O_DIRTY`, `fsopnclo.c:541-545`).
#[test]
fn cross_dir_rename_sets_archive_and_keeps_stamp() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS18D"), E_OK);
    assert_eq!(dcreate("C:\\FS18E"), E_OK);
    let h = fcreate("C:\\FS18D\\M.TXT", 0);
    assert_eq!(fwrite(h, b"moved"), 5);
    assert_eq!(fclose(h), E_OK);
    assert_eq!(fattrib("C:\\FS18D\\M.TXT", 1, 0x02), 0x02);
    let before = listing("C:\\FS18D\\M.TXT");
    assert_eq!(frename("C:\\FS18D\\M.TXT", "C:\\FS18E\\N.TXT"), E_OK);
    let after = listing("C:\\FS18E\\N.TXT");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].1, 0x22, "FA_HIDDEN | FA_ARCHIVE");
    assert_eq!((after[0].2, after[0].3, after[0].4), (before[0].2, before[0].3, before[0].4));
    assert_eq!(contents("C:\\FS18E\\N.TXT").as_deref(), Some(&b"moved"[..]));
    assert_eq!(fsck(host), Vec::<String>::new());
}

/// A moved folder keeps its attribute (no archive bit: `ixclose(…,CL_DIR)`,
/// then the parent's attribute byte is set to `att`, `fsdir.c:1059-1064`).
#[test]
fn cross_dir_folder_move_keeps_folder_attribute() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS18F"), E_OK);
    assert_eq!(dcreate("C:\\FS18F\\INNER"), E_OK);
    assert_eq!(frename("C:\\FS18F\\INNER", "C:\\INNER18"), E_OK);
    let after = listing("C:\\INNER18");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].1, 0x10);
    assert_eq!(names("C:\\FS18F\\*.*"), [".", ".."]);
    assert_eq!(fsck(host), Vec::<String>::new());
}

// ── FS14: ixcreat failures across directories are EPTHNF ─────────────────

#[test]
fn cross_dir_rename_to_empty_name_is_epthnf() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS14"), E_OK);
    let h = fcreate("C:\\FS14\\X.TXT", 0);
    assert_eq!(fclose(h), E_OK);
    let root_before = names("C:\\*.*");
    assert_eq!(frename("C:\\FS14\\X.TXT", "C:\\"), EPTHNF);
    assert_eq!(names("C:\\*.*"), root_before, "no blank entry in the root");
    assert_eq!(names("C:\\FS14\\*.*"), [".", "..", "X.TXT"]);
    assert_eq!(fsck(host), Vec::<String>::new());
}

#[test]
fn cross_dir_rename_to_dot_name_is_epthnf() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS14D"), E_OK);
    let h = fcreate("C:\\FS14D\\Y.TXT", 0);
    assert_eq!(fclose(h), E_OK);
    let root_before = names("C:\\*.*");
    assert_eq!(frename("C:\\FS14D\\Y.TXT", "C:\\.Y"), EPTHNF);
    assert_eq!(names("C:\\*.*"), root_before);
    assert_eq!(names("C:\\FS14D\\*.*"), [".", "..", "Y.TXT"]);
}

/// A full root cannot grow: `ixcreat` answers `EACCDN`, `xrename` `EPTHNF`.
#[test]
fn cross_dir_rename_into_full_root_is_epthnf() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    assert_eq!(dcreate("C:\\FS14F"), E_OK);
    let h = fcreate("C:\\FS14F\\Z.TXT", 0);
    assert_eq!(fclose(h), E_OK);
    let mut made = Vec::new();
    for i in 0..ROOT_SLOTS {
        let n = format!("C:\\F{i:05}.R");
        let h = fcreate(&n, 0);
        if h < 0 {
            break;
        }
        assert_eq!(fclose(h), E_OK);
        made.push(n);
    }
    assert!(!made.is_empty());
    let r = frename("C:\\FS14F\\Z.TXT", "C:\\Z.TXT");
    for n in &made {
        assert_eq!(fdelete(n), E_OK);
    }
    assert_eq!(r, EPTHNF);
    assert_eq!(names("C:\\FS14F\\*.*"), [".", "..", "Z.TXT"]);
    assert_eq!(fsck(host), Vec::<String>::new());
}

// ── error order (`fsdir.c:912-976`) ──────────────────────────────────────

#[test]
fn target_exists_is_checked_before_the_source() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    // Source directory missing, target exists: EmuTOS checks the target first.
    assert_eq!(frename("C:\\NODIR18\\X.TXT", "C:\\HELLO.TXT"), EACCDN);
    // Source file missing, target exists.
    assert_eq!(frename("C:\\NOFILE18.TXT", "C:\\HELLO.TXT"), EACCDN);
}

#[test]
fn source_is_checked_before_the_target_directory() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    // Source missing, target directory missing: the source answers first.
    assert_eq!(frename("C:\\NOFILE18.TXT", "C:\\NODIR18\\Y.TXT"), EFILNF);
    // Source present, target directory missing.
    assert_eq!(frename("C:\\HELLO.TXT", "C:\\NODIR18\\Y.TXT"), EPTHNF);
    assert_eq!(contents("C:\\HELLO.TXT").as_deref(), Some(&b"hello"[..]));
}
