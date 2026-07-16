//! QEMU `ramfb` framebuffer driver (configured through fw_cfg).
//!
//! The `virt` machine has no built-in display; `-device ramfb` adds a
//! simple linear framebuffer whose parameters the guest publishes via
//! the fw_cfg DMA interface. We allocate a contiguous XRGB8888 buffer
//! from the frame allocator, point ramfb at its physical address, and
//! then draw straight into it (through the high-half mapping).
//!
//! This is the *development vehicle*: the real RPi4 framebuffer comes
//! from the VideoCore mailbox, not ramfb. Verified headless with QMP
//! `screendump`.
//!
//! ## fw_cfg interface (ARM `virt`, MMIO @ 0x0902_0000)
//!
//! Registers: data @0x00, selector @0x08 (BE), DMA @0x10 (BE). The DMA
//! register takes the **physical address** of a [`DmaAccess`] control
//! block (big-endian fields); QEMU performs the transfer synchronously.
//! We use it to (1) read the file directory and locate `etc/ramfb`,
//! then (2) write the [`RamFbCfg`] (also big-endian) to that file.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::arch::aarch64::mmu::{va_to_pa, VA_OFFSET};
use crate::bios::machine::frames::{alloc_frames_contig, Pool, FRAME_SIZE};
use crate::hal::framebuffer::Framebuffer;

const FWCFG_BASE: usize = crate::drivers::mmio_va(0x0902_0000);
const FWCFG_DMA: usize = FWCFG_BASE + 0x10;

// fw_cfg DMA control bits.
const CTL_READ: u32 = 0x02;
const CTL_SELECT: u32 = 0x08;
const CTL_WRITE: u32 = 0x10;
/// Well-known fw_cfg selector for the file directory.
const FILE_DIR: u32 = 0x0019;
/// DRM_FORMAT_XRGB8888 fourcc ('XR24').
const FOURCC_XRGB8888: u32 = 0x3432_5258;
/// On-the-wire size of the ramfb config item (not `size_of::<RamFbCfg>()`,
/// which is padded to 32 by the leading `u64`).
const RAMFB_CFG_WIRE: u32 = 28;

/// fw_cfg DMA control block (all fields big-endian on the wire).
#[repr(C, align(8))]
struct DmaAccess {
    control: u32,
    length: u32,
    address: u64,
}

/// ramfb configuration record (all fields big-endian).
#[repr(C)]
struct RamFbCfg {
    addr: u64,
    fourcc: u32,
    flags: u32,
    width: u32,
    height: u32,
    stride: u32,
}

// Static scratch in BSS so we can hand QEMU stable physical addresses.
static mut DMA: DmaAccess = DmaAccess { control: 0, length: 0, address: 0 };
static mut DIRBUF: [u8; 4096] = [0; 4096];
static mut CFG: RamFbCfg = RamFbCfg {
    addr: 0,
    fourcc: 0,
    flags: 0,
    width: 0,
    height: 0,
    stride: 0,
};

/// Display geometry, read from the shared [`crate::drivers::BOOT_WIDTH`]/
/// [`BOOT_HEIGHT`]. Asymmetry vs. the RPi4 driver (ADR-017 rev 2): the RPi4
/// VideoCore path *auto-detects* the firmware/config.txt geometry and uses
/// these only as a fallback, whereas ramfb has no firmware display to query —
/// the guest defines the geometry — so it always uses the const default. We
/// publish this surface to ramfb via fw_cfg, so a headless `screendump`
/// captures it at the same size.
const WIDTH: u32 = crate::drivers::BOOT_WIDTH;
const HEIGHT: u32 = crate::drivers::BOOT_HEIGHT;

/// Issue one fw_cfg DMA op via the static [`DMA`] block.
///
/// # Safety
///
/// Single-CPU boot context; `buf_pa` must be a valid physical address
/// for `len` bytes.
// `&mut *(&raw mut X)` is the idiomatic scoped access to a `static
// mut` (the bare `&mut X` form is the harder `static_mut_refs` error);
// clippy's deref_addrof is a false positive against it, as noted in
// `arch::aarch64::mmu`.
#[allow(clippy::deref_addrof)]
unsafe fn fwcfg_dma(control: u32, len: u32, buf_pa: u64) -> u32 {
    // SAFETY: BSS static, single-CPU boot; we fill it then hand its
    // physical address to QEMU.
    unsafe {
        let dma = &mut *(&raw mut DMA);
        dma.control = control.to_be();
        dma.length = len.to_be();
        dma.address = buf_pa.to_be();
        let dma_pa = va_to_pa((&raw const DMA) as u64);
        // Writing the 64-bit DMA register (big-endian) triggers the
        // transfer synchronously.
        core::ptr::write_volatile(FWCFG_DMA as *mut u64, dma_pa.to_be());
        // QEMU clears control to 0 on success (or sets bit0 = error).
        u32::from_be((&raw const DMA).read().control)
    }
}

/// The installed framebuffer.
pub struct RamFb {
    pixels: AtomicU64, // high-half VA of the pixel store
    width: AtomicU32,
    height: AtomicU32,
}

/// Board-selected framebuffer instance.
pub static FRAMEBUFFER: RamFb = RamFb {
    pixels: AtomicU64::new(0),
    width: AtomicU32::new(0),
    height: AtomicU32::new(0),
};

impl Framebuffer for RamFb {
    fn width(&self) -> u32 {
        self.width.load(Ordering::Acquire)
    }
    fn height(&self) -> u32 {
        self.height.load(Ordering::Acquire)
    }
    fn stride_px(&self) -> u32 {
        self.width.load(Ordering::Acquire)
    }
    fn pixels(&self) -> *mut u32 {
        self.pixels.load(Ordering::Acquire) as *mut u32
    }
}

/// Bring up the ramfb framebuffer: allocate the pixel store, locate
/// `etc/ramfb` in the fw_cfg directory, and publish the configuration.
/// Returns `false` if fw_cfg has no ramfb (e.g. `-device ramfb` was not
/// passed) or the allocation failed — the caller then skips graphics.
///
/// # Safety
///
/// Boot CPU, single-threaded, after the frame allocator is up.
#[allow(clippy::deref_addrof)] // see `fwcfg_dma`.
pub unsafe fn init() -> bool {
    // 1. Allocate a contiguous XRGB8888 buffer.
    let bytes = (WIDTH * HEIGHT * 4) as u64;
    let frames = bytes.div_ceil(FRAME_SIZE) as usize;
    let fb = match alloc_frames_contig(Pool::Ttram, frames) {
        Some(f) => f,
        None => return false,
    };
    let fb_pa = fb.pa();
    let fb_va = fb_pa + VA_OFFSET; // write through the high half

    // 2. Read the fw_cfg file directory and find "etc/ramfb".
    // SAFETY: single-CPU boot; DIRBUF is a BSS static.
    let ramfb_key = unsafe {
        let dir_pa = va_to_pa((&raw const DIRBUF) as u64);
        let _ = fwcfg_dma((FILE_DIR << 16) | CTL_SELECT | CTL_READ, 4096, dir_pa);
        match find_file((&raw const DIRBUF) as *const u8, b"etc/ramfb") {
            Some(k) => k,
            None => return false,
        }
    };

    // 3. Publish the ramfb config.
    // SAFETY: single-CPU boot; CFG is a BSS static.
    unsafe {
        let cfg = &mut *(&raw mut CFG);
        cfg.addr = fb_pa.to_be();
        cfg.fourcc = FOURCC_XRGB8888.to_be();
        cfg.flags = 0u32.to_be();
        cfg.width = WIDTH.to_be();
        cfg.height = HEIGHT.to_be();
        cfg.stride = (WIDTH * 4).to_be();
        let cfg_pa = va_to_pa((&raw const CFG) as u64);
        // The ramfb item is exactly 28 bytes on the wire (8+4+4+4+4+4);
        // `size_of::<RamFbCfg>()` is 32 because the leading `u64` rounds
        // the struct up to 8-byte alignment, and a 32-byte write
        // overruns the item → fw_cfg error. fw_cfg sets bit0 of the
        // control word on failure; bail out (no graphics) if so.
        let wr = fwcfg_dma((ramfb_key << 16) | CTL_SELECT | CTL_WRITE, RAMFB_CFG_WIRE, cfg_pa);
        if wr & 0x01 != 0 {
            return false;
        }
    }

    FRAMEBUFFER.pixels.store(fb_va, Ordering::Release);
    FRAMEBUFFER.width.store(WIDTH, Ordering::Release);
    FRAMEBUFFER.height.store(HEIGHT, Ordering::Release);
    true
}

/// Parse a fw_cfg file directory and return the selector key for
/// `name`. Layout: BE `u32` count, then `count` × { BE `u32` size,
/// BE `u16` select, `u16` reserved, `[u8; 56]` name }.
///
/// # Safety
///
/// `dir` must point at the 4096-byte directory buffer just filled by
/// the fw_cfg read.
unsafe fn find_file(dir: *const u8, name: &[u8]) -> Option<u32> {
    // SAFETY: caller guarantees `dir` is the 4 KiB DIRBUF.
    let rd = |i: usize| -> u8 { unsafe { dir.add(i).read() } };
    let count = u32::from_be_bytes([rd(0), rd(1), rd(2), rd(3)]) as usize;
    for i in 0..count {
        let base = 4 + i * 64;
        if base + 64 > 4096 {
            break;
        }
        let select = u16::from_be_bytes([rd(base + 4), rd(base + 5)]) as u32;
        // Compare the name (offset +8, up to 56 bytes) to `name`,
        // requiring a NUL (or field end) right after.
        let mut matched = true;
        for (j, &want) in name.iter().enumerate() {
            if j >= 56 || rd(base + 8 + j) != want {
                matched = false;
                break;
            }
        }
        if matched {
            let after = name.len();
            if after >= 56 || rd(base + 8 + after) == 0 {
                return Some(select);
            }
        }
    }
    None
}
