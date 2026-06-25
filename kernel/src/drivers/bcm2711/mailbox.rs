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
