//! Minimal flat device-tree (FDT v17) parser.
//!
//! Used by Phase 03 D8 to discover actual DRAM extent at runtime
//! instead of trusting the compile-time `board::DRAM_SIZE` constant.
//! We only extract what the memory manager needs: the first
//! `/memory@…` node's first `(address, size)` pair from its `reg`
//! property, plus the root-level `#address-cells` / `#size-cells`
//! that govern how that property is decoded.
//!
//! References:
//! - Devicetree Specification v0.4, §5 ("Flattened Devicetree Format")
//! - Linux `Documentation/devicetree/booting-without-of.txt`
//!
//! This parser deliberately ignores everything else (interrupts, CPUs,
//! status flags, multi-bank memory). Later phases that need richer DT
//! information will grow this module — for D8 a single memory bank is
//! sufficient on QEMU virt and RPi4.

use core::ptr;

const FDT_MAGIC: u32 = 0xd00d_feed;

const FDT_BEGIN_NODE: u32 = 0x0000_0001;
const FDT_END_NODE: u32 = 0x0000_0002;
const FDT_PROP: u32 = 0x0000_0003;
const FDT_NOP: u32 = 0x0000_0004;
const FDT_END: u32 = 0x0000_0009;

/// Parsed memory information from the first `/memory@…` node.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // `start` is read by callers via destructuring only.
pub struct DtbMemory {
    /// Physical base address of the memory bank.
    pub start: u64,
    /// Size of the memory bank in bytes.
    pub size: u64,
}

/// Try to parse the DTB at `dtb_phys` and return the first memory
/// bank. Returns `None` on any malformed structure, missing magic,
/// or absent memory node — callers must fall back to the build-time
/// `board::DRAM_SIZE` in that case.
///
/// # Safety
///
/// `dtb_phys` must either be 0 (no DTB) or point to a flat-device-tree
/// blob that is fully readable for at least the size declared in its
/// header. The QEMU `-kernel` path and the RPi firmware both satisfy
/// this; bare `cargo test` runs do not pass a DTB and call us with 0.
pub unsafe fn parse_memory(dtb_phys: usize) -> Option<DtbMemory> {
    if dtb_phys == 0 {
        return None;
    }
    let base = dtb_phys as *const u8;

    // ---- header ---------------------------------------------------
    let magic = unsafe { read_be_u32(base, 0) };
    if magic != FDT_MAGIC {
        return None;
    }
    let off_dt_struct = unsafe { read_be_u32(base, 8) } as usize;
    let off_dt_strings = unsafe { read_be_u32(base, 12) } as usize;
    let size_dt_struct = unsafe { read_be_u32(base, 36) } as usize;

    let strings_base = unsafe { base.add(off_dt_strings) };
    let mut p = unsafe { base.add(off_dt_struct) };
    let end = unsafe { p.add(size_dt_struct) };

    // ---- defaults from the FDT spec --------------------------------
    let mut root_addr_cells: u32 = 2;
    let mut root_size_cells: u32 = 1;

    let mut depth: u32 = 0;
    let mut in_memory_node = false;

    while p < end {
        let tok = unsafe { read_be_u32(p, 0) };
        p = unsafe { p.add(4) };

        match tok {
            FDT_BEGIN_NODE => {
                // NUL-terminated node name, then padded to 4 bytes.
                let name_len = unsafe { cstr_len(p) };
                let is_memory = depth == 1 && unsafe { is_memory_name(p, name_len) };
                let advance = align_up(name_len + 1, 4);
                p = unsafe { p.add(advance) };
                depth += 1;
                if is_memory {
                    in_memory_node = true;
                }
            }
            FDT_END_NODE => {
                if in_memory_node && depth == 2 {
                    in_memory_node = false;
                }
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            FDT_PROP => {
                let len = unsafe { read_be_u32(p, 0) } as usize;
                let nameoff = unsafe { read_be_u32(p, 4) } as usize;
                p = unsafe { p.add(8) };
                let val = p;
                let advance = align_up(len, 4);
                p = unsafe { p.add(advance) };

                let prop_name_ptr = unsafe { strings_base.add(nameoff) };

                if depth == 1 {
                    // Root-level cell-count overrides.
                    if unsafe { cstr_eq(prop_name_ptr, b"#address-cells") } && len >= 4 {
                        root_addr_cells = unsafe { read_be_u32(val, 0) };
                    } else if unsafe { cstr_eq(prop_name_ptr, b"#size-cells") } && len >= 4 {
                        root_size_cells = unsafe { read_be_u32(val, 0) };
                    }
                } else if in_memory_node
                    && depth == 2
                    && unsafe { cstr_eq(prop_name_ptr, b"reg") }
                {
                    let cell_bytes = (root_addr_cells + root_size_cells) as usize * 4;
                    if len < cell_bytes {
                        return None;
                    }
                    let addr = unsafe { read_be_cells(val, root_addr_cells) };
                    let size = unsafe {
                        read_be_cells(val.add(root_addr_cells as usize * 4), root_size_cells)
                    };
                    return Some(DtbMemory { start: addr, size });
                }
            }
            FDT_NOP => {}
            FDT_END => return None,
            _ => return None, // unknown token — give up safely
        }
    }
    None
}

/// PCIe host-bridge config-space (ECAM) region from the device tree.
#[derive(Clone, Copy)]
pub struct DtbPcie {
    /// ECAM (configuration space) physical base address.
    pub ecam_base: u64,
    /// ECAM size in bytes.
    pub ecam_size: u64,
}

/// Find the first `pci@…` / `pcie@…` node and return its ECAM region (the
/// node's `reg`). `None` on absent/malformed DTB. Used by the USB host stack
/// (PHASE-16) to locate the xHCI on QEMU `virt`, where the ECAM moved to a
/// high address (`0x40_1000_0000`) that is not in our build-time MMIO map.
///
/// # Safety
///
/// `dtb_phys` must point at a readable flattened device tree (or be 0).
pub unsafe fn parse_pcie_ecam(dtb_phys: usize) -> Option<DtbPcie> {
    if dtb_phys == 0 {
        return None;
    }
    // Unlike `parse_memory` (called early, while the boot identity map is
    // still live in TTBR0), this runs *after* the low-half is dropped
    // (ADR-014 / EMPTY_L0), so reach the DTB through its high-half VA.
    let base = (dtb_phys + super::mmu::VA_OFFSET as usize) as *const u8;
    if unsafe { read_be_u32(base, 0) } != FDT_MAGIC {
        return None;
    }
    let off_dt_struct = unsafe { read_be_u32(base, 8) } as usize;
    let off_dt_strings = unsafe { read_be_u32(base, 12) } as usize;
    let size_dt_struct = unsafe { read_be_u32(base, 36) } as usize;
    let strings_base = unsafe { base.add(off_dt_strings) };
    let mut p = unsafe { base.add(off_dt_struct) };
    let end = unsafe { p.add(size_dt_struct) };

    let mut root_addr_cells: u32 = 2;
    let mut root_size_cells: u32 = 1;
    let mut depth: u32 = 0;
    let mut in_pci_node = false;

    while p < end {
        let tok = unsafe { read_be_u32(p, 0) };
        p = unsafe { p.add(4) };
        match tok {
            FDT_BEGIN_NODE => {
                let name_len = unsafe { cstr_len(p) };
                let is_pci = depth == 1 && unsafe { is_pci_name(p, name_len) };
                p = unsafe { p.add(align_up(name_len + 1, 4)) };
                depth += 1;
                if is_pci {
                    in_pci_node = true;
                }
            }
            FDT_END_NODE => {
                if in_pci_node && depth == 2 {
                    in_pci_node = false;
                }
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            FDT_PROP => {
                let len = unsafe { read_be_u32(p, 0) } as usize;
                let nameoff = unsafe { read_be_u32(p, 4) } as usize;
                p = unsafe { p.add(8) };
                let val = p;
                p = unsafe { p.add(align_up(len, 4)) };
                let prop_name_ptr = unsafe { strings_base.add(nameoff) };

                if depth == 1 {
                    if unsafe { cstr_eq(prop_name_ptr, b"#address-cells") } && len >= 4 {
                        root_addr_cells = unsafe { read_be_u32(val, 0) };
                    } else if unsafe { cstr_eq(prop_name_ptr, b"#size-cells") } && len >= 4 {
                        root_size_cells = unsafe { read_be_u32(val, 0) };
                    }
                } else if in_pci_node
                    && depth == 2
                    && unsafe { cstr_eq(prop_name_ptr, b"reg") }
                {
                    let cell_bytes = (root_addr_cells + root_size_cells) as usize * 4;
                    if len < cell_bytes {
                        return None;
                    }
                    let ecam_base = unsafe { read_be_cells(val, root_addr_cells) };
                    let ecam_size = unsafe {
                        read_be_cells(val.add(root_addr_cells as usize * 4), root_size_cells)
                    };
                    return Some(DtbPcie { ecam_base, ecam_size });
                }
            }
            FDT_NOP => {}
            FDT_END => return None,
            _ => return None,
        }
    }
    None
}

/// True if the node name at `p` (length `len`) is `pci`/`pcie`, optionally
/// `@…`-addressed (the FDT convention for the host bridge).
#[inline]
unsafe fn is_pci_name(p: *const u8, len: usize) -> bool {
    const PCI: &[u8] = b"pci";
    if len < PCI.len() {
        return false;
    }
    for (i, &b) in PCI.iter().enumerate() {
        if unsafe { *p.add(i) } != b {
            return false;
        }
    }
    // "pci", "pcie", "pci@…", "pcie@…" all start with "pci".
    true
}

#[inline]
fn align_up(v: usize, a: usize) -> usize {
    (v + a - 1) & !(a - 1)
}

#[inline]
unsafe fn read_be_u32(p: *const u8, off: usize) -> u32 {
    u32::from_be_bytes(unsafe { ptr::read_unaligned(p.add(off) as *const [u8; 4]) })
}

#[inline]
unsafe fn read_be_cells(p: *const u8, cells: u32) -> u64 {
    let mut v: u64 = 0;
    for i in 0..cells as usize {
        v = (v << 32) | unsafe { read_be_u32(p, i * 4) } as u64;
    }
    v
}

/// Length of a NUL-terminated byte string (excluding the NUL).
#[inline]
unsafe fn cstr_len(p: *const u8) -> usize {
    let mut n = 0usize;
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    n
}

#[inline]
unsafe fn cstr_eq(p: *const u8, expected: &[u8]) -> bool {
    for (i, &b) in expected.iter().enumerate() {
        if unsafe { *p.add(i) } != b {
            return false;
        }
    }
    unsafe { *p.add(expected.len()) == 0 }
}

/// True if the name at `p` (length `len`) is `memory` or starts with
/// `memory@…` (the FDT convention for an addressed memory node).
#[inline]
unsafe fn is_memory_name(p: *const u8, len: usize) -> bool {
    const MEM: &[u8] = b"memory";
    if len < MEM.len() {
        return false;
    }
    for (i, &b) in MEM.iter().enumerate() {
        if unsafe { *p.add(i) } != b {
            return false;
        }
    }
    len == MEM.len() || unsafe { *p.add(MEM.len()) } == b'@'
}
