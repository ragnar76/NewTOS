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
}
