//! BCM2711 VideoCore framebuffer (allocated via the mailbox property
//! interface).
//!
//! Reference: [`docs/hardware/mailbox-framebuffer.md`]. Unlike QEMU
//! `ramfb` (where the guest allocates the pixel store and points the
//! device at it), here the **VideoCore allocates** the framebuffer: we
//! send one property message setting the geometry/depth/order and an
//! allocate tag, and the VC returns the buffer's bus address, size and
//! pitch.
//!
//! This is the RPi4 counterpart of `qemu_virt::ramfb`, behind the
//! `board-rpi4` feature, implementing the same [`Framebuffer`] trait.
//! First test vehicle is QEMU `raspi4b`; the GDB-observed mailbox
//! transaction is the primary success signal (see the hardware ref §5).

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::arch::aarch64::mmu::{va_to_pa, VA_OFFSET};
use crate::drivers::bcm2711::mailbox;
use crate::hal::framebuffer::Framebuffer;

/// Default display geometry for bring-up (32-bit XRGB).
const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const DEPTH_BPP: u32 = 32;
/// VC pixel order. `0 = BGR, 1 = RGB` (firmware wiki). Our pixel store is
/// XRGB8888 packed by [`hal::framebuffer::rgb`] as `0xFF_RR_GG_BB`, i.e.
/// little-endian memory bytes `[B, G, R, X]` — blue at the lowest
/// address. The VC's **BGR** order reads the lowest byte as blue, which
/// matches; selecting RGB here swaps the red/blue channels (verified by
/// QMP screendump on raspi4b). So BGR is correct for our layout.
const PIXEL_ORDER_BGR: u32 = 0;

/// Mask turning a VideoCore bus address into an ARM physical address:
/// the top 2 bits select the cache alias (hardware ref §4).
const BUS_TO_PHYS: u32 = 0x3FFF_FFFF;

/// Property message buffer, 16-byte aligned (mailbox requirement). Sized
/// generously (36 words) to hold the six-tag allocate sequence plus the
/// end tag and 16-byte padding.
#[repr(C, align(16))]
struct MboxBuf {
    words: [u32; 36],
}

static mut MBOX_BUF: MboxBuf = MboxBuf { words: [0; 36] };

/// Indices into [`MBOX_BUF`] for the response fields we read back.
const I_PHYS_W: usize = 5; // set-physical width (echoed/adjusted)
const I_PHYS_H: usize = 6; // set-physical height
const I_FB_BASE: usize = 23; // allocate → base (bus address)
const I_FB_SIZE: usize = 24; // allocate → size
const I_PITCH: usize = 28; // get-pitch → bytes per line

/// The installed framebuffer.
pub struct VcFb {
    pixels: AtomicU64, // high-half VA of the pixel store
    width: AtomicU32,
    height: AtomicU32,
    stride_px: AtomicU32,
}

/// Board-selected framebuffer instance.
pub static FRAMEBUFFER: VcFb = VcFb {
    pixels: AtomicU64::new(0),
    width: AtomicU32::new(0),
    height: AtomicU32::new(0),
    stride_px: AtomicU32::new(0),
};

impl Framebuffer for VcFb {
    fn width(&self) -> u32 {
        self.width.load(Ordering::Acquire)
    }
    fn height(&self) -> u32 {
        self.height.load(Ordering::Acquire)
    }
    fn stride_px(&self) -> u32 {
        self.stride_px.load(Ordering::Acquire)
    }
    fn pixels(&self) -> *mut u32 {
        self.pixels.load(Ordering::Acquire) as *mut u32
    }
}

/// Fill the property buffer with the framebuffer-allocate tag sequence.
///
/// # Safety
/// Single-CPU boot; `MBOX_BUF` is a BSS static.
#[allow(clippy::deref_addrof)] // see `ramfb::fwcfg_dma` for the static-mut idiom.
unsafe fn build_request() {
    // SAFETY: single-CPU boot context, exclusive access to the static.
    let b = unsafe { &mut (*(&raw mut MBOX_BUF)).words };

    b[0] = 0; // total size in bytes, patched below
    b[1] = 0x0000_0000; // request

    // set physical (display) width/height — tag 0x00048003, value 8B.
    b[2] = 0x0004_8003;
    b[3] = 8;
    b[4] = 0;
    b[5] = WIDTH;
    b[6] = HEIGHT;

    // set virtual (buffer) width/height — tag 0x00048004, value 8B.
    b[7] = 0x0004_8004;
    b[8] = 8;
    b[9] = 0;
    b[10] = WIDTH;
    b[11] = HEIGHT;

    // set depth — tag 0x00048005, value 4B.
    b[12] = 0x0004_8005;
    b[13] = 4;
    b[14] = 0;
    b[15] = DEPTH_BPP;

    // set pixel order — tag 0x00048006, value 4B.
    b[16] = 0x0004_8006;
    b[17] = 4;
    b[18] = 0;
    b[19] = PIXEL_ORDER_BGR;

    // allocate buffer — tag 0x00040001, value 8B (req: alignment; resp:
    // base + size).
    b[20] = 0x0004_0001;
    b[21] = 8;
    b[22] = 0;
    b[23] = 16; // requested alignment in bytes
    b[24] = 0;

    // get pitch — tag 0x00040008, value 4B (resp: bytes per line).
    b[25] = 0x0004_0008;
    b[26] = 4;
    b[27] = 0;
    b[28] = 0;

    // end tag.
    b[29] = 0x0000_0000;

    // Total size = whole buffer (incl. padding to a 16-byte multiple).
    b[0] = (b.len() * 4) as u32;
}

/// Bring up the VideoCore framebuffer. Returns `false` if the mailbox
/// transaction fails or returns an implausible buffer — the caller then
/// skips graphics (never a stubbed success).
///
/// # Safety
/// Boot CPU, single-threaded, after the MMU/heap are up.
#[allow(clippy::deref_addrof)]
pub unsafe fn init() -> bool {
    // SAFETY: single-CPU boot; exclusive use of the static buffer.
    unsafe {
        build_request();

        let buf_va = (&raw const MBOX_BUF) as usize;
        let buf_pa = va_to_pa(buf_va as u64) as u32;
        let len = core::mem::size_of::<MboxBuf>();

        if !mailbox::call_property(buf_va, buf_pa, len) {
            return false;
        }

        let b = &(*(&raw const MBOX_BUF)).words;
        let fb_bus = b[I_FB_BASE];
        let fb_size = b[I_FB_SIZE];
        let pitch = b[I_PITCH];
        let w = b[I_PHYS_W];
        let h = b[I_PHYS_H];

        // A zero base / pitch means the VC did not allocate (e.g. the
        // emulated path is incomplete) — bail out cleanly.
        if fb_bus == 0 || fb_size == 0 || pitch == 0 {
            return false;
        }

        let fb_phys = (fb_bus & BUS_TO_PHYS) as u64;
        let fb_va = fb_phys + VA_OFFSET; // draw through the high half

        FRAMEBUFFER.pixels.store(fb_va, Ordering::Release);
        FRAMEBUFFER.width.store(w, Ordering::Release);
        FRAMEBUFFER.height.store(h, Ordering::Release);
        FRAMEBUFFER.stride_px.store(pitch / 4, Ordering::Release);
        true
    }
}
