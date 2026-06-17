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
    }
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
