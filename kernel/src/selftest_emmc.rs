//! Phase 08 microSD read/write self-test (board-rpi4 / QEMU raspi4b).
//!
//! Initialises the EMMC2 microSD card, reads sector 0 (checking the
//! MBR/FAT boot-sector signature `0x55AA`), reports the capacity from
//! the CSD, and does a **non-destructive** write + read-back on the last
//! sector (original saved and restored).

use crate::drivers::bcm2711::emmc;
use crate::hal::block::BlockDevice;
use crate::kprintln;

/// Run the microSD read/write self-test.
pub fn run() {
    kprintln!("[emmc] === microSD (SD-host) read/write test ===");
    // SAFETY: boot CPU; the VideoCore firmware has finished with the card.
    if !unsafe { emmc::init() } {
        kprintln!("[emmc] card init failed (no card? attach -drive ...,if=sd)");
        return;
    }
    let blocks = emmc::SDCARD.block_count();
    kprintln!(
        "[emmc] card initialised: {} blocks ({} MiB)",
        blocks,
        blocks * 512 / (1024 * 1024),
    );

    // ── read sector 0 ──
    let mut buf = [0u8; 512];
    match emmc::SDCARD.read_blocks(0, &mut buf) {
        Ok(()) => {
            let sig = u16::from_le_bytes([buf[510], buf[511]]);
            kprintln!(
                "[emmc] sector 0: {:02x} {:02x} {:02x} {:02x} ..., sig=0x{:04x} {}",
                buf[0],
                buf[1],
                buf[2],
                buf[3],
                sig,
                if sig == 0xAA55 { "(valid 55AA)" } else { "(no 55AA)" },
            );
        }
        Err(e) => {
            kprintln!("[emmc] sector 0 read FAILED: {:?}", e);
            return;
        }
    }

    // ── non-destructive write + read-back on the last sector ──
    if blocks > 1 {
        let scratch = blocks - 1;
        let mut orig = [0u8; 512];
        let mut test = [0u8; 512];
        let mut back = [0u8; 512];
        for (i, b) in test.iter_mut().enumerate() {
            *b = (i as u8) ^ 0xA5;
        }
        test[..9].copy_from_slice(b"NEWTOS-WR");

        let result = emmc::SDCARD
            .read_blocks(scratch, &mut orig)
            .and_then(|_| emmc::SDCARD.write_blocks(scratch, &test))
            .and_then(|_| emmc::SDCARD.read_blocks(scratch, &mut back))
            .map(|_| back == test);
        // Restore the original contents regardless of outcome.
        let _ = emmc::SDCARD.write_blocks(scratch, &orig);

        match result {
            Ok(true) => kprintln!("[emmc] write+read-back @ sector {} OK (restored)", scratch),
            Ok(false) => kprintln!("[emmc] write+read-back @ sector {} MISMATCH", scratch),
            Err(e) => kprintln!("[emmc] write test @ sector {} FAILED: {:?}", scratch, e),
        }
    }

    // ── partition table + BIOS Rwabs over the real trap path ──
    let n = tos_api::bios::mount();
    let map = drvmap();
    kprintln!("[emmc] mounted {} partition(s); Drvmap=0x{:x}", n, map);

    if map != 0 {
        let drv = map.trailing_zeros() as u64; // first present drive
        let letter = (b'A' + drv as u8) as char;
        let mut rbuf = [0u8; 512];
        // Read sector 0 of the partition (its FAT/boot sector).
        let rc = rwabs(0, rbuf.as_mut_ptr() as u64, 1, 0, drv, 0);
        let sig = u16::from_le_bytes([rbuf[510], rbuf[511]]);
        kprintln!(
            "[emmc] Rwabs(read sec 0 of {}:) -> {} ; first {:02x} {:02x}, sig=0x{:04x} {}",
            letter,
            rc,
            rbuf[0],
            rbuf[1],
            sig,
            if rc == 0 { "OK" } else { "FAIL" },
        );

        // ── Getbpb on the FAT partition (C:) and the bare one (D:) ──
        let bp = getbpb(drv);
        if bp != 0 {
            // The BPB is nine consecutive UWORDs at the returned pointer.
            let p = bp as *const u16;
            let f = |i: usize| unsafe { p.add(i).read() };
            kprintln!(
                "[emmc] Getbpb({}:) recsiz={} clsiz={} clsizb={} rdlen={} fsiz={} fatrec={} datrec={} numcl={} flags=0x{:x}",
                letter, f(0), f(1), f(2), f(3), f(4), f(5), f(6), f(7), f(8),
            );
        } else {
            kprintln!("[emmc] Getbpb({}:) -> NULL", letter);
        }
        let bd = getbpb(drv + 1);
        kprintln!(
            "[emmc] Getbpb({}:) -> {}",
            (b'A' + (drv + 1) as u8) as char,
            if bd == 0 { "NULL (no FAT BPB)" } else { "ptr" },
        );

        // ── Phase 09 D1: FAT cluster-chain reader on C: ──
        fat_test(drv as u16, letter);
    }
}

/// Phase 09 D1: mount the FAT volume on `drive` and walk the cluster
/// chain of the first directory entry, verifying the chain and the data.
fn fat_test(drive: u16, letter: char) {
    use tos_api::bdos::fs::fat::Volume;

    let vol = match Volume::mount(drive) {
        Some(v) => v,
        None => {
            kprintln!("[emmc] FAT mount({}:) -> not a FAT volume", letter);
            return;
        }
    };
    kprintln!(
        "[emmc] FAT mount({}:) clsizb={} numcl={} root@{}(rel)",
        letter,
        vol.clsizb,
        vol.numcl,
        vol.root_sector(),
    );

    // Read the first root-directory sector and take the first 8.3 entry.
    let mut sec = [0u8; 512];
    if !read_part_sector(drive, vol.root_sector(), &mut sec) {
        kprintln!("[emmc] FAT root read failed");
        return;
    }
    if sec[0] == 0x00 || sec[0] == 0xE5 {
        kprintln!("[emmc] FAT root dir empty");
        return;
    }
    let mut name = [0u8; 11];
    name.copy_from_slice(&sec[0..11]);
    let start = u16::from_le_bytes([sec[0x1A], sec[0x1B]]) as u32;
    let size = u32::from_le_bytes([sec[0x1C], sec[0x1D], sec[0x1E], sec[0x1F]]);
    kprintln!(
        "[emmc] dir[0]=\"{}\" cluster={} size={}",
        core::str::from_utf8(&name).unwrap_or("?"),
        start,
        size,
    );

    // Walk the FAT chain and read the first cluster's content.
    let mut cl = start;
    let mut chain = 0u32;
    let mut first = [0u8; 64];
    while vol.is_data_cluster(cl) && chain < 64 {
        if chain == 0 {
            let mut buf = [0u8; 2048];
            if vol.read_cluster(cl, &mut buf) {
                first.copy_from_slice(&buf[..64]);
            }
        }
        chain += 1;
        match vol.fat_entry(cl) {
            Some(next) if vol.is_eoc(next) => break,
            Some(next) => cl = next,
            None => break,
        }
    }
    let want = size.div_ceil(vol.clsizb).max(1);
    kprintln!(
        "[emmc] chain length={} (expected {} clusters) {}",
        chain,
        want,
        if chain == want { "OK" } else { "MISMATCH" },
    );
    kprintln!(
        "[emmc] cluster[0] first bytes: \"{}\"",
        core::str::from_utf8(&first[..32]).unwrap_or("?"),
    );

    // FAT type + next-cluster sanity (genuine FAT16: fat_entry(2) == 3).
    kprintln!("[emmc] fat_entry(2) -> {:?} (expect Some(3))", vol.fat_entry(2));

    // ── directory traversal (D5 foundation): list root dir + lookup ──
    use tos_api::bdos::fs::dir;
    let mut count = 0u32;
    for e in dir::RootDir::new(&vol) {
        kprintln!(
            "[emmc]   entry: \"{}\" attr=0x{:02x} cl={} size={}",
            core::str::from_utf8(&e.name).unwrap_or("?"),
            e.attrib,
            e.cluster,
            e.size,
        );
        count += 1;
    }
    let hit = dir::fcb_name("hello.txt")
        .and_then(|n| dir::find_in_root(&vol, &n))
        .map(|e| e.cluster);
    kprintln!(
        "[emmc] root entries={}; find(\"hello.txt\") -> {}",
        count,
        hit.unwrap_or_default(),
    );

    // ── Phase 09 D4: GEMDOS Fopen/Fread/Fseek/Fclose (EC1) ──
    gemdos_file_test();
}

/// Phase 09 D4 / EC1: open `c:/hello.txt` through GEMDOS, read it across
/// a cluster boundary, seek, and close — all over the real `svc #1` path.
fn gemdos_file_test() {
    let path = b"c:/hello.txt\0";
    let h = gemdos_fopen(path.as_ptr() as u64, 0);
    kprintln!("[emmc] Fopen(\"c:/hello.txt\") -> {}", h);
    if h < 6 {
        kprintln!("[emmc] Fopen FAILED");
        return;
    }
    let handle = h as u16;

    // Read 64 bytes from the start.
    let mut buf = [0u8; 64];
    let n = gemdos_fread(handle, buf.len() as i64, buf.as_mut_ptr() as u64);
    kprintln!(
        "[emmc] Fread(64) -> {} : \"{}\"",
        n,
        core::str::from_utf8(&buf[..32]).unwrap_or("?"),
    );

    // Seek to size-via-END, then back to an offset spanning cluster 2->3
    // (cluster boundary is at clsizb = 512 bytes).
    let size = gemdos_fseek(0, handle, 2);
    kprintln!("[emmc] Fseek(0,END) -> size={} (expect 1000)", size);
    let pos = gemdos_fseek(508, handle, 0); // 4 bytes before cluster boundary
    let mut b2 = [0u8; 32];
    let n2 = gemdos_fread(handle, b2.len() as i64, b2.as_mut_ptr() as u64);
    kprintln!(
        "[emmc] Fseek(508,SET)->{} ; Fread(32) -> {} across cluster 2->3: \"{}\"",
        pos,
        n2,
        core::str::from_utf8(&b2[..n2.max(0) as usize]).unwrap_or("?"),
    );

    // Read at EOF returns 0.
    gemdos_fseek(0, handle, 2);
    let eof = gemdos_fread(handle, 16, buf.as_mut_ptr() as u64);
    kprintln!("[emmc] Fread at EOF -> {} (expect 0)", eof);

    let rc = gemdos_fclose(handle);
    kprintln!("[emmc] Fclose -> {} {}", rc, if rc == 0 { "OK" } else { "FAIL" });
}

/// GEMDOS `Fopen` (0x3D) via `svc #1`: x0=name, x1=mode.
fn gemdos_fopen(name: u64, mode: u64) -> i64 {
    let r: i64;
    // SAFETY: svc #1, GEMDOS Fopen ABI; `name` is a kernel string pointer.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x3Du64, inout("x0") name => r, in("x1") mode, options(nostack));
    }
    r
}

/// GEMDOS `Fread` (0x3F) via `svc #1`: x0=handle, x1=len, x2=buf.
fn gemdos_fread(handle: u16, len: i64, buf: u64) -> i64 {
    let r: i64;
    // SAFETY: svc #1, GEMDOS Fread ABI; `buf` is a kernel buffer.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x3Fu64, inout("x0") handle as u64 => r, in("x1") len, in("x2") buf, options(nostack));
    }
    r
}

/// GEMDOS `Fseek` (0x42) via `svc #1`: x0=offset, x1=handle, x2=flag.
fn gemdos_fseek(offset: i64, handle: u16, flag: u16) -> i64 {
    let r: i64;
    // SAFETY: svc #1, GEMDOS Fseek ABI.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x42u64, inout("x0") offset => r, in("x1") handle as u64, in("x2") flag as u64, options(nostack));
    }
    r
}

/// GEMDOS `Fclose` (0x3E) via `svc #1`: x0=handle.
fn gemdos_fclose(handle: u16) -> i64 {
    let r: i64;
    // SAFETY: svc #1, GEMDOS Fclose ABI.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x3Eu64, inout("x0") handle as u64 => r, options(nostack));
    }
    r
}

/// Read one partition-relative sector via `Rwabs` (read).
fn read_part_sector(drive: u16, rel: u32, buf: &mut [u8; 512]) -> bool {
    rwabs(0, buf.as_mut_ptr() as u64, 1, rel as u64, drive as u64, 0) == 0
}

/// BIOS `Getbpb` via `svc #13` (function id 0x07) — returns a `BPB *`.
#[inline(always)]
fn getbpb(drive: u64) -> i64 {
    let r: i64;
    // SAFETY: svc #13, BIOS Getbpb (func 0x07), drive in x0.
    unsafe {
        core::arch::asm!("svc #13", in("x8") 0x07u64, inout("x0") drive => r, options(nostack));
    }
    r
}

/// BIOS `Drvmap` via `svc #13` (function id 0x0A).
#[inline(always)]
fn drvmap() -> u32 {
    let r: i64;
    // SAFETY: svc #13, BIOS Drvmap (func 0x0A), no arguments.
    unsafe {
        core::arch::asm!("svc #13", in("x8") 0x0Au64, out("x0") r, options(nostack));
    }
    r as u32
}

/// BIOS `Rwabs` via `svc #13` (function id 4 in x8).
#[inline(always)]
fn rwabs(r_w: u64, adr: u64, numb: u64, first: u64, drive: u64, lfirst: u64) -> i64 {
    let r: i64;
    // SAFETY: svc #13, BIOS Rwabs ABI (func 4); kernel buffer in `adr`.
    unsafe {
        core::arch::asm!(
            "svc #13",
            in("x8") 4u64,
            inout("x0") r_w => r,
            in("x1") adr,
            in("x2") numb,
            in("x3") first,
            in("x4") drive,
            in("x5") lfirst,
            options(nostack),
        );
    }
    r
}
