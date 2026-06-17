//! Phase 08 microSD read self-test (board-rpi4 / QEMU raspi4b).
//!
//! Initialises the EMMC2 microSD card and reads sector 0, checking the
//! MBR / FAT boot-sector signature `0x55AA` at offset 510 (Phase 08 EC1
//! precursor, before the BIOS `Rwabs` layer lands).

use crate::drivers::bcm2711::emmc;
use crate::hal::block::BlockDevice;
use crate::kprintln;

/// Run the microSD read self-test.
pub fn run() {
    kprintln!("[emmc] === microSD (EMMC2) read test ===");
    // SAFETY: boot CPU; the VideoCore firmware has finished with the card.
    if !unsafe { emmc::init() } {
        kprintln!("[emmc] card init failed (no card? attach -drive ...,if=sd)");
        return;
    }
    kprintln!("[emmc] card initialised");

    let mut buf = [0u8; 512];
    match emmc::SDCARD.read_blocks(0, &mut buf) {
        Ok(()) => {
            let sig = u16::from_le_bytes([buf[510], buf[511]]);
            kprintln!(
                "[emmc] sector 0 read OK: bytes {:02x} {:02x} {:02x} {:02x} ..., sig=0x{:04x} {}",
                buf[0],
                buf[1],
                buf[2],
                buf[3],
                sig,
                if sig == 0xAA55 { "(valid 55AA)" } else { "(no 55AA)" },
            );
        }
        Err(e) => kprintln!("[emmc] sector 0 read FAILED: {:?}", e),
    }
}
