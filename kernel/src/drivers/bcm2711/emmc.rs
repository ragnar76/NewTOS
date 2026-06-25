//! BCM2711 EMMC2 (Arasan/iproc SDHCI) microSD driver.
//!
//! Reference: [`docs/hardware/emmc-sd.md`] — the on-board microSD card
//! slot (EMMC2 @ `0xFE34_0000`), the SD Host Controller + SD Physical
//! Layer specs. Bring-up scope: PIO single-block **read** (no DMA, no
//! IRQ). First test vehicle is QEMU `raspi4b`'s `emmc` device.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::hal::block::{BlockDevice, BlockError, SECTOR_SIZE};

/// SD-host controller candidate bases (low-peripheral mode). On real
/// RPi4 the microSD slot is **EMMC2** (`0xFE34_0000`); QEMU `raspi4b`
/// wires its emulated card to the legacy **EMMC** SDHCI (`0xFE30_0000`).
/// Both are standard SDHCI, so one driver serves both — `init()` probes
/// EMMC2 first (real HW), then EMMC (QEMU).
const EMMC2_BASE: usize = crate::drivers::mmio_va(0xFE34_0000);
const EMMC_BASE: usize = crate::drivers::mmio_va(0xFE30_0000);

/// The controller base the driver is currently bound to.
static BASE_ADDR: AtomicUsize = AtomicUsize::new(EMMC2_BASE);

fn base() -> usize {
    BASE_ADDR.load(Ordering::Relaxed)
}

// Register offsets (datasheet §5 grouping; see the Track-B reference).
const BLKSIZECNT: usize = 0x04;
const ARG1: usize = 0x08;
const CMDTM: usize = 0x0C;
const RESP0: usize = 0x10;
const RESP1: usize = 0x14;
const RESP2: usize = 0x18;
const RESP3: usize = 0x1C;
const DATA: usize = 0x20;
const STATUS: usize = 0x24;
const CONTROL0: usize = 0x28;
const CONTROL1: usize = 0x2C;
const INTERRUPT: usize = 0x30;
const IRPT_MASK: usize = 0x34;
const IRPT_EN: usize = 0x38;
const CAPABILITIES: usize = 0x40;

// STATUS (present state) bits.
const SR_CMD_INHIBIT: u32 = 1 << 0;
const SR_DAT_INHIBIT: u32 = 1 << 1;

// CONTROL1 bits.
const C1_CLK_INTLEN: u32 = 1 << 0;
const C1_CLK_STABLE: u32 = 1 << 1;
const C1_CLK_EN: u32 = 1 << 2;
const C1_TOUNIT_MAX: u32 = 0xE << 16;
const C1_SRST_HC: u32 = 1 << 24;

// INTERRUPT bits.
const INT_CMD_DONE: u32 = 1 << 0;
const INT_DATA_DONE: u32 = 1 << 1;
const INT_WRITE_RDY: u32 = 1 << 4;
const INT_READ_RDY: u32 = 1 << 5;
const INT_ERROR_MASK: u32 = 0xFFFF_0000;

// CMDTM command-word flags (upper half).
const RSP_NONE: u32 = 0;
const RSP_136: u32 = 1 << 16;
const RSP_48: u32 = 2 << 16;
const RSP_48BUSY: u32 = 3 << 16;
const CMD_CRCCHK: u32 = 1 << 19;
const CMD_IXCHK: u32 = 1 << 20;
const CMD_ISDATA: u32 = 1 << 21;
const fn cmd(index: u32) -> u32 {
    index << 24
}
// Transfer-mode flags (lower half).
const TM_DAT_CARD2HOST: u32 = 1 << 4;

// SD commands as full CMDTM words.
const GO_IDLE: u32 = cmd(0) | RSP_NONE;
const SEND_IF_COND: u32 = cmd(8) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const ALL_SEND_CID: u32 = cmd(2) | RSP_136 | CMD_CRCCHK;
const SEND_REL_ADDR: u32 = cmd(3) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const SEND_CSD: u32 = cmd(9) | RSP_136 | CMD_CRCCHK;
const SELECT_CARD: u32 = cmd(7) | RSP_48BUSY | CMD_CRCCHK | CMD_IXCHK;
const SET_BLOCKLEN: u32 = cmd(16) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const READ_SINGLE: u32 =
    cmd(17) | RSP_48 | CMD_CRCCHK | CMD_IXCHK | CMD_ISDATA | TM_DAT_CARD2HOST;
const WRITE_SINGLE: u32 = cmd(24) | RSP_48 | CMD_CRCCHK | CMD_IXCHK | CMD_ISDATA;
const APP_CMD: u32 = cmd(55) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const SD_SEND_OP_COND: u32 = cmd(41) | RSP_48; // ACMD41, R3 (no CRC/index check)

/// Busy-wait iteration budget for register polling.
const SPIN_LIMIT: u32 = 1_000_000;

#[inline(always)]
fn rd(off: usize) -> u32 {
    // SAFETY: MMIO read at a valid SD-host register address.
    unsafe { core::ptr::read_volatile((base() + off) as *const u32) }
}
#[inline(always)]
fn wr(off: usize, val: u32) {
    // SAFETY: MMIO write at a valid SD-host register address.
    unsafe { core::ptr::write_volatile((base() + off) as *mut u32, val) }
}

/// Extract CSD field `[hi:lo]` from the 4-word response. SDHCI stores a
/// 136-bit response right-shifted by 8 bits (the CRC byte is dropped),
/// so CSD bit `n` is at response bit `n-8`.
fn csd_bits(r: &[u32; 4], hi: u32, lo: u32) -> u32 {
    let mut v = 0u32;
    for b in lo..=hi {
        let rb = b - 8;
        v |= ((r[(rb / 32) as usize] >> (rb % 32)) & 1) << (b - lo);
    }
    v
}

/// Compute the card's 512-byte block count from its CSD (v1 SDSC or
/// v2 SDHC/SDXC).
fn parse_capacity(r: &[u32; 4]) -> u64 {
    if csd_bits(r, 127, 126) == 1 {
        // v2: capacity = (C_SIZE + 1) × 512 KiB.
        (csd_bits(r, 69, 48) as u64 + 1) * 1024
    } else {
        // v1: (C_SIZE+1) × 2^(C_SIZE_MULT+2) blocks × 2^READ_BL_LEN bytes.
        let read_bl_len = csd_bits(r, 83, 80);
        let c_size = csd_bits(r, 73, 62) as u64;
        let c_size_mult = csd_bits(r, 49, 47);
        let blocknr = (c_size + 1) * (1u64 << (c_size_mult + 2));
        blocknr * (1u64 << read_bl_len) / SECTOR_SIZE as u64
    }
}

/// The microSD device on EMMC2.
pub struct Emmc {
    rca: AtomicU32,
    sdhc: AtomicBool,
    ready: AtomicBool,
    blocks: AtomicU64,
}

/// Board-selected block device instance.
pub static SDCARD: Emmc = Emmc {
    rca: AtomicU32::new(0),
    sdhc: AtomicBool::new(false),
    ready: AtomicBool::new(false),
    blocks: AtomicU64::new(0),
};

impl Emmc {
    /// Poll until `(rd(off) & mask) == 0`, or time out.
    fn wait_clear(off: usize, mask: u32) -> bool {
        for _ in 0..SPIN_LIMIT {
            if rd(off) & mask == 0 {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    /// Wait for `flag` in INTERRUPT (clearing it), or an error / timeout.
    fn wait_interrupt(flag: u32) -> Result<(), BlockError> {
        for _ in 0..SPIN_LIMIT {
            let i = rd(INTERRUPT);
            if i & INT_ERROR_MASK != 0 {
                wr(INTERRUPT, i); // clear (W1C)
                return Err(BlockError::Io);
            }
            if i & flag != 0 {
                wr(INTERRUPT, flag); // clear the bit we consumed
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(BlockError::Timeout)
    }

    /// Issue a command and return `RESP0`.
    fn command(&self, cmdtm: u32, arg: u32) -> Result<u32, BlockError> {
        if !Self::wait_clear(STATUS, SR_CMD_INHIBIT) {
            return Err(BlockError::Timeout);
        }
        wr(INTERRUPT, rd(INTERRUPT)); // clear any leftover status
        wr(ARG1, arg);
        wr(CMDTM, cmdtm);
        Self::wait_interrupt(INT_CMD_DONE)?;
        Ok(rd(RESP0))
    }

    /// An application command (`CMD55` then the ACMD).
    fn app_command(&self, cmdtm: u32, arg: u32) -> Result<u32, BlockError> {
        let rca = self.rca.load(Ordering::Relaxed) << 16;
        self.command(APP_CMD, rca)?;
        self.command(cmdtm, arg)
    }

    /// Reset the controller and bring the clock up to `target_hz`.
    fn reset_and_clock(target_hz: u32) -> Result<(), BlockError> {
        wr(CONTROL0, 0);
        wr(CONTROL1, C1_SRST_HC);
        for _ in 0..SPIN_LIMIT {
            if rd(CONTROL1) & C1_SRST_HC == 0 {
                break;
            }
            core::hint::spin_loop();
        }
        // Power the SD bus: CONTROL0 byte 1 = Power Control —
        // SD_BUS_POWER (bit 8) + voltage select 3.3 V (0b111 at bits 11:9).
        wr(CONTROL0, 0x0F00);
        Self::set_clock(target_hz)?;
        wr(IRPT_EN, 0); // poll, don't signal
        wr(IRPT_MASK, 0xFFFF_FFFF); // but record all status bits
        Ok(())
    }

    /// Program the SD clock divider for ~`target_hz`.
    fn set_clock(target_hz: u32) -> Result<(), BlockError> {
        // Base clock from CAPABILITIES[15:8] (MHz); fall back to 100 MHz.
        let base_mhz = (rd(CAPABILITIES) >> 8) & 0xFF;
        let base_hz = if base_mhz == 0 { 100_000_000 } else { base_mhz * 1_000_000 };
        // SDCLK = base / (2 * div); pick the smallest div that fits.
        let mut div = 1u32;
        while base_hz / (2 * div) > target_hz && div < 0x3FF {
            div += 1;
        }
        let freq8 = (div & 0xFF) << 8;
        let freq_ms2 = ((div >> 8) & 0x3) << 6;
        wr(CONTROL1, C1_CLK_INTLEN | freq8 | freq_ms2 | C1_TOUNIT_MAX);
        for _ in 0..SPIN_LIMIT {
            if rd(CONTROL1) & C1_CLK_STABLE != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        wr(CONTROL1, rd(CONTROL1) | C1_CLK_EN);
        Ok(())
    }

    /// Full card init: reset, clock, then the SD command sequence.
    fn init_card(&self) -> Result<(), BlockError> {
        Self::reset_and_clock(400_000)?; // identification speed

        self.command(GO_IDLE, 0)?;

        // CMD8: voltage check. Success ⇒ SD v2 (high-capacity capable).
        let v2 = self.command(SEND_IF_COND, 0x1AA).is_ok();

        // ACMD41: wait for power-up; HCS set when v2.
        let hcs = if v2 { 0x4000_0000 } else { 0 };
        let mut ocr = 0;
        for _ in 0..SPIN_LIMIT {
            ocr = self.app_command(SD_SEND_OP_COND, hcs | 0x00FF_8000)?;
            if ocr & 0x8000_0000 != 0 {
                break; // power-up complete
            }
            core::hint::spin_loop();
        }
        if ocr & 0x8000_0000 == 0 {
            return Err(BlockError::Timeout);
        }
        self.sdhc.store(ocr & 0x4000_0000 != 0, Ordering::Relaxed);

        self.command(ALL_SEND_CID, 0)?;
        let rca = self.command(SEND_REL_ADDR, 0)? >> 16;
        self.rca.store(rca, Ordering::Relaxed);

        // CMD9 (SEND_CSD) is valid in stand-by state — read it before
        // selecting the card to learn the capacity.
        self.command(SEND_CSD, rca << 16)?;
        let csd = [rd(RESP0), rd(RESP1), rd(RESP2), rd(RESP3)];
        self.blocks.store(parse_capacity(&csd), Ordering::Relaxed);

        self.command(SELECT_CARD, rca << 16)?;

        if !self.sdhc.load(Ordering::Relaxed) {
            self.command(SET_BLOCKLEN, SECTOR_SIZE as u32)?;
        }

        // Switch to default speed (~25 MHz) for transfers.
        Self::set_clock(25_000_000)?;
        self.ready.store(true, Ordering::Release);
        Ok(())
    }

    /// Read one 512-byte block at `lba` into `buf` (PIO).
    fn read_one(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        if !self.ready.load(Ordering::Acquire) {
            return Err(BlockError::NotReady);
        }
        if !Self::wait_clear(STATUS, SR_DAT_INHIBIT) {
            return Err(BlockError::Timeout);
        }
        // SDHC addresses by block; SDSC by byte.
        let addr = if self.sdhc.load(Ordering::Relaxed) {
            lba as u32
        } else {
            (lba as u32) * SECTOR_SIZE as u32
        };
        wr(BLKSIZECNT, (1 << 16) | SECTOR_SIZE as u32);
        let resp0 = self.command(READ_SINGLE, addr)?;
        let _ = resp0;
        Self::wait_interrupt(INT_READ_RDY)?;
        for word in buf.chunks_exact_mut(4) {
            let w = rd(DATA);
            word.copy_from_slice(&w.to_le_bytes());
        }
        Self::wait_interrupt(INT_DATA_DONE)?;
        Ok(())
    }

    /// Write one 512-byte block at `lba` from `buf` (PIO).
    fn write_one(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {
        if !self.ready.load(Ordering::Acquire) {
            return Err(BlockError::NotReady);
        }
        if !Self::wait_clear(STATUS, SR_DAT_INHIBIT) {
            return Err(BlockError::Timeout);
        }
        let addr = if self.sdhc.load(Ordering::Relaxed) {
            lba as u32
        } else {
            (lba as u32) * SECTOR_SIZE as u32
        };
        wr(BLKSIZECNT, (1 << 16) | SECTOR_SIZE as u32);
        self.command(WRITE_SINGLE, addr)?;
        Self::wait_interrupt(INT_WRITE_RDY)?;
        for word in buf.chunks_exact(4) {
            wr(DATA, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }
        Self::wait_interrupt(INT_DATA_DONE)?;
        Ok(())
    }
}

impl BlockDevice for Emmc {
    fn block_count(&self) -> u64 {
        self.blocks.load(Ordering::Relaxed)
    }

    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        if !buf.len().is_multiple_of(SECTOR_SIZE) {
            return Err(BlockError::Unsupported);
        }
        for (i, sector) in buf.chunks_exact_mut(SECTOR_SIZE).enumerate() {
            self.read_one(lba + i as u64, sector)?;
        }
        Ok(())
    }

    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {
        if !buf.len().is_multiple_of(SECTOR_SIZE) {
            return Err(BlockError::Unsupported);
        }
        for (i, sector) in buf.chunks_exact(SECTOR_SIZE).enumerate() {
            self.write_one(lba + i as u64, sector)?;
        }
        Ok(())
    }
}

/// Initialise the microSD card. Probes EMMC2 (real RPi4 SD slot) then
/// EMMC (QEMU raspi4b), using whichever has a responding card. Returns
/// `false` if neither has a card (the caller then has no block device —
/// never a stubbed success).
///
/// # Safety
/// Boot CPU, single-threaded; the VideoCore firmware has finished using
/// the card.
pub unsafe fn init() -> bool {
    // Idempotent: once a card is up, re-initialising would needlessly
    // re-run the SD power-up sequence. Callers may invoke this from both
    // boot bring-up and the self-test.
    if SDCARD.ready.load(Ordering::Acquire) {
        return true;
    }
    for &candidate in &[EMMC2_BASE, EMMC_BASE] {
        BASE_ADDR.store(candidate, Ordering::Relaxed);
        // Skip a base with no SDHCI controller behind it.
        let ver = rd(0xFC);
        if ver == 0 || ver == 0xFFFF_FFFF {
            continue;
        }
        if SDCARD.init_card().is_ok() {
            crate::kprintln!("[emmc] SD card on controller @ 0x{:08x}", candidate);
            return true;
        }
    }
    false
}
