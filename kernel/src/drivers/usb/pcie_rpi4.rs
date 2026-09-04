//! BCM2711 PCIe root-complex bring-up for the real Pi-4 (PHASE-16 D7).
//!
//! The four USB-A sockets are driven by the **VL805** xHCI, a PCIe endpoint
//! behind the BCM2711 PCIe root complex. This brings the RC up, trains the
//! link, enumerates the VL805 (bus 1, slot 0), assigns its xHCI BAR into the
//! outbound window and loads its firmware — then returns the CPU base the
//! existing `xhci::Xhci::open` runs on.
//!
//! Clean-room from `references/circle` (`lib/bcmpciehostbridge.cpp` +
//! `lib/usb/xhcidevice.cpp`); the BCM2711 datasheet does not document the PCIe
//! RC. Spec: `docs/api/usb/pcie-rpi4.md`. **Real-HW only** (QEMU does not model
//! the VL805).

use crate::arch::aarch64::{delay_ms, dtb, mmu};
use crate::drivers::mmio_va;

// ── addresses (RASPPI == 4) ──────────────────────────────────────────────
const RC_BASE: u64 = 0xFD50_0000; // PCIe RC register block (mapped low-half MMIO)
const OUT_CPU: u64 = 0x6_0000_0000; // outbound window, CPU side (xHCI BAR lands here)
const OUT_PCIE: u64 = 0xF800_0000; // outbound window, PCIe side
const OUT_SIZE: u64 = 0x400_0000; // 64 MiB
const DMA_WINDOW_SIZE: u64 = 0x4000_0000; // inbound RC_BAR2: cover low 1 GiB (our DMA structs)

// VL805 on the bus.
const XHCI_BUS: u32 = 1;
const XHCI_SLOT: u32 = 0;
const XHCI_FUNC: u32 = 0;
const XHCI_CLASS: u32 = 0x0C0330; // serial bus / USB / xHCI

// ── RC register offsets ──────────────────────────────────────────────────
const PCIE_RGR1_SW_INIT_1: u64 = 0x9210;
const SW_INIT_GENERIC_MASK: u32 = 0x2;
const SW_INIT_GENERIC_SHIFT: u32 = 1;
const SW_INIT_PERST_MASK: u32 = 0x1;
const SW_INIT_PERST_SHIFT: u32 = 0;

const PCIE_MISC_MISC_CTRL: u64 = 0x4008;
const MISC_SCB_ACCESS_EN_MASK: u32 = 0x1000;
const MISC_SCB_ACCESS_EN_SHIFT: u32 = 0xc;
const MISC_CFG_READ_UR_MODE_MASK: u32 = 0x2000;
const MISC_CFG_READ_UR_MODE_SHIFT: u32 = 0xd;
const MISC_MAX_BURST_SIZE_MASK: u32 = 0x300000;
const MISC_MAX_BURST_SIZE_SHIFT: u32 = 0x14;
const MISC_SCB0_SIZE_MASK: u32 = 0xf800_0000;
const MISC_SCB0_SIZE_SHIFT: u32 = 0x1b;

const PCIE_MISC_CPU_2_PCIE_MEM_WIN0_LO: u64 = 0x400c;
const PCIE_MISC_CPU_2_PCIE_MEM_WIN0_HI: u64 = 0x4010;
const PCIE_MISC_RC_BAR1_CONFIG_LO: u64 = 0x402c;
const PCIE_MISC_RC_BAR2_CONFIG_LO: u64 = 0x4034;
const PCIE_MISC_RC_BAR2_CONFIG_HI: u64 = 0x4038;
const PCIE_MISC_RC_BAR3_CONFIG_LO: u64 = 0x403c;
const RC_BAR_SIZE_MASK: u32 = 0x1f;
const RC_BAR_SIZE_SHIFT: u32 = 0;

const PCIE_MISC_PCIE_STATUS: u64 = 0x4068;
const STATUS_PCIE_PORT_MASK: u32 = 0x80;
const STATUS_PCIE_PORT_SHIFT: u32 = 7;
const STATUS_DL_ACTIVE_MASK: u32 = 0x20;
const STATUS_DL_ACTIVE_SHIFT: u32 = 5;
const STATUS_PHYLINKUP_MASK: u32 = 0x10;
const STATUS_PHYLINKUP_SHIFT: u32 = 4;

const PCIE_MISC_REVISION: u64 = 0x406c;

const PCIE_MISC_CPU_2_PCIE_MEM_WIN0_BASE_LIMIT: u64 = 0x4070;
const WIN0_BL_LIMIT_MASK: u32 = 0xfff0_0000;
const WIN0_BL_LIMIT_SHIFT: u32 = 0x14;
const WIN0_BL_BASE_MASK: u32 = 0xfff0;
const WIN0_BL_BASE_SHIFT: u32 = 0x4;
const WIN0_NUM_MASK_BITS: u32 = 0xc;
const PCIE_MISC_CPU_2_PCIE_MEM_WIN0_BASE_HI: u64 = 0x4080;
const WIN0_BASE_HI_MASK: u32 = 0xff;
const WIN0_BASE_HI_SHIFT: u32 = 0;
const PCIE_MISC_CPU_2_PCIE_MEM_WIN0_LIMIT_HI: u64 = 0x4084;
const WIN0_LIMIT_HI_MASK: u32 = 0xff;
const WIN0_LIMIT_HI_SHIFT: u32 = 0;

const PCIE_MISC_HARD_PCIE_HARD_DEBUG: u64 = 0x4204;
const HARD_SERDES_IDDQ_MASK: u32 = 0x0800_0000;
const HARD_SERDES_IDDQ_SHIFT: u32 = 0x1b;
const HARD_CLKREQ_DEBUG_ENABLE_MASK: u32 = 0x2;
const HARD_CLKREQ_DEBUG_ENABLE_SHIFT: u32 = 0x1;

const PCIE_INTR2_CPU_BASE: u64 = 0x4300;
const INTR2_CLR: u64 = 0x8;
const INTR2_MASK_SET: u64 = 0x10;

const PCIE_RC_CFG_VENDOR_VSPEC1: u64 = 0x0188;
const VSPEC1_ENDIAN_MODE_BAR2_MASK: u32 = 0xc;
const VSPEC1_ENDIAN_MODE_BAR2_SHIFT: u32 = 0x2;
const PCIE_RC_CFG_PRIV1_ID_VAL3: u64 = 0x043c;
const PRIV1_CLASS_CODE_MASK: u32 = 0xff_ffff;
const PRIV1_CLASS_CODE_SHIFT: u32 = 0;

const BRCM_PCIE_CAP_REGS: u64 = 0x00ac;
const PCIE_EXT_CFG_INDEX: u64 = 0x9000;
const PCIE_EXT_CFG_DATA: u64 = 0x8000;

const PCIE_GEN: u32 = 2;
const BURST_SIZE_128: u32 = 0;

// ── PCI capability + config-space offsets (standard) ─────────────────────
const PCI_EXP_LNKCAP: u64 = 0x0c;
const PCI_EXP_LNKCTL2: u64 = 0x30;
const PCI_EXP_LNKCAP_SLS: u32 = 0xf;
const PCI_CLASS_REVISION: u64 = 0x08;
const PCI_HEADER_TYPE: u64 = 0x0e;
const PCI_CACHE_LINE_SIZE: u64 = 0x0c;
const PCI_SECONDARY_BUS: u64 = 0x19;
const PCI_SUBORDINATE_BUS: u64 = 0x1a;
const PCI_MEMORY_BASE: u64 = 0x20;
const PCI_MEMORY_LIMIT: u64 = 0x22;
const PCI_BRIDGE_CONTROL: u64 = 0x3e;
const PCI_COMMAND: u64 = 0x04;
const PCI_BASE_ADDRESS_0: u64 = 0x10;
const PCI_BASE_ADDRESS_1: u64 = 0x14;
const PCI_INTERRUPT_PIN: u64 = 0x3d;
const PCI_HEADER_TYPE_BRIDGE: u8 = 1;
const PCI_HEADER_TYPE_NORMAL: u8 = 0;
const PCI_COMMAND_MEMORY: u16 = 0x2;
const PCI_COMMAND_MASTER: u16 = 0x4;
const PCI_COMMAND_PARITY: u16 = 0x40;
const PCI_COMMAND_SERR: u16 = 0x100;
const PCI_BRIDGE_CTL_PARITY: u8 = 0x01;
const PCI_BASE_ADDRESS_MEM_TYPE_64: u32 = 0x4;

// ── raw MMIO at a high-half VA ───────────────────────────────────────────
#[inline]
fn rd32(va: u64) -> u32 {
    unsafe { core::ptr::read_volatile(va as *const u32) }
}
#[inline]
fn wr32(va: u64, v: u32) {
    unsafe { core::ptr::write_volatile(va as *mut u32, v) }
}
#[inline]
fn rd16(va: u64) -> u16 {
    unsafe { core::ptr::read_volatile(va as *const u16) }
}
#[inline]
fn wr16(va: u64, v: u16) {
    unsafe { core::ptr::write_volatile(va as *mut u16, v) }
}
#[inline]
fn rd8(va: u64) -> u8 {
    unsafe { core::ptr::read_volatile(va as *const u8) }
}
#[inline]
fn wr8(va: u64, v: u8) {
    unsafe { core::ptr::write_volatile(va as *mut u8, v) }
}

/// RC register VA (the RC block is in the low-half MMIO map).
#[inline]
fn reg(off: u64) -> u64 {
    mmio_va((RC_BASE + off) as usize) as u64
}

/// Read-modify-write a field in an RC register (circle `wr_fld`), with read-back.
fn wr_fld(off: u64, mask: u32, shift: u32, val: u32) {
    let va = reg(off);
    let r = (rd32(va) & !mask) | ((val << shift) & mask);
    wr32(va, r);
    let _ = rd32(va); // read-back (wr_fld_rb)
}
fn rd_fld(off: u64, mask: u32, shift: u32) -> u32 {
    (rd32(reg(off)) & mask) >> shift
}

fn ilog2(v: u64) -> u32 {
    let mut l = 0u32;
    while (1u64 << l) < v {
        l += 1;
    }
    l
}

/// circle `encode_ibar_size`: inbound BAR size → RC_BARx_CONFIG_LO.SIZE field.
fn encode_ibar_size(size: u64) -> u32 {
    let l2 = ilog2(size);
    if (12..=15).contains(&l2) {
        l2 - 12 + 0x1c
    } else if (16..=35).contains(&l2) {
        l2 - 15
    } else {
        0
    }
}

fn bridge_sw_init(val: u32) {
    wr_fld(
        PCIE_RGR1_SW_INIT_1,
        SW_INIT_GENERIC_MASK,
        SW_INIT_GENERIC_SHIFT,
        val,
    );
}
fn perst_set(val: u32) {
    wr_fld(
        PCIE_RGR1_SW_INIT_1,
        SW_INIT_PERST_MASK,
        SW_INIT_PERST_SHIFT,
        val,
    );
}
fn link_up() -> bool {
    let v = rd32(reg(PCIE_MISC_PCIE_STATUS));
    let dla = (v & STATUS_DL_ACTIVE_MASK) >> STATUS_DL_ACTIVE_SHIFT;
    let plu = (v & STATUS_PHYLINKUP_MASK) >> STATUS_PHYLINKUP_SHIFT;
    dla != 0 && plu != 0
}
fn rc_mode() -> bool {
    (rd32(reg(PCIE_MISC_PCIE_STATUS)) & STATUS_PCIE_PORT_MASK) >> STATUS_PCIE_PORT_SHIFT != 0
}

/// Limit the link to a specific generation (`PCIE_GEN`) via the PCIe cap regs.
fn set_gen(gen: u32) {
    let cap = BRCM_PCIE_CAP_REGS;
    let lnkcap = rd32(reg(cap + PCI_EXP_LNKCAP));
    wr32(reg(cap + PCI_EXP_LNKCAP), (lnkcap & !PCI_EXP_LNKCAP_SLS) | gen);
    let lnkctl2 = rd16(reg(cap + PCI_EXP_LNKCTL2));
    wr16(
        reg(cap + PCI_EXP_LNKCTL2),
        (lnkctl2 & !0xf) | gen as u16,
    );
}

fn set_outbound_win(cpu: u64, pcie: u64, size: u64) {
    wr32(reg(PCIE_MISC_CPU_2_PCIE_MEM_WIN0_LO), pcie as u32);
    wr32(reg(PCIE_MISC_CPU_2_PCIE_MEM_WIN0_HI), (pcie >> 32) as u32);
    let cpu_mb = cpu >> 20;
    let limit_mb = (cpu + size - 1) >> 20;
    wr_fld(
        PCIE_MISC_CPU_2_PCIE_MEM_WIN0_BASE_LIMIT,
        WIN0_BL_BASE_MASK,
        WIN0_BL_BASE_SHIFT,
        cpu_mb as u32,
    );
    wr_fld(
        PCIE_MISC_CPU_2_PCIE_MEM_WIN0_BASE_LIMIT,
        WIN0_BL_LIMIT_MASK,
        WIN0_BL_LIMIT_SHIFT,
        limit_mb as u32,
    );
    wr_fld(
        PCIE_MISC_CPU_2_PCIE_MEM_WIN0_BASE_HI,
        WIN0_BASE_HI_MASK,
        WIN0_BASE_HI_SHIFT,
        (cpu_mb >> WIN0_NUM_MASK_BITS) as u32,
    );
    wr_fld(
        PCIE_MISC_CPU_2_PCIE_MEM_WIN0_LIMIT_HI,
        WIN0_LIMIT_HI_MASK,
        WIN0_LIMIT_HI_SHIFT,
        (limit_mb >> WIN0_NUM_MASK_BITS) as u32,
    );
}

// ── config-space access ──────────────────────────────────────────────────
fn cfg_index(bus: u32, slot: u32, func: u32) -> u32 {
    ((slot & 0x1f) << 15) | ((func & 0x07) << 12) | (bus << 20)
}
/// VA of config register `where` for `(bus, slot, func)`. Bus 0 = the RC
/// itself (direct); bus ≥1 routes through the EXT_CFG index/data window.
fn map_conf(bus: u32, slot: u32, func: u32, off: u64) -> u64 {
    if bus == 0 {
        if slot != 0 {
            return 0;
        }
        return reg(off);
    }
    wr32(reg(PCIE_EXT_CFG_INDEX), cfg_index(bus, slot, func));
    reg(PCIE_EXT_CFG_DATA + off)
}

/// The RC bring-up sequence (`pcie_setup`, RASPPI==4). Returns false on link
/// failure.
fn pcie_setup(rc_bar2_offset: u64, rc_bar2_size: u64) -> bool {
    bridge_sw_init(1); // reset bridge
    perst_set(1); // assert fundamental reset
    delay_ms(1); // ~100-200us
    bridge_sw_init(0); // bridge out of reset
    wr_fld(
        PCIE_MISC_HARD_PCIE_HARD_DEBUG,
        HARD_SERDES_IDDQ_MASK,
        HARD_SERDES_IDDQ_SHIFT,
        0,
    ); // power up SerDes
    delay_ms(1);

    let rev = rd32(reg(PCIE_MISC_REVISION)) & 0xffff;
    crate::kprintln!("[pcie] RC revision {:#06x}", rev);

    // MISC_CTRL: SCB access, CFG read UR mode, burst 128.
    wr_fld(
        PCIE_MISC_MISC_CTRL,
        MISC_SCB_ACCESS_EN_MASK,
        MISC_SCB_ACCESS_EN_SHIFT,
        1,
    );
    wr_fld(
        PCIE_MISC_MISC_CTRL,
        MISC_CFG_READ_UR_MODE_MASK,
        MISC_CFG_READ_UR_MODE_SHIFT,
        1,
    );
    wr_fld(
        PCIE_MISC_MISC_CTRL,
        MISC_MAX_BURST_SIZE_MASK,
        MISC_MAX_BURST_SIZE_SHIFT,
        BURST_SIZE_128,
    );

    // Inbound window RC_BAR2: the PCIe-bus offset + size come from the DTB
    // `dma-ranges` (a wrong offset HSE's the VL805 on its first DMA).
    let lo = (rc_bar2_offset as u32 & !RC_BAR_SIZE_MASK)
        | ((encode_ibar_size(rc_bar2_size) << RC_BAR_SIZE_SHIFT) & RC_BAR_SIZE_MASK);
    wr32(reg(PCIE_MISC_RC_BAR2_CONFIG_LO), lo);
    wr32(reg(PCIE_MISC_RC_BAR2_CONFIG_HI), (rc_bar2_offset >> 32) as u32);
    wr_fld(
        PCIE_MISC_MISC_CTRL,
        MISC_SCB0_SIZE_MASK,
        MISC_SCB0_SIZE_SHIFT,
        ilog2(rc_bar2_size) - 15,
    );
    crate::kprintln!(
        "[pcie] RC_BAR2 readback: lo={:#x} hi={:#x} misc_ctrl={:#x} (wrote lo={:#x} hi={:#x} scb0={})",
        rd32(reg(PCIE_MISC_RC_BAR2_CONFIG_LO)),
        rd32(reg(PCIE_MISC_RC_BAR2_CONFIG_HI)),
        rd32(reg(PCIE_MISC_MISC_CTRL)),
        lo,
        (rc_bar2_offset >> 32) as u32,
        ilog2(rc_bar2_size) - 15,
    );

    // Disable RC_BAR1 / RC_BAR3.
    wr_fld(PCIE_MISC_RC_BAR1_CONFIG_LO, RC_BAR_SIZE_MASK, RC_BAR_SIZE_SHIFT, 0);
    wr_fld(PCIE_MISC_RC_BAR3_CONFIG_LO, RC_BAR_SIZE_MASK, RC_BAR_SIZE_SHIFT, 0);

    // Clear + mask all interrupts.
    wr32(reg(PCIE_INTR2_CPU_BASE + INTR2_CLR), 0xffff_ffff);
    let _ = rd32(reg(PCIE_INTR2_CPU_BASE + INTR2_CLR));
    wr32(reg(PCIE_INTR2_CPU_BASE + INTR2_MASK_SET), 0xffff_ffff);
    let _ = rd32(reg(PCIE_INTR2_CPU_BASE + INTR2_MASK_SET));

    set_gen(PCIE_GEN);

    perst_set(0); // de-assert fundamental reset
    delay_ms(100);

    // Poll for link up (~100ms exponential backoff).
    let mut i = 1u32;
    let mut j = 0u32;
    let limit = 100u32;
    while j < limit && !link_up() {
        let step = if i + j > limit { limit - j } else { i };
        delay_ms(step as u64);
        j += i;
        i *= 2;
    }
    if !link_up() {
        crate::kprintln!("[pcie] link down");
        return false;
    }
    if !rc_mode() {
        crate::kprintln!("[pcie] misconfigured (EP mode)");
        return false;
    }

    // Outbound window: CPU 0x6_0000_0000 -> PCIe 0xF800_0000.
    set_outbound_win(OUT_CPU, OUT_PCIE, OUT_SIZE);

    // Present as a PCIe-PCIe bridge.
    wr_fld(
        PCIE_RC_CFG_PRIV1_ID_VAL3,
        PRIV1_CLASS_CODE_MASK,
        PRIV1_CLASS_CODE_SHIFT,
        0x060400,
    );

    // BAR endian + CLKREQ debug.
    wr_fld(
        PCIE_RC_CFG_VENDOR_VSPEC1,
        VSPEC1_ENDIAN_MODE_BAR2_MASK,
        VSPEC1_ENDIAN_MODE_BAR2_SHIFT,
        0,
    );
    wr_fld(
        PCIE_MISC_HARD_PCIE_HARD_DEBUG,
        HARD_CLKREQ_DEBUG_ENABLE_MASK,
        HARD_CLKREQ_DEBUG_ENABLE_SHIFT,
        1,
    );

    crate::kprintln!("[pcie] link up");
    let _ = rd_fld; // (kept for symmetry with circle rd_fld)
    true
}

/// Configure the RC's own bridge (bus 0, dev 0) — secondary bus, mem window,
/// command bits.
fn enable_bridge() -> bool {
    let conf = map_conf(0, 0, 0, 0);
    if conf == 0 {
        return false;
    }
    if rd32(reg(PCI_CLASS_REVISION)) >> 8 != 0x060400 || rd8(reg(PCI_HEADER_TYPE)) != PCI_HEADER_TYPE_BRIDGE
    {
        crate::kprintln!("[pcie] bridge not present");
        return false;
    }
    wr8(reg(PCI_CACHE_LINE_SIZE), 64 / 4);
    wr8(reg(PCI_SECONDARY_BUS), 1);
    wr8(reg(PCI_SUBORDINATE_BUS), 1);
    wr16(reg(PCI_MEMORY_BASE), (OUT_PCIE >> 16) as u16);
    wr16(reg(PCI_MEMORY_LIMIT), (OUT_PCIE >> 16) as u16);
    wr8(reg(PCI_BRIDGE_CONTROL), PCI_BRIDGE_CTL_PARITY);
    wr16(
        reg(PCI_COMMAND),
        PCI_COMMAND_MEMORY | PCI_COMMAND_MASTER | PCI_COMMAND_PARITY | PCI_COMMAND_SERR,
    );
    true
}

/// Configure the VL805 (bus 1, slot 0) — verify class, assign its 64-bit xHCI
/// BAR into the outbound window, enable mem+busmaster.
fn enable_device() -> bool {
    let c = |off: u64| map_conf(XHCI_BUS, XHCI_SLOT, XHCI_FUNC, off);
    if c(0) == 0 {
        return false;
    }
    let class = rd32(c(PCI_CLASS_REVISION)) >> 8;
    if class != XHCI_CLASS || rd8(c(PCI_HEADER_TYPE)) != PCI_HEADER_TYPE_NORMAL {
        crate::kprintln!("[pcie] VL805 not found (class {:#08x})", class);
        return false;
    }
    wr8(c(PCI_CACHE_LINE_SIZE), 64 / 4);
    wr32(
        c(PCI_BASE_ADDRESS_0),
        (OUT_PCIE as u32) | PCI_BASE_ADDRESS_MEM_TYPE_64,
    );
    wr32(c(PCI_BASE_ADDRESS_1), (OUT_PCIE >> 32) as u32);
    if rd8(c(PCI_INTERRUPT_PIN)) != 1 {
        wr8(c(PCI_INTERRUPT_PIN), 1);
    }
    wr16(
        c(PCI_COMMAND),
        PCI_COMMAND_MEMORY | PCI_COMMAND_MASTER | PCI_COMMAND_PARITY | PCI_COMMAND_SERR,
    );
    true
}

/// Bring up the PCIe RC, load the VL805 firmware, enumerate the VL805 and
/// return the CPU base its xHCI is reachable at (`0x6_0000_0000`, mapped
/// high-half). `None` on any failure. Mirrors `xhcidevice.cpp` ordering.
pub fn find_xhci(dtb_phys: usize) -> Option<u64> {
    // Probe the PCIe root complex before any destructive bring-up. This code
    // is real-HW-only: QEMU's `raspi4b` does not model the BCM2711 PCIe RC, so
    // the first RC register access (`PCIE_RGR1_SW_INIT_1`) takes a synchronous
    // external abort and panics the kernel before the desktop runs. A
    // recoverable probe returns `None` there (skip USB) while proceeding on
    // real hardware, which responds. The RC block is mapped device MMIO, so a
    // non-responding controller aborts (recovered), not translation-faults.
    if crate::arch::aarch64::exceptions::probe_read32(reg(PCIE_RGR1_SW_INIT_1)).is_none() {
        crate::kprintln!("[pcie] RC not present (no response at RC_BASE); USB disabled");
        return None;
    }

    // The outbound window is a >4 GiB address → high L2 device mapping.
    unsafe { mmu::map_device_high(OUT_CPU, 0x20_0000) }; // 2 MiB covers the xHCI BAR

    // Inbound DMA window (RC_BAR2): the PCIe-bus offset + size from the DTB
    // `dma-ranges`. A wrong offset makes the VL805 HSE on its first DMA. Fall
    // back to the identity low-1-GiB window if the DTB lacks dma-ranges.
    let (rc_offset, rc_size) = match unsafe { dtb::parse_pcie_dma_ranges(dtb_phys) } {
        Some(r) => {
            crate::kprintln!(
                "[pcie] dma-ranges: pcie={:#x} cpu={:#x} size={:#x}",
                r.pcie_addr,
                r.cpu_addr,
                r.size
            );
            // RC_BAR2 size must be a power of two — round the DTB size up.
            (r.pcie_addr, 1u64 << ilog2(r.size.max(1)))
        }
        None => {
            crate::kprintln!("[pcie] dma-ranges absent in DTB; identity 1 GiB window");
            (0, DMA_WINDOW_SIZE)
        }
    };

    // The inbound window maps CPU-physical `[0, rc_size)` onto PCIe
    // `[rc_offset, rc_offset + rc_size)`, so every DMA structure the
    // controller is told about must live below `rc_size`. The `.dma` block
    // is placed by the linker (in the Atari `$E00000` window, so the ST-RAM
    // pool keeps its fixed size), and the BCM2711 datasheet documents no
    // inbound window at all — it describes only the outbound PCIe range in
    // its address map (RP-008248-DS-1 §1.2.1/§1.2.2). So rather than assume
    // the placement is reachable, check it against the window we just
    // programmed and say so on the console.
    unsafe extern "C" {
        static __dma_start: u8;
        static __dma_end: u8;
    }
    let dma_lo = mmu::va_to_pa((&raw const __dma_start) as u64);
    let dma_hi = mmu::va_to_pa((&raw const __dma_end) as u64);
    if dma_hi > rc_size {
        crate::kprintln!(
            "[pcie] .dma block {:#x}..{:#x} is OUTSIDE the {:#x}-byte inbound \
             window — the controller cannot reach it; USB disabled",
            dma_lo, dma_hi, rc_size,
        );
        return None;
    }
    crate::kprintln!(
        "[pcie] .dma block {:#x}..{:#x} inside the {:#x}-byte inbound window",
        dma_lo, dma_hi, rc_size,
    );

    // The same PCIe-bus offset applies to every DMA address we hand the xHCI
    // controller (its inbound window is based at `rc_offset`, not 0).
    super::xhci::set_dma_bus_offset(rc_offset);

    if !pcie_setup(rc_offset, rc_size) {
        return None;
    }
    if !enable_bridge() {
        return None;
    }
    // Load the VIA VL805 firmware after the PCIe reset (mailbox NOTIFY_XHCI_RESET).
    let pcie_dev = (XHCI_BUS << 20) | (XHCI_SLOT << 15) | (XHCI_FUNC << 12);
    crate::drivers::bcm2711::mailbox::notify_xhci_reset(pcie_dev);
    if !enable_device() {
        return None;
    }
    crate::kprintln!("[pcie] VL805 xHCI enabled @ cpu {:#x}", OUT_CPU);
    Some(OUT_CPU)
}
