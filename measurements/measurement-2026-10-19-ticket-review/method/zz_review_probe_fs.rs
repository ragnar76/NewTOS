//! Ticket review 2026-10-19 — observation probe (NOT a product test).
//! Copied temporarily into tos-api/tests/, run with --nocapture, then removed.
//! Each case prints what NewTOS answers next to what EmuTOS answers by source
//! (cited in docs/analysis/open-tickets-review-2026-10-19.md).

mod fatvol;

use fatvol::*;
use tos_api::bdos::process;
use tos_types::BasePage;

const FRENAME: u16 = 0x56;
const FATTRIB: u16 = 0x43;
const FSEEK: u16 = 0x42;
const FDATIME: u16 = 0x57;

struct Pd(*mut BasePage);
impl Pd {
    fn new() -> Pd {
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
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
fn fclose(h: i32) -> i32 {
    gemdos1(FCLOSE, h as u16 as u64)
}
fn fseek(off: i32, h: i32, mode: u16) -> i32 {
    call(FSEEK, [off as u32 as u64, h as u16 as u64, mode as u64, 0, 0, 0, 0, 0])
}
fn fattrib_get(name: &str) -> i32 {
    let p = cpath(name);
    call(FATTRIB, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0])
}
fn frename(src: &str, dst: &str) -> i32 {
    let s = cpath(src);
    let d = cpath(dst);
    call(FRENAME, [0, s.as_ptr() as u64, d.as_ptr() as u64, 0, 0, 0, 0, 0])
}
fn fdatime(buf: &mut [u16; 2], h: i32, wrt: u16) -> i32 {
    call(FDATIME, [buf.as_mut_ptr() as u64, h as u16 as u64, wrt as u64, 0, 0, 0, 0, 0])
}
fn listing(dir_glob: &str) -> Vec<String> {
    let mut dta = Dta::new();
    gemdos1(FSETDTA, dta.ptr());
    let g = cpath(dir_glob);
    let mut out = Vec::new();
    let mut r = gemdos2(FSFIRST, g.as_ptr() as u64, FA_ANY);
    while r == E_OK {
        out.push(dta.name());
        r = call(FSNEXT, [0; 8]);
    }
    out
}

#[test]
fn fs14_cross_directory_rename_to_empty_name() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    let _ = dcreate("C:\\FS14");
    let h = fcreate("C:\\FS14\\X.TXT", 0);
    assert!(h >= 6, "Fcreate -> {h}");
    fclose(h);
    let r = frename("C:\\FS14\\X.TXT", "C:\\");
    println!("PROBE FS14 Frename(C:\\FS14\\X.TXT, C:\\) -> {r}  (EmuTOS: -34 EPTHNF)");
    println!("PROBE FS14 root listing after: {:?}", listing("C:\\*.*"));
    println!("PROBE FS14 FS14 listing after: {:?}", listing("C:\\FS14\\*.*"));
}

#[test]
fn fs15_device_names_in_fopen() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    for n in ["CON:", "con:", "AUX:", "PRN:", "Con:"] {
        let r = fopen(n, 1);
        println!("PROBE FS15 Fopen({n},1) -> {r} (EmuTOS: CON:/con: 0xFFFF, AUX: 0xFFFE, PRN: 0xFFFD; mixed case -> normal path)");
        let c = fcreate(n, 0);
        println!("PROBE FS15 Fcreate({n},0) -> {c}");
        if r >= 6 {
            fclose(r);
        }
        if c >= 6 {
            fclose(c);
        }
    }
}

#[test]
fn fs17_1_fdelete_folder() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    let _ = dcreate("C:\\FS17D");
    let r = fdelete("C:\\FS17D");
    println!("PROBE FS17.1 Fdelete(folder) -> {r} (EmuTOS: -33 EFILNF, scan FA_NORM)");
}

#[test]
fn fs17_2_two_fopen_share_length() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    let h = fcreate("C:\\FS172.TXT", 0);
    fwrite(h, b"abc");
    fclose(h);
    let h1 = fopen("C:\\FS172.TXT", 2);
    let h2 = fopen("C:\\FS172.TXT", 2);
    fseek(0, h1, 2);
    let w = fwrite(h1, b"defgh");
    let end2 = fseek(0, h2, 2);
    println!("PROBE FS17.2 write 5 via h1 (len 3 -> 8, w={w}); Fseek(0,h2,SEEK_END) -> {end2} (EmuTOS shared DFD: 8)");
    let c1 = fclose(h1);
    let c2 = fclose(h2);
    let h3 = fopen("C:\\FS172.TXT", 0);
    let len = fseek(0, h3, 2);
    fclose(h3);
    println!("PROBE FS17.2 close h1 -> {c1}, close h2 -> {c2}; length on disk after both closes -> {len} (EmuTOS: 8)");
}

#[test]
fn fs17_5_archive_bit_on_close() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    let h = fcreate("C:\\FS175.TXT", 0);
    let a0 = fattrib_get("C:\\FS175.TXT");
    fclose(h);
    let a1 = fattrib_get("C:\\FS175.TXT");
    println!("PROBE FS17.5 Fcreate(attr 0) attr while open={a0}, after Fclose={a1} (EmuTOS: Fcreate sets O_DIRTY -> ixclose attr|=FA_ARCHIVE -> 0x20=32)");
    // clear the archive bit, reopen, extend, close
    let pth = cpath("C:\\FS175.TXT");
    call(FATTRIB, [pth.as_ptr() as u64, 1, 0, 0, 0, 0, 0, 0]);
    let a2 = fattrib_get("C:\\FS175.TXT");
    let h = fopen("C:\\FS175.TXT", 2);
    fwrite(h, b"grow");
    fclose(h);
    let a3 = fattrib_get("C:\\FS175.TXT");
    println!("PROBE FS17.5 after Fattrib(set 0)={a2}; after Fopen+Fwrite(extend)+Fclose={a3} (EmuTOS: 32)");
}

#[test]
fn fs17_6_fcreate_over_existing_reuses_slot() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    let _ = dcreate("C:\\FS176");
    for n in ["X", "Y", "Z"] {
        let h = fcreate(&format!("C:\\FS176\\{n}.TXT"), 0);
        fclose(h);
    }
    fdelete("C:\\FS176\\X.TXT");
    let h = fcreate("C:\\FS176\\Z.TXT", 0);
    fclose(h);
    println!("PROBE FS17.6 order after Fdelete X + Fcreate over Z: {:?} (EmuTOS: ixcreat scans for space from Z's own position -> ., .., Y.TXT, Z.TXT)", listing("C:\\FS176\\*.*"));
}

#[test]
fn fs17_4_fdatime_set_then_close() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    let h = fcreate("C:\\FS174.TXT", 0);
    let mut td = [0x1234u16, 0x4321u16];
    let s = fdatime(&mut td, h, 1);
    fwrite(h, b"data");
    fclose(h);
    let h = fopen("C:\\FS174.TXT", 0);
    let mut back = [0u16; 2];
    fdatime(&mut back, h, 0);
    fclose(h);
    println!("PROBE FS17.4 Fdatime set {:04x}/{:04x} -> {s}, Fwrite, Fclose; read back {:04x}/{:04x} (EmuTOS: o_td set, written on close -> same)", td[0], td[1], back[0], back[1]);
}

#[test]
fn fs17_3_rename_open_file_same_directory() {
    let (_g, _h) = fs();
    let p = Pd::new();
    p.run();
    let _ = dcreate("C:\\FS173");
    for n in ["A", "B"] {
        let h = fcreate(&format!("C:\\FS173\\{n}.TXT"), 0);
        fclose(h);
    }
    // order after a plain same-directory rename (EmuTOS overwrites the name in place, fsdir.c:1076-1083)
    let r0 = frename("C:\\FS173\\A.TXT", "C:\\FS173\\C.TXT");
    println!("PROBE FS17.3 Frename A->C (same dir, closed) -> {r0}; order {:?} (EmuTOS: in place -> ., .., C.TXT, B.TXT)", listing("C:\\FS173\\*.*"));
    // an open, still empty file renamed in the same directory, then written and closed
    let h = fcreate("C:\\FS173\\D.TXT", 0);
    let r1 = frename("C:\\FS173\\D.TXT", "C:\\FS173\\E.TXT");
    let w = fwrite(h, &[b'e'; 1000]);
    let c = fclose(h);
    let h2 = fopen("C:\\FS173\\E.TXT", 0);
    let len = if h2 >= 0 { let l = fseek(0, h2, 2); fclose(h2); l } else { h2 };
    println!("PROBE FS17.3 open D, Frename D->E -> {r1}, Fwrite 1000 -> {w}, Fclose -> {c}; E.TXT length -> {len} (EmuTOS: same slot, ixclose writes o_fileln -> 1000)");
    println!("PROBE FS17.3 listing after: {:?}", listing("C:\\FS173\\*.*"));
}

fn free_clusters(host: &DiskHost) -> usize {
    let mut free = 0;
    let mut cl = 0usize;
    for s in 0..20u64 {
        let sec = host.read_sector(fat_lba(0) + s);
        for e in sec.chunks(2) {
            if (2..5002).contains(&cl) && e == [0, 0] {
                free += 1;
            }
            cl += 1;
        }
    }
    free
}

#[test]
fn fs17_3b_rename_open_file_leaks_clusters() {
    let (_g, host) = fs();
    let p = Pd::new();
    p.run();
    let _ = dcreate("C:\\FS173B");
    let before = free_clusters(host);
    let h = fcreate("C:\\FS173B\\D.TXT", 0);
    frename("C:\\FS173B\\D.TXT", "C:\\FS173B\\E.TXT");
    fwrite(h, &[b'e'; 3000]);
    fclose(h);
    let mid = free_clusters(host);
    let d = fdelete("C:\\FS173B\\E.TXT");
    let after = free_clusters(host);
    println!("PROBE FS17.3b free clusters before={before} after write+close={mid} after Fdelete(E.TXT)={d} -> {after} (leak = before - after = {})", before as i64 - after as i64);
}
