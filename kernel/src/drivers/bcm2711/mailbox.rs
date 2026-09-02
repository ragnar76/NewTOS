//! BCM2711 VideoCore mailbox transport (property channel).
//!
//! Reference: [`docs/hardware/mailbox-framebuffer.md`] — derived from the
//! BCM2711 ARM Peripherals datasheet (§1.3) and the raspberrypi/firmware
//! wiki (Mailboxes / Accessing-mailboxes / Mailbox-property-interface).
//!
//! Two 8-deep FIFOs at `MBOX_BASE`: mailbox 0 (VC→ARM, we read), mailbox
//! 1 (ARM→VC, we write). A message is a 16-byte-aligned address whose low
//! 4 bits carry the channel number. The property interface is channel 8.
//!
//! The property buffer lives in cacheable DRAM (the board maps the low
//! 3 GiB Normal-WBWA), but the VideoCore reads/writes it by DMA outside
//! the ARM caches. We therefore clean the buffer to PoC before handing it
//! over and invalidate it afterwards so we observe the VC's reply.

use core::arch::asm;

/// Mailbox register block, BCM2711 low-peripheral mode (high-half MMIO VA).
/// (`0xFE00_0000` peripheral base + `0xB880`.)
const MBOX_BASE: usize = crate::drivers::mmio_va(0xFE00_B880);

// Register offsets from `MBOX_BASE` (firmware wiki "Mailboxes").
// The `+ 0x00` keeps the offset column symmetric for documentation
// (same rationale as the boot page-table entries in `arch::boot`).
#[allow(clippy::identity_op)]
const MBOX0_READ: usize = MBOX_BASE + 0x00; // VC→ARM data
const MBOX0_STATUS: usize = MBOX_BASE + 0x18; // VC→ARM status
const MBOX1_WRITE: usize = MBOX_BASE + 0x20; // ARM→VC data
const MBOX1_STATUS: usize = MBOX_BASE + 0x38; // ARM→VC status

// Status bits (firmware wiki "Accessing mailboxes").
const MAIL_FULL: u32 = 0x8000_0000; // write mailbox full
const MAIL_EMPTY: u32 = 0x4000_0000; // read mailbox empty

/// Property/tags channel.
pub const CHANNEL_PROP: u32 = 8;

/// AArch64 cache line size assumed for PoC maintenance. The BCM2711
/// Cortex-A72 has a 64-byte D-cache line (CTR_EL0.DminLine = 4 words ×
/// 4). Using the architectural minimum is always safe for clean/
/// invalidate-by-VA loops.
const CACHE_LINE: usize = 64;

#[inline(always)]
fn dsb_sy() {
    // SAFETY: a barrier has no memory effects of its own.
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
}

/// Clean (write back) the buffer to the point of coherency so the VC
/// sees our request words.
///
/// # Safety
/// `va`/`len` must describe a mapped, readable region.
unsafe fn clean_dcache(va: usize, len: usize) {
    let start = va & !(CACHE_LINE - 1);
    let end = va + len;
    let mut p = start;
    while p < end {
        // SAFETY: `dc cvac` on a mapped VA is well-defined; loop bounds
        // stay within the buffer's rounded range.
        unsafe { asm!("dc cvac, {}", in(reg) p, options(nostack, preserves_flags)) };
        p += CACHE_LINE;
    }
    dsb_sy();
}

/// Invalidate the buffer at the point of coherency so we read the VC's
/// reply rather than a stale cached copy.
///
/// # Safety
/// `va`/`len` must describe a mapped region we own (invalidate discards
/// any dirty lines).
unsafe fn invalidate_dcache(va: usize, len: usize) {
    let start = va & !(CACHE_LINE - 1);
    let end = va + len;
    let mut p = start;
    while p < end {
        // SAFETY: `dc ivac` on a mapped VA is well-defined; the buffer is
        // ours and was just cleaned, so nothing dirty is lost.
        unsafe { asm!("dc ivac, {}", in(reg) p, options(nostack, preserves_flags)) };
        p += CACHE_LINE;
    }
    dsb_sy();
}

/// Write a 28-bit payload + channel to mailbox 1.
///
/// # Safety
/// MMIO; boot CPU.
unsafe fn write(channel: u32, payload: u32) {
    // SAFETY: MMIO register access at a fixed, valid address.
    unsafe {
        while core::ptr::read_volatile(MBOX1_STATUS as *const u32) & MAIL_FULL != 0 {
            core::hint::spin_loop();
        }
        dsb_sy();
        // The payload is already 16-byte aligned, so its low 4 bits are
        // free for the channel (see hardware ref §1: no shift).
        core::ptr::write_volatile(MBOX1_WRITE as *mut u32, (payload & !0xF) | channel);
    }
}

/// Read the next payload addressed to `channel` from mailbox 0,
/// discarding any reply for a different channel.
///
/// # Safety
/// MMIO; boot CPU.
unsafe fn read(channel: u32) -> u32 {
    loop {
        // SAFETY: MMIO register access at a fixed, valid address.
        unsafe {
            while core::ptr::read_volatile(MBOX0_STATUS as *const u32) & MAIL_EMPTY != 0 {
                core::hint::spin_loop();
            }
            dsb_sy();
            let word = core::ptr::read_volatile(MBOX0_READ as *const u32);
            if word & 0xF == channel {
                return word & !0xF;
            }
            // Not ours — discard and keep polling.
        }
    }
}

/// Perform one property-channel transaction against a 16-byte-aligned
/// message buffer. `buf_va` is the kernel (high-half) virtual address we
/// filled and will read back; `buf_pa` is its bus-visible address handed
/// to the mailbox. `len` is the buffer length in bytes.
///
/// Returns `true` iff the buffer's response code is `0x8000_0000`
/// (request processed successfully). On `false` the caller must treat
/// the results as invalid (no graphics) — never as a stubbed success.
///
/// # Safety
/// Boot CPU, single-threaded. `buf_va` must be mapped and 16-byte
/// aligned; `buf_pa` must be the matching VC-visible address.
pub unsafe fn call_property(buf_va: usize, buf_pa: u32, len: usize) -> bool {
    // SAFETY: caller guarantees the buffer mapping/alignment.
    unsafe {
        clean_dcache(buf_va, len);
        write(CHANNEL_PROP, buf_pa);
        let _ = read(CHANNEL_PROP);
        invalidate_dcache(buf_va, len);
        // Word [1] of the buffer is the response code.
        let code = core::ptr::read_volatile((buf_va + 4) as *const u32);
        code == 0x8000_0000
    }
}

// 16-byte-aligned property buffer for the VL805 firmware reset (the mailbox
// write OR's the channel into the low 4 bits, so the address must be aligned).
#[repr(C, align(16))]
struct XhciResetBuf {
    words: [u32; 8],
}
static mut XHCI_RESET_BUF: XhciResetBuf = XhciResetBuf { words: [0; 8] };

/// Ask the VideoCore firmware to (re)load the VIA VL805 xHCI firmware after a
/// PCIe reset — property tag `NOTIFY_XHCI_RESET` (`0x0003_0058`). `pcie_dev` is
/// the VL805's PCIe address `(bus<<20)|(slot<<15)|(func<<12)`. Real-Pi-4 USB-A
/// bring-up (PHASE-16 D7, `docs/api/usb/pcie-rpi4.md`). Returns the mailbox
/// success flag.
pub fn notify_xhci_reset(pcie_dev: u32) -> bool {
    // SAFETY: boot CPU, single-threaded; exclusive use of the static buffer.
    unsafe {
        let b = &mut (*core::ptr::addr_of_mut!(XHCI_RESET_BUF)).words;
        b[0] = 7 * 4; // total size in bytes
        b[1] = 0; // request code
        b[2] = 0x0003_0058; // PROPTAG_NOTIFY_XHCI_RESET
        b[3] = 4; // value buffer size
        b[4] = 4; // request parameter size
        b[5] = pcie_dev; // value: VL805 PCIe address
        b[6] = 0; // end tag
        let va = b.as_ptr() as usize;
        let pa = crate::arch::aarch64::mmu::va_to_pa(va as u64) as u32;
        call_property(va, pa, 7 * 4)
    }
}

// ─────────────────────────────────────────────────────────────────────
// Memory tags: "Get ARM memory" (0x00010005) / "Get VC memory" (0x00010006)
// ─────────────────────────────────────────────────────────────────────

/// Maximum number of `(base, size)` pairs we accept from a firmware
/// memory tag.
///
/// The documented response is a single pair, but the property-interface
/// reference explicitly reserves the right to answer with more:
/// *"Future formats may specify multiple base+size combinations."*
/// (`docs/hardware/mailbox-framebuffer.md` §3a, from the firmware
/// *Mailbox property interface* documentation.) We therefore offer a value
/// buffer sized for eight pairs and let the tag's **response length**
/// decide how many the firmware actually filled in.
pub const MAX_MEMORY_REGIONS: usize = 8;

/// One `(base, size)` region as reported by a firmware memory tag.
/// Both fields are `u32` on the wire; widened here because every caller
/// works in 64-bit physical addresses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemRegion {
    /// Physical base address of the region, in bytes.
    pub base: u64,
    /// Size of the region, in bytes.
    pub size: u64,
}

/// The VideoCore firmware's answer to "how is the DRAM split".
///
/// `arm` is what the tag calls *ARM memory* — the part of the fitted DRAM
/// the firmware hands to the CPU, i.e. total minus the `gpu_mem` split
/// (the complementary `vc` region). It is exactly the extent the kernel
/// may allocate from; the VideoCore owns `vc` (framebuffer, codecs,
/// firmware housekeeping) and we must not hand it to `Malloc`.
#[derive(Clone, Copy, Debug)]
pub struct MemoryReport {
    /// ARM-visible regions, `arm_len` of them are valid.
    pub arm: [MemRegion; MAX_MEMORY_REGIONS],
    /// Number of valid entries in [`Self::arm`] (0 if the firmware did
    /// not answer that tag).
    pub arm_len: usize,
    /// VideoCore-owned region, if the firmware answered that tag.
    pub vc: Option<MemRegion>,
}

// Word indices into `MEMORY_BUF` (see the buffer format in
// `docs/hardware/mailbox-framebuffer.md` §2).
const M_ARM_TAG: usize = 2; // tag id
const M_ARM_LEN: usize = 3; // value buffer size we provide, bytes
const M_ARM_CODE: usize = 4; // request/response code
const M_ARM_VAL: usize = 5; // value buffer: MAX_MEMORY_REGIONS × (base, size)
const M_VC_TAG: usize = M_ARM_VAL + 2 * MAX_MEMORY_REGIONS;
const M_VC_LEN: usize = M_VC_TAG + 1;
const M_VC_CODE: usize = M_VC_TAG + 2;
const M_VC_VAL: usize = M_VC_TAG + 3; // value buffer: (base, size)
const M_END: usize = M_VC_VAL + 2; // end tag
/// Buffer length in words: the end tag plus padding, rounded up so the
/// byte length is a multiple of 16 (the property buffer's alignment unit).
const MEMORY_BUF_WORDS: usize = (M_END + 1 + 3) & !3;

/// Bit 31 of a tag's code word: set in a response, clear in a request.
const TAG_RESPONSE: u32 = 0x8000_0000;

/// 16-byte-aligned property buffer for the memory query (the mailbox
/// write OR's the channel into the low 4 bits, so the address must be
/// aligned).
#[repr(C, align(16))]
struct MemoryBuf {
    words: [u32; MEMORY_BUF_WORDS],
}
static mut MEMORY_BUF: MemoryBuf = MemoryBuf { words: [0; MEMORY_BUF_WORDS] };

/// Ask the VideoCore firmware how the fitted DRAM is split between the
/// ARM and the GPU — property tags `0x0001_0005` ("Get ARM memory") and
/// `0x0001_0006` ("Get VC memory"), sent as one two-tag message.
///
/// Reference: `docs/hardware/mailbox-framebuffer.md` §3a (tag ids, value
/// layouts, response-length rule), extracted from the firmware *Mailbox
/// property interface* documentation. Per CLAUDE.md Rule 6 nothing here
/// is from memory.
///
/// Returns `None` when the transaction fails outright or the firmware
/// leaves the ARM tag unanswered (an unknown tag comes back with its
/// response bit clear — that is how a mailbox that does not implement the
/// tag, e.g. an emulator, presents itself). The caller must then fall
/// back to another source; it must never treat `None` as "zero memory".
#[allow(clippy::deref_addrof)]
pub fn get_memory() -> Option<MemoryReport> {
    // SAFETY: boot CPU, single-threaded; exclusive use of the static buffer.
    unsafe {
        {
            let b = &mut (*core::ptr::addr_of_mut!(MEMORY_BUF)).words;
            b.fill(0);
            b[0] = (MEMORY_BUF_WORDS * 4) as u32; // total size in bytes
            b[1] = 0; // request code
            b[M_ARM_TAG] = 0x0001_0005; // PROPTAG_GET_ARM_MEMORY
            b[M_ARM_LEN] = (MAX_MEMORY_REGIONS * 8) as u32;
            b[M_ARM_CODE] = 0; // request
            b[M_VC_TAG] = 0x0001_0006; // PROPTAG_GET_VC_MEMORY
            b[M_VC_LEN] = 8;
            b[M_VC_CODE] = 0; // request
            b[M_END] = 0; // end tag
        }

        let va = (&raw const MEMORY_BUF) as usize;
        let pa = crate::arch::aarch64::mmu::va_to_pa(va as u64) as u32;
        if !call_property(va, pa, MEMORY_BUF_WORDS * 4) {
            return None;
        }

        let b = &(*(&raw const MEMORY_BUF)).words;

        // The tag's response code carries the response bit plus the
        // *value length in bytes*. A firmware that does not know the tag
        // leaves the bit clear.
        let arm_code = b[M_ARM_CODE];
        if arm_code & TAG_RESPONSE == 0 {
            return None;
        }
        let arm_bytes =
            ((arm_code & !TAG_RESPONSE) as usize).min(MAX_MEMORY_REGIONS * 8);
        let arm_len = arm_bytes / 8;
        if arm_len == 0 {
            return None;
        }
        let mut arm = [MemRegion::default(); MAX_MEMORY_REGIONS];
        for (i, r) in arm.iter_mut().enumerate().take(arm_len) {
            r.base = b[M_ARM_VAL + 2 * i] as u64;
            r.size = b[M_ARM_VAL + 2 * i + 1] as u64;
        }

        let vc_code = b[M_VC_CODE];
        let vc = if vc_code & TAG_RESPONSE != 0 && (vc_code & !TAG_RESPONSE) >= 8 {
            Some(MemRegion { base: b[M_VC_VAL] as u64, size: b[M_VC_VAL + 1] as u64 })
        } else {
            None
        };

        Some(MemoryReport { arm, arm_len, vc })
    }
}
