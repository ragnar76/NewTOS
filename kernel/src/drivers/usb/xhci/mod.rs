//! xHCI host controller (PHASE-16 D1). Enough of the xHCI init sequence to
//! bring the controller up and read root-hub port connect status. Built on
//! `rust-osdev/xhci` (register layer). See `docs/api/usb/xhci-bringup.md`.
//!
//! DMA structures are cacheable static BSS — correct in QEMU (no caches).
//! The real Pi-4 (D7) must make them Normal-NC / cache-coherent (ADR-014).

#![allow(dead_code)]

use core::num::NonZeroUsize;

use xhci::accessor::Mapper;
use xhci::context::{self, InputHandler};
use xhci::ring::trb::{command, event, transfer};
use xhci::Registers;

use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::aarch64::mmu::va_to_pa;
use crate::drivers::mmio_va;
use crate::kprintln;

/// PCIe DMA bus offset. The controller reaches system RAM through the host
/// bridge's inbound window, whose PCIe-bus base is **not** 0 on the real Pi-4
/// (the `dma-ranges` `pcie_addr`, e.g. `0x4_0000_0000`). So a controller-
/// visible DMA address is the CPU physical address **plus** this offset; 0 on
/// QEMU (identity). Set by the board's controller discovery before `open`.
static DMA_BUS_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Set the PCIe DMA bus offset (the inbound-window `pcie_addr`).
pub fn set_dma_bus_offset(off: u64) {
    DMA_BUS_OFFSET.store(off, Ordering::Relaxed);
}

/// CPU virtual address of a DMA structure → the address the controller must
/// issue to reach it: physical address + the PCIe bus offset.
#[inline]
fn dma_addr(va: u64) -> u64 {
    va_to_pa(va) + DMA_BUS_OFFSET.load(Ordering::Relaxed)
}

/// Maps a peripheral physical address to its high-half VA — the same
/// `drivers::mmio_va` mapping all NewTOS MMIO uses (ADR-014).
#[derive(Clone)]
struct HighHalf;
impl Mapper for HighHalf {
    unsafe fn map(&mut self, phys: usize, _bytes: usize) -> NonZeroUsize {
        // SAFETY: the xHCI MMIO is mapped high-half by Stage-2.
        NonZeroUsize::new(mmio_va(phys)).expect("non-zero MMIO VA")
    }
    fn unmap(&mut self, _virt: usize, _bytes: usize) {}
}

const CMD_RING_LEN: usize = 16;
const EVT_RING_LEN: usize = 16;
const MAX_SLOTS: usize = 16;

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Trb([u32; 4]);

// The command ring, event ring, DCBAA and ERST each need **64-byte**
// alignment (xHCI spec). Wrap each so its offset inside the page-aligned
// block is 64-byte aligned (the compiler pads to the field's alignment).
#[repr(C, align(64))]
struct Dcbaa([u64; MAX_SLOTS + 1]);
#[repr(C, align(64))]
struct Ring([Trb; CMD_RING_LEN]);
#[repr(C, align(64))]
struct Erst([u64; 2]);

/// All the controller-level DMA structures, one page-aligned block.
#[repr(C, align(4096))]
struct XhciDma {
    /// Device Context Base Address Array (index 0 = scratchpad array ptr).
    dcbaa: Dcbaa,
    cmd_ring: Ring,
    evt_ring: Ring,
    /// One Event Ring Segment Table entry: base (u64) + (size | rsvd<<32).
    erst: Erst,
}

#[link_section = ".dma"]
static mut DMA: XhciDma = XhciDma {
    dcbaa: Dcbaa([0; MAX_SLOTS + 1]),
    cmd_ring: Ring([Trb([0; 4]); CMD_RING_LEN]),
    evt_ring: Ring([Trb([0; 4]); EVT_RING_LEN]),
    erst: Erst([0; 2]),
};

/// How many HID devices we support concurrently (keyboard + mouse).
// Max device slots we track. Counts *every* enumerated device, not just HID:
// on the real Pi-4 the keyboard/mouse sit behind the VL805 internal USB2 hub
// (and possibly a second hub in the keyboard), so a slot is spent on each hub
// in the path plus each HID endpoint.
pub const MAX_HID: usize = 8;

// Per-device DMA. The Input and Device contexts are 32-byte contexts (CSZ=0;
// checked in `open`): the Input is control + slot + 31 endpoint contexts
// (33 × 8 u32), the Device is slot + 31 endpoints (32 × 8 u32). Laid out as
// `[u32]` so we can view the region as `context::Input32Byte` (a `#[repr(C)]`
// struct of the same u32s). One per device — the controller reads each
// device's contexts continuously, so they cannot be shared.
#[repr(C, align(64))]
struct DevDma {
    input: [u32; 33 * 8],
    device: [u32; 32 * 8],
    ep0_ring: Ring,
    int_ring: Ring,
    buf: [u8; 256],
    intbuf: [u8; 8],
}

const DEV_INIT: DevDma = DevDma {
    input: [0; 33 * 8],
    device: [0; 32 * 8],
    ep0_ring: Ring([Trb([0; 4]); CMD_RING_LEN]),
    int_ring: Ring([Trb([0; 4]); CMD_RING_LEN]),
    buf: [0; 256],
    intbuf: [0; 8],
};

#[link_section = ".dma"]
static mut DEV: [DevDma; MAX_HID] = [DEV_INIT; MAX_HID];

// Scratchpad buffers: many controllers (the real Pi-4 VL805 does; QEMU does
// not) require software to provide N page-sized scratch buffers, advertised in
// HCSPARAMS2. DCBAA[0] points at an array of their addresses. If we leave
// DCBAA[0] = 0 when the controller wants scratchpads, it DMA-faults (HSE) on
// its first command. Both the array and the buffers are page-aligned.
const MAX_SCRATCH: usize = 64;
const SCRATCH_PAGE: usize = 4096;
#[repr(C, align(4096))]
struct ScratchArray([u64; MAX_SCRATCH]);
#[repr(C, align(4096))]
struct ScratchBufs([[u8; SCRATCH_PAGE]; MAX_SCRATCH]);
#[link_section = ".dma"]
static mut SCRATCH_ARRAY: ScratchArray = ScratchArray([0; MAX_SCRATCH]);
#[link_section = ".dma"]
static mut SCRATCH_BUFS: ScratchBufs = ScratchBufs([[0; SCRATCH_PAGE]; MAX_SCRATCH]);

/// Per-device runtime state (slot, the interrupt endpoint, ring producers).
#[derive(Clone, Copy)]
struct HidDev {
    slot: u8,
    dci: u8,
    /// 1 = keyboard, 2 = mouse, 9 = hub (not polled), 0 = unused.
    protocol: u8,
    /// Root-hub port (1-based) the device's path starts at + descriptor IDs.
    port: u8,
    vid: u16,
    pid: u16,
    /// `bDeviceClass` from the device descriptor (9 = hub).
    class: u8,
    /// xHCI route string: the downstream-port path through the hub tiers
    /// (0 for a device on a root port).
    route: u32,
    /// PORTSC speed id (1 full, 2 low, 3 high, 4 super).
    speed: u8,
    /// Transaction-Translator hub for a full/low-speed device behind a
    /// high-speed hub: the hub's slot id + the downstream port (0 = none).
    tt_hub_slot: u8,
    tt_port: u8,
    ep0_enq: usize,
    ep0_cycle: bool,
    int_enq: usize,
    int_cycle: bool,
}

impl HidDev {
    const fn new() -> Self {
        Self {
            slot: 0,
            dci: 0,
            protocol: 0,
            port: 0,
            vid: 0,
            pid: 0,
            class: 0,
            route: 0,
            speed: 0,
            tt_hub_slot: 0,
            tt_port: 0,
            ep0_enq: 0,
            ep0_cycle: true,
            int_enq: 0,
            int_cycle: true,
        }
    }
}

/// A discovered USB device, for display (the init-screen USB tree).
#[derive(Clone, Copy, Default)]
pub struct DeviceInfo {
    pub port: u8,
    /// 1 = keyboard, 2 = mouse.
    pub protocol: u8,
    pub vid: u16,
    pub pid: u16,
}

/// A HID boot device's configuration, as parsed from its config descriptor.
#[derive(Clone, Copy)]
pub struct HidInfo {
    /// `bConfigurationValue` for SET_CONFIGURATION.
    pub config_value: u8,
    /// The HID interface number.
    pub iface: u8,
    /// `bInterfaceProtocol`: 1 = keyboard, 2 = mouse.
    pub protocol: u8,
    /// Interrupt-IN endpoint address (bit 7 = IN, low 4 = number).
    pub ep_addr: u8,
    /// Endpoint max packet size.
    pub ep_max: u16,
    /// Endpoint polling interval (`bInterval`).
    pub interval: u8,
}

/// Default EP0 max packet size for a port speed (PORTSC speed id).
fn ep0_max_packet(speed: u8) -> u16 {
    match speed {
        4 => 512, // SuperSpeed
        3 => 64,  // High
        _ => 8,   // Full / Low
    }
}

/// Push a TRB onto a producer ring: set the producer cycle bit, and on
/// reaching the last slot write a Link TRB back to the ring base (Toggle
/// Cycle) so the ring is circular.
fn ring_push(ring: &mut [Trb], enq: &mut usize, cycle: &mut bool, mut data: [u32; 4]) {
    let len = ring.len();
    if *cycle {
        data[3] |= 1;
    } else {
        data[3] &= !1;
    }
    ring[*enq] = Trb(data);
    *enq += 1;
    if *enq == len - 1 {
        let base = dma_addr(ring.as_ptr() as u64);
        let mut link = [base as u32, (base >> 32) as u32, 0, (6u32 << 10) | (1 << 1)];
        if *cycle {
            link[3] |= 1;
        } else {
            link[3] &= !1;
        }
        ring[len - 1] = Trb(link);
        *enq = 0;
        *cycle = !*cycle;
    }
}

// ── DMA ordering barriers (real Pi-4: PCIe DMA is not cache-coherent) ─────
//
// The `.dma` block is mapped **Device-nGnRnE** (see `usb::map_dma_coherent`),
// so the controller and the CPU already share one un-cached, strongly-ordered
// view of RAM — no cache clean/invalidate is needed (and `dc cvac`/`dc ivac`
// on Device memory would be meaningless). What remains is *ordering*: a full
// barrier so our DMA-struct writes complete before we ring a doorbell, and so
// we observe the controller's write-backs before we read them. `dsb sy` also
// orders against any Normal memory the arguments alias. QEMU is coherent, so
// these compile out there.
#[inline(always)]
fn dcache_clean(_va: u64, _len: usize) {
    #[cfg(feature = "board-rpi4")]
    // SAFETY: a plain ordering barrier; no memory is dereferenced.
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}
#[inline(always)]
fn dcache_inval(_va: u64, _len: usize) {
    #[cfg(feature = "board-rpi4")]
    // SAFETY: a plain ordering barrier; no memory is dereferenced.
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

// The `[u32; 33*8]` DMA buffer we cast to `Input32Byte` must be exactly its
// size, or the crate's slot/endpoint setters would land at the wrong stride
// (e.g. a 64-byte layout writing Slot@[16]/EP0@[32] instead of [8]/[16]).
// Verified at compile time: Input32Byte = control(32B) + slot(32B) +
// 31×endpoint(32B) = 1056 B = 33*8 u32. If this ever fails to build, the crate
// changed its 32-byte context stride and every `input`/`device` offset is off.
const _: () = assert!(core::mem::size_of::<context::Input32Byte>() == 33 * 8 * 4);

pub struct Xhci {
    regs: Registers<HighHalf>,
    ports: u8,
    /// Command-ring producer index + cycle state.
    cmd_enq: usize,
    cmd_cycle: bool,
    /// Event-ring consumer index + cycle state.
    evt_deq: usize,
    evt_cycle: bool,
    /// MMIO VA of the ERDP register's high 32 bits, re-written explicitly each
    /// event because the BCM2711 PCIe mangles the crate's 64-bit ERDP store.
    erdp_hi: u64,
    /// Per-device state (EP0/interrupt ring producers, slot, endpoint).
    hid: [HidDev; MAX_HID],
    /// Number of configured HID devices.
    n_hid: usize,
}

impl Xhci {
    /// Bring the controller at `mmio_base` (physical) up: reset → max slots
    /// → DCBAA → command ring → event ring → run.
    ///
    /// # Safety
    ///
    /// `mmio_base` must be the controller's MMIO base; single-CPU boot;
    /// `DMA` is exclusively ours.
    pub unsafe fn open(mmio_base: u64) -> Self {
        // SAFETY: caller contract — valid MMIO base, mapped high-half.
        let mut regs = unsafe { Registers::new(mmio_base as usize, HighHalf) };

        // Wait for ready, then reset.
        while regs.operational.usbsts.read_volatile().controller_not_ready() {
            core::hint::spin_loop();
        }
        regs.operational.usbcmd.update_volatile(|c| {
            c.set_host_controller_reset();
        });
        while regs.operational.usbcmd.read_volatile().host_controller_reset() {
            core::hint::spin_loop();
        }
        while regs.operational.usbsts.read_volatile().controller_not_ready() {
            core::hint::spin_loop();
        }

        let hcs1 = regs.capability.hcsparams1.read_volatile();
        let max_slots = hcs1.number_of_device_slots();
        let max_ports = hcs1.number_of_ports();
        let slots = max_slots.min(MAX_SLOTS as u8);
        regs.operational.config.update_volatile(|c| {
            c.set_max_device_slots_enabled(slots);
        });

        // SAFETY: single-CPU boot; DMA is BSS we own exclusively.
        let dma = unsafe { &mut *(&raw mut DMA) };

        let dcbaa_pa = dma_addr((&raw const dma.dcbaa) as u64);
        regs.operational.dcbaap.update_volatile(|d| d.set(dcbaa_pa));

        let cmd_pa = dma_addr((&raw const dma.cmd_ring) as u64);
        regs.operational.crcr.update_volatile(|c| {
            c.set_command_ring_pointer(cmd_pa);
            // Ring Cycle State = 1: the controller's consumer cycle state must
            // match our producer cycle (our first command TRB has cycle = 1).
            // The crate's pointer setter leaves RCS at 0; QEMU tolerates the
            // mismatch, the real VL805 then never processes the command.
            c.set_ring_cycle_state();
        });

        // Event ring: ERST[0] = { segment base, segment size }.
        let evt_pa = dma_addr((&raw const dma.evt_ring) as u64);
        dma.erst.0[0] = evt_pa;
        dma.erst.0[1] = EVT_RING_LEN as u64;
        let erst_pa = dma_addr((&raw const dma.erst) as u64);
        {
            let mut intr = regs.interrupter_register_set.interrupter_mut(0);
            intr.erstsz.update_volatile(|s| s.set(1));
            intr.erdp.update_volatile(|d| d.set_event_ring_dequeue_pointer(evt_pa));
            intr.erstba.update_volatile(|b| b.set(erst_pa));
        }

        // The BCM2711 PCIe MMIO mishandles a single 64-bit store to the 64-bit
        // xHCI pointer registers: the high 32 bits come back as the low value
        // (verified by readback). So after the crate's 64-bit writes set the
        // (correct) low halves, re-write each register's HIGH 32 bits with an
        // explicit 32-bit store. Harmless on QEMU (addresses < 4 GiB → HI = 0).
        // Offsets (xHCI spec): operational = cap + CAPLENGTH; runtime = cap +
        // RTSOFF; interrupter 0 = runtime + 0x20. CRCR 0x18, DCBAAP 0x30 in the
        // operational set; ERSTBA +0x10, ERDP +0x18 in the interrupter set.
        let erdp_hi = unsafe {
            let cap = mmio_va(mmio_base as usize) as u64;
            let caplength = core::ptr::read_volatile(cap as *const u8) as u64;
            let op = cap + caplength;
            let rtsoff = (core::ptr::read_volatile((cap + 0x18) as *const u32) & !0x1f) as u64;
            let intr0 = cap + rtsoff + 0x20;
            let wr_hi = |reg_lo: u64, addr: u64| {
                core::ptr::write_volatile((reg_lo + 4) as *mut u32, (addr >> 32) as u32);
            };
            wr_hi(op + 0x30, dcbaa_pa); // DCBAAP
            wr_hi(op + 0x18, cmd_pa); // CRCR
            wr_hi(intr0 + 0x10, erst_pa); // ERSTBA
            wr_hi(intr0 + 0x18, evt_pa); // ERDP
            intr0 + 0x1c // ERDP-high register VA — poll_event re-writes it per event
        };

        // Scratchpad buffers (HCSPARAMS2): the controller may require N page
        // buffers for its own use, reached through DCBAA[0] → a scratchpad
        // array of their addresses. The real Pi-4 VL805 needs them (else it
        // HSE-DMA-faults on the first command); QEMU reports 0.
        let n_scratch =
            regs.capability.hcsparams2.read_volatile().max_scratchpad_buffers() as usize;
        if n_scratch > 0 {
            let n = n_scratch.min(MAX_SCRATCH);
            // SAFETY: single-CPU boot; the scratch statics are ours.
            let arr = unsafe { &mut *(&raw mut SCRATCH_ARRAY) };
            for i in 0..n {
                let buf = unsafe { (&raw const (*(&raw const SCRATCH_BUFS)).0[i]) as u64 };
                arr.0[i] = dma_addr(buf);
            }
            dma.dcbaa.0[0] = dma_addr((&raw const SCRATCH_ARRAY) as u64);
            dcache_clean(
                core::ptr::addr_of!(SCRATCH_ARRAY) as u64,
                core::mem::size_of::<ScratchArray>(),
            );
        }
        kprintln!("[usb] scratchpad buffers needed: {}", n_scratch);
        kprintln!(
            "[usb] dma addrs: dcbaap={:#x} crcr={:#x} erstba={:#x} dcbaa0={:#x}",
            dcbaa_pa,
            cmd_pa,
            erst_pa,
            dma.dcbaa.0[0]
        );
        // Read DCBAAP back from the controller: does the 64-bit register hold
        // the full 35-bit address (incl. the 0x4_… bus offset), or did the
        // BCM2711 MMIO drop the high half? Also AC64 (64-bit addressing cap).
        let cap1 = regs.capability.hccparams1.read_volatile();
        // Raw 32-bit read of HCCPARAMS1 (CAPBASE+0x10), bypassing the crate, so
        // CSZ (bit 2) is verified independent of any register-read mangling.
        let hccp1_raw =
            unsafe { core::ptr::read_volatile((mmio_va(mmio_base as usize) + 0x10) as *const u32) };
        kprintln!(
            "[usb] reg readback: ac64={} csz={} dcbaap_hw={:#x} | HCCPARAMS1_raw={:#010x} csz_raw={}",
            cap1.addressing_capability() as u8,
            cap1.context_size() as u8, // 1 => 64-byte contexts; we build 32-byte!
            regs.operational.dcbaap.read_volatile().get(),
            hccp1_raw,
            (hccp1_raw >> 2) & 1,
        );

        // Run.
        // Make the DCBAA, command ring and ERST visible to a non-coherent
        // controller (real Pi-4) before it starts; no-op on QEMU.
        dcache_clean(core::ptr::addr_of!(DMA) as u64, core::mem::size_of::<XhciDma>());
        dcache_clean(
            core::ptr::addr_of!(DEV) as u64,
            core::mem::size_of::<[DevDma; MAX_HID]>(),
        );
        regs.operational.usbcmd.update_volatile(|c| {
            c.set_run_stop();
        });
        while regs.operational.usbsts.read_volatile().hc_halted() {
            core::hint::spin_loop();
        }

        kprintln!("[usb] xHCI up: {} slots enabled, {} root ports", slots, max_ports);

        // Power on every root port (PORTSC.PP, bit 9). The real Pi-4 VL805 does
        // not power its ports by default — a device gets no power (LEDs dark,
        // never reports a connect) until software sets PP; QEMU powers them
        // implicitly (HCCPARAMS.PPC tells us which, but setting PP is harmless
        // when ports are always-powered). Then wait for power to stabilise
        // (USB2 PWRON2PWRGOOD ≈ 20 ms) before the connect scan / enumeration.
        let ppc = regs.capability.hccparams1.read_volatile().port_power_control();
        for i in 0..max_ports as usize {
            regs.port_register_set.update_volatile_at(i, |p| {
                p.portsc.set_port_power();
            });
        }
        kprintln!("[usb] root ports powered (PPC={})", ppc);
        crate::arch::aarch64::delay_ms(20);

        Xhci {
            regs,
            ports: max_ports,
            cmd_enq: 0,
            cmd_cycle: true,
            evt_deq: 0,
            evt_cycle: true,
            erdp_hi,
            hid: [HidDev::new(); MAX_HID],
            n_hid: 0,
        }
    }

    /// Queue an interrupt-IN transfer (8-byte HID boot report) on the device's
    /// interrupt endpoint and ring its doorbell. The completion arrives later
    /// as a Transfer Event (drained by [`Self::poll_interrupt`]).
    pub fn queue_interrupt(&mut self, di: usize) {
        let slot = self.hid[di].slot;
        let dci = self.hid[di].dci;
        // SAFETY: single-CPU; DEV is ours.
        let buf_pa = unsafe { dma_addr((&raw const (*(&raw const DEV))[di].intbuf) as u64) };
        let mut n = transfer::Normal::new();
        n.set_data_buffer_pointer(buf_pa)
            .set_trb_transfer_length(8)
            .set_interrupt_on_completion();
        let data = transfer::Allowed::Normal(n).into_raw();
        // SAFETY: single-CPU; DEV is ours.
        let ring = unsafe { &mut (*(&raw mut DEV))[di].int_ring.0 };
        ring_push(ring, &mut self.hid[di].int_enq, &mut self.hid[di].int_cycle, data);
        self.dma_clean();
        self.regs.doorbell.update_volatile_at(slot as usize, |db| {
            db.set_doorbell_target(dci);
        });
    }

    /// Number of configured HID devices, and a device's protocol.
    pub fn n_hid(&self) -> usize {
        self.n_hid
    }
    pub fn protocol(&self, di: usize) -> u8 {
        self.hid[di].protocol
    }

    /// Write back the whole DMA region (rings, DCBAA, contexts, buffers) so a
    /// non-coherent controller (real Pi-4) reads our latest writes. Called
    /// before every doorbell; a no-op on coherent QEMU.
    #[inline]
    fn dma_clean(&self) {
        dcache_clean(core::ptr::addr_of!(DMA) as u64, core::mem::size_of::<XhciDma>());
        dcache_clean(
            core::ptr::addr_of!(DEV) as u64,
            core::mem::size_of::<[DevDma; MAX_HID]>(),
        );
    }

    /// Drain one event; if it is a Transfer Event (a HID report landed),
    /// return the device index + a copy of its 8-byte report buffer.
    pub fn poll_interrupt(&mut self) -> Option<(usize, [u8; 8])> {
        if let Some(event::Allowed::TransferEvent(te)) = self.poll_event() {
            let slot = te.slot_id();
            for di in 0..self.n_hid {
                if self.hid[di].slot == slot {
                    // SAFETY: single-CPU; DEV is ours.
                    let p = unsafe { (&raw const (*(&raw const DEV))[di].intbuf) as u64 };
                    dcache_inval(p, 8); // the controller DMA'd the HID report
                    return Some((di, unsafe { (*(&raw const DEV))[di].intbuf }));
                }
            }
        }
        None
    }

    /// Enqueue a command TRB, ring the command doorbell, and wait for its
    /// Command Completion event. Returns the completion (slot id + code).
    ///
    /// D2 issues only a handful of commands, far fewer than `CMD_RING_LEN`,
    /// so the ring does not wrap during enumeration — a Link TRB / wrap is a
    /// later refinement.
    fn command(&mut self, mut data: [u32; 4]) -> Option<event::CommandCompletion> {
        // SAFETY: single-CPU; DMA is ours.
        let dma = unsafe { &mut *(&raw mut DMA) };
        if self.cmd_cycle {
            data[3] |= 1;
        } else {
            data[3] &= !1;
        }
        dma.cmd_ring.0[self.cmd_enq] = Trb(data);
        self.cmd_enq += 1;
        if self.cmd_enq >= CMD_RING_LEN - 1 {
            self.cmd_enq = 0;
        }
        // Ring doorbell 0 (the command ring).
        self.dma_clean();
        self.regs.doorbell.update_volatile_at(0, |d| {
            d.set_doorbell_target(0);
        });
        for _ in 0..200_000_000u64 {
            // poll_event also consumes interleaved Port Status Change events
            // (the controller queues them on the same ring); we only act on the
            // command's completion.
            if let Some(event::Allowed::CommandCompletion(cc)) = self.poll_event() {
                return Some(cc);
            }
            core::hint::spin_loop();
        }
        // Timed out — dump state to localise the DMA round-trip failure. evt0
        // tells us whether the controller wrote *any* completion event (DMA
        // write reached RAM) or nothing (it never read our command TRB).
        let sts = self.regs.operational.usbsts.read_volatile();
        // SAFETY: single-CPU; DMA is ours.
        let dma = unsafe { &*(&raw const DMA) };
        dcache_inval((&raw const dma.evt_ring.0[0]) as u64, 64);
        let e = dma.evt_ring.0[0].0;
        kprintln!(
            "[usb] cmd timeout: hch={} hse={} hce={} cnr={} evt0={:08x} {:08x} {:08x} {:08x}",
            sts.hc_halted(),
            sts.host_system_error(),
            sts.host_controller_error(),
            sts.controller_not_ready(),
            e[0],
            e[1],
            e[2],
            e[3]
        );
        None
    }

    /// Consume one event from the event ring if its cycle bit is current,
    /// advancing the dequeue pointer + ERDP. `None` if no new event.
    fn poll_event(&mut self) -> Option<event::Allowed> {
        // SAFETY: single-CPU; DMA is ours.
        let dma = unsafe { &mut *(&raw mut DMA) };
        // Drop the stale cached copy of this event TRB so we see the
        // controller's write (real Pi-4; no-op on QEMU). One cache line.
        dcache_inval((&raw const dma.evt_ring.0[self.evt_deq]) as u64, 16);
        let data = dma.evt_ring.0[self.evt_deq].0;
        if ((data[3] & 1) != 0) != self.evt_cycle {
            return None; // not yet written by the controller
        }
        let ev = event::Allowed::try_from(data).ok();
        self.evt_deq += 1;
        if self.evt_deq >= EVT_RING_LEN {
            self.evt_deq = 0;
            self.evt_cycle = !self.evt_cycle;
        }
        let erdp = dma_addr((&raw const dma.evt_ring.0[self.evt_deq]) as u64);
        self.regs
            .interrupter_register_set
            .interrupter_mut(0)
            .erdp
            .update_volatile(|d| {
                d.set_event_ring_dequeue_pointer(erdp);
            });
        // The crate's 64-bit ERDP store loses its high half on the BCM2711 (see
        // open); re-write it so the controller keeps a valid dequeue pointer and
        // keeps delivering events. Harmless on QEMU (erdp < 4 GiB → high = 0).
        unsafe { core::ptr::write_volatile(self.erdp_hi as *mut u32, (erdp >> 32) as u32) };
        ev
    }

    /// Enable Slot command → a device slot ID.
    pub fn enable_slot(&mut self) -> Option<u8> {
        let data = command::Allowed::EnableSlot(command::EnableSlot::new()).into_raw();
        let cc = self.command(data)?;
        Some(cc.slot_id())
    }

    /// Reset a root-hub port (1-based) and wait for it to enable.
    fn port_reset(&mut self, port: u8) -> bool {
        let i = (port - 1) as usize;
        self.regs.port_register_set.update_volatile_at(i, |p| {
            p.portsc.set_port_reset();
        });
        for _ in 0..20_000_000u64 {
            let p = self.regs.port_register_set.read_volatile_at(i).portsc;
            if !p.port_reset() && p.port_enabled_disabled() {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    fn port_speed(&self, port: u8) -> u8 {
        self.regs
            .port_register_set
            .read_volatile_at((port - 1) as usize)
            .portsc
            .port_speed()
    }

    /// Address a device on `port` (speed `speed`) to `slot`: build the Input
    /// Context (slot + EP0), point DCBAA at the Device Context, and issue the
    /// Address Device command. Returns true on Success.
    fn address_device(&mut self, di: usize, port: u8, speed: u8) -> bool {
        let slot = self.hid[di].slot;
        // SAFETY: single-CPU; DEV/DMA are ours.
        let dev = unsafe { &mut (*(&raw mut DEV))[di] };
        dev.input.iter_mut().for_each(|x| *x = 0);
        dev.device.iter_mut().for_each(|x| *x = 0);

        let ep0_pa = dma_addr((&raw const dev.ep0_ring) as u64); // 64-byte aligned

        // View the zeroed input region as a 32-byte Input Context and fill it.
        // SAFETY: `Input32Byte` is `#[repr(C)]` over the same u32s as `input`.
        let input: &mut context::Input32Byte =
            unsafe { &mut *(dev.input.as_mut_ptr().cast()) };
        input.control_mut().set_add_context_flag(0); // slot
        input.control_mut().set_add_context_flag(1); // EP0
        let (route, tt_slot, tt_port) =
            (self.hid[di].route, self.hid[di].tt_hub_slot, self.hid[di].tt_port);
        {
            let s = input.device_mut().slot_mut();
            s.set_context_entries(1);
            s.set_root_hub_port_number(port);
            s.set_speed(speed);
            s.set_route_string(route);
            // A full/low-speed device behind a high-speed hub reaches the bus
            // through that hub's Transaction Translator.
            if tt_slot != 0 {
                s.set_parent_hub_slot_id(tt_slot);
                s.set_parent_port_number(tt_port);
            }
        }
        {
            let e = input.device_mut().endpoint_mut(1);
            e.set_endpoint_type(context::EndpointType::Control);
            e.set_max_packet_size(ep0_max_packet(speed));
            e.set_tr_dequeue_pointer(ep0_pa);
            e.set_dequeue_cycle_state(); // DCS = 1, matches the ring's PCS
            e.set_error_count(3);
            // Average TRB Length must be non-zero (circle sets 8); an EP0 with
            // ATL=0 is a malformed Endpoint Context that Address Device can
            // "Success" while leaving the slot in a bad state for later commands.
            e.set_average_trb_length(8);
        }

        // DIAGNOSTIC: dump the raw Input Context our cast produced. Expect
        // ctrl add-flags at [1]=0x3 (A0|A1), the slot context at [8..16]
        // (dw0 ~0x08300000: entries=1, speed=3), EP0 at [16..24] (dw2 =
        // tr_dequeue|DCS). If the slot values land at the wrong offset the
        // `[u32]` → Input32Byte cast (non-repr(C) crate structs) is the bug.
        unsafe {
            let inp = &(*(&raw const DEV))[di].input;
            kprintln!(
                "[usb] addr_dev input: ctrl[1]={:08x} slot[8..12]={:08x} {:08x} {:08x} {:08x} ep0[16..20]={:08x} {:08x} {:08x} {:08x}",
                inp[1], inp[8], inp[9], inp[10], inp[11], inp[16], inp[17], inp[18], inp[19]
            );
        }

        // DCBAA[slot] → the (controller-written) Device Context.
        let dma = unsafe { &mut *(&raw mut DMA) };
        dma.dcbaa.0[slot as usize] = dma_addr((&raw const dev.device) as u64);

        let input_pa = dma_addr((&raw const dev.input) as u64);
        // DIAGNOSTIC: does the Input Context Pointer point at input[0] (correct)
        // or accidentally input[8] (would put Slot where EP0 is)?
        unsafe {
            kprintln!(
                "[usb] addr_dev input_pa={:#x} &input[0]={:#x} &input[8]={:#x}",
                input_pa,
                dma_addr((&raw const (*(&raw const DEV))[di].input[0]) as u64),
                dma_addr((&raw const (*(&raw const DEV))[di].input[8]) as u64),
            );
        }
        let mut cmd = command::AddressDevice::new();
        cmd.set_input_context_pointer(input_pa);
        cmd.set_slot_id(slot);
        // BSR (Block Set Address Request) MUST be 0 so the controller performs
        // SET_ADDRESS and moves the slot Default → Addressed. new() zeroes it,
        // but clear it explicitly so a slot left in Default (EP0 works, but
        // Configure Endpoint later fails with Context State Error) is ruled out.
        cmd.clear_block_set_address_request();
        let data = command::Allowed::AddressDevice(cmd).into_raw();
        match self.command(data) {
            Some(cc) => {
                let ok = matches!(cc.completion_code(), Ok(event::CompletionCode::Success));
                // Full Device Context dump (CSZ=0 → 32-byte): slot ctx dw0..7 +
                // EP0 ctx dw0..7. If EP0 (dw8..15) holds our tr_dequeue + a
                // 64-byte max-packet, the controller writes the DC correctly and
                // only the slot state is off; if EP0 is garbage too, the VL805
                // does not mirror the DC to RAM at all.
                unsafe {
                    dcache_inval((&raw const (*(&raw const DEV))[di].device) as u64, 64);
                    let d = &(*(&raw const DEV))[di].device;
                    kprintln!(
                        "[usb] address_device slot {} → {:?}; slot[0..8]={:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}",
                        slot, cc.completion_code(),
                        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
                    );
                    kprintln!(
                        "[usb]   ep0[8..16]={:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} (expect tr_dequeue={:#x})",
                        d[8], d[9], d[10], d[11], d[12], d[13], d[14], d[15], ep0_pa
                    );
                }
                if ok {
                    // The VL805 needs settling time after Address Device before
                    // the slot is fully usable for later commands — circle does
                    // CTimer::MsDelay(50) here.
                    crate::arch::aarch64::delay_ms(50);
                }
                ok
            }
            None => {
                kprintln!("[usb] address_device: no completion");
                false
            }
        }
    }

    /// Push a TRB onto the EP0 transfer ring.
    fn ep0_push(&mut self, di: usize, data: [u32; 4]) {
        // SAFETY: single-CPU; DEV is ours.
        let ring = unsafe { &mut (*(&raw mut DEV))[di].ep0_ring.0 };
        ring_push(ring, &mut self.hid[di].ep0_enq, &mut self.hid[di].ep0_cycle, data);
    }

    /// Wait for a Transfer Event; true on Success.
    fn wait_transfer(&mut self) -> bool {
        for _ in 0..50_000_000u64 {
            if let Some(event::Allowed::TransferEvent(te)) = self.poll_event() {
                return matches!(te.completion_code(), Ok(event::CompletionCode::Success));
            }
            core::hint::spin_loop();
        }
        false
    }

    /// A control transfer on EP0. `setup` = the 8-byte SETUP packet; a data
    /// stage of `len` bytes at `buf_pa` follows when `len > 0` (direction
    /// `dir_in`); `len == 0` = no data stage.
    fn control(&mut self, di: usize, setup: [u8; 8], buf_pa: u64, len: u16, dir_in: bool) -> bool {
        let mut s = transfer::SetupStage::new();
        s.set_request_type(setup[0])
            .set_request(setup[1])
            .set_value(u16::from_le_bytes([setup[2], setup[3]]))
            .set_index(u16::from_le_bytes([setup[4], setup[5]]))
            .set_length(len)
            .set_transfer_type(if len == 0 {
                transfer::TransferType::No
            } else if dir_in {
                transfer::TransferType::In
            } else {
                transfer::TransferType::Out
            });
        self.ep0_push(di, transfer::Allowed::SetupStage(s).into_raw());
        if len > 0 {
            let mut d = transfer::DataStage::new();
            d.set_data_buffer_pointer(buf_pa)
                .set_trb_transfer_length(len as u32)
                .set_direction(if dir_in {
                    transfer::Direction::In
                } else {
                    transfer::Direction::Out
                });
            self.ep0_push(di, transfer::Allowed::DataStage(d).into_raw());
        }
        let mut st = transfer::StatusStage::new();
        // Status stage runs opposite the data stage. `set_direction()` is a
        // flag = IN; the default (no call) = OUT. So set IN unless this is an
        // IN data transfer (whose status stage is OUT).
        if !(len > 0 && dir_in) {
            st.set_direction();
        }
        st.set_interrupt_on_completion();
        self.ep0_push(di, transfer::Allowed::StatusStage(st).into_raw());

        let slot = self.hid[di].slot;
        self.dma_clean();
        self.regs.doorbell.update_volatile_at(slot as usize, |db| {
            db.set_doorbell_target(1);
        });
        let ok = self.wait_transfer();
        // The controller DMA'd the IN data into our buffer — drop the stale
        // cached copy so the caller reads it (real Pi-4; no-op on QEMU).
        if dir_in && len > 0 {
            // SAFETY: single-CPU; DEV is ours.
            let dev = unsafe { &*(&raw const DEV) };
            dcache_inval((&raw const dev[di].buf) as u64, dev[di].buf.len());
        }
        ok
    }

    /// GET_DESCRIPTOR into `DEV[di].buf` (data there on success).
    fn get_descriptor(&mut self, di: usize, desc_type: u8, index: u8, len: u16) -> bool {
        // SAFETY: single-CPU; DEV is ours.
        let dev = unsafe { &mut (*(&raw mut DEV))[di] };
        dev.buf.iter_mut().for_each(|b| *b = 0);
        let buf_pa = dma_addr((&raw const dev.buf) as u64);
        let value = ((desc_type as u16) << 8) | index as u16;
        let setup = [
            0x80,
            6,
            value as u8,
            (value >> 8) as u8,
            0,
            0,
            len as u8,
            (len >> 8) as u8,
        ];
        self.control(di, setup, buf_pa, len, true)
    }

    /// Read + log the 18-byte device descriptor.
    pub fn get_device_descriptor(&mut self, di: usize) -> bool {
        if !self.get_descriptor(di, 1, 0, 18) {
            kprintln!("[usb] get device descriptor failed");
            return false;
        }
        // SAFETY: single-CPU; DEV is ours.
        let b = unsafe { &(*(&raw const DEV))[di].buf };
        kprintln!(
            "[usb] device descriptor: bLength={} type={} bcdUSB={:02x}{:02x} class={} maxpkt0={} VID={:02x}{:02x} PID={:02x}{:02x}",
            b[0], b[1], b[3], b[2], b[4], b[7], b[9], b[8], b[11], b[10]
        );
        self.hid[di].class = b[4];
        self.hid[di].vid = u16::from_le_bytes([b[8], b[9]]);
        self.hid[di].pid = u16::from_le_bytes([b[10], b[11]]);
        true
    }

    /// Discovered-device info for display (the init-screen USB tree).
    pub fn device_info(&self, di: usize) -> DeviceInfo {
        let d = &self.hid[di];
        DeviceInfo {
            port: d.port,
            protocol: d.protocol,
            vid: d.vid,
            pid: d.pid,
        }
    }

    /// Number of root-hub ports the controller exposes (for the tree header).
    pub fn root_ports(&self) -> u8 {
        self.ports
    }

    /// Read the configuration descriptor and find the first HID boot
    /// interrupt-IN endpoint (keyboard/mouse). `None` if not a HID boot device.
    pub fn read_config(&mut self, di: usize) -> Option<HidInfo> {
        // Read the 9-byte config header first for wTotalLength.
        if !self.get_descriptor(di, 2, 0, 9) {
            return None;
        }
        // SAFETY: single-CPU; DEV is ours.
        let total = {
            let b = unsafe { &(*(&raw const DEV))[di].buf };
            u16::from_le_bytes([b[2], b[3]]).min(255)
        };
        if !self.get_descriptor(di, 2, 0, total) {
            return None;
        }
        // SAFETY: single-CPU; DEV is ours. Take the field ref, then slice.
        let buf = unsafe { &(*(&raw const DEV))[di].buf };
        let b = &buf[..total as usize];
        let config_value = b[5];
        let mut iface = 0u8;
        let mut protocol = 0u8;
        let mut i = 0usize;
        while i + 2 <= b.len() {
            let dlen = b[i] as usize;
            let dtype = b[i + 1];
            if dlen == 0 || i + dlen > b.len() {
                break;
            }
            match dtype {
                4 => {
                    // Interface descriptor: class 3 = HID; protocol 1/2.
                    iface = b[i + 2];
                    protocol = b[i + 7];
                }
                5 => {
                    // Endpoint descriptor: interrupt (attr&3==3) + IN (bit 7).
                    let addr = b[i + 2];
                    let attr = b[i + 3];
                    if attr & 0x3 == 3 && addr & 0x80 != 0 {
                        let ep_max = u16::from_le_bytes([b[i + 4], b[i + 5]]);
                        let interval = b[i + 6];
                        kprintln!(
                            "[usb] HID iface {} protocol {} → int-IN ep {:#04x} max {} interval {}",
                            iface, protocol, addr, ep_max, interval
                        );
                        return Some(HidInfo {
                            config_value,
                            iface,
                            protocol,
                            ep_addr: addr,
                            ep_max,
                            interval,
                        });
                    }
                }
                _ => {}
            }
            i += dlen;
        }
        None
    }

    /// Read the configuration descriptor and return its `bConfigurationValue`
    /// only — the minimal bring-up a hub needs. Unlike [`read_config`] it does
    /// not parse (or log) a HID interface/endpoint, so a hub's status-change
    /// interrupt endpoint is not mislabelled as a HID device.
    fn read_config_value(&mut self, di: usize) -> Option<u8> {
        // The 9-byte config header carries bConfigurationValue at offset 5.
        if !self.get_descriptor(di, 2, 0, 9) {
            return None;
        }
        // SAFETY: single-CPU; DEV is ours.
        Some(unsafe { (*(&raw const DEV))[di].buf[5] })
    }

    /// SET_CONFIGURATION (no-data control OUT).
    fn set_configuration(&mut self, di: usize, value: u8) -> bool {
        self.control(di, [0x00, 9, value, 0, 0, 0, 0, 0], 0, 0, false)
    }

    /// HID SET_PROTOCOL → boot protocol (class control OUT to the interface).
    fn set_protocol_boot(&mut self, di: usize, iface: u8) -> bool {
        self.control(di, [0x21, 0x0B, 0, 0, iface, 0, 0, 0], 0, 0, false)
    }

    /// Configure Endpoint: add the HID interrupt-IN endpoint to the device.
    /// Returns its DCI (Device Context Index) on success, 0 on failure.
    fn configure_endpoint(&mut self, di: usize, _port: u8, _speed: u8, hid: &HidInfo) -> u8 {
        let slot = self.hid[di].slot;
        // SAFETY: single-CPU; DEV is ours.
        let dev = unsafe { &mut (*(&raw mut DEV))[di] };
        dev.input.iter_mut().for_each(|x| *x = 0);
        // Seed the Input Context from the whole live Device Context so the
        // controller-managed slot fields (route, speed, TT, max exit latency,
        // interrupter — set at Address Device) are preserved rather than
        // zeroed. QEMU requires A0 (the slot context) on Configure Endpoint or
        // it returns TRB Error; the real VL805 rejects a *rebuilt-from-scratch*
        // slot context with Context State Error. Copying + A0 satisfies both.
        dcache_inval((&raw const dev.device) as u64, core::mem::size_of_val(&dev.device));
        let n = dev.device.len();
        for k in 0..n {
            dev.input[8 + k] = dev.device[k];
        }
        let ep_num = hid.ep_addr & 0x0F;
        let dci = ep_num * 2 + if hid.ep_addr & 0x80 != 0 { 1 } else { 0 };
        let int_pa = dma_addr((&raw const dev.int_ring) as u64);

        // SAFETY: `Input32Byte` is `#[repr(C)]` over the same u32s as `input`.
        let input: &mut context::Input32Byte =
            unsafe { &mut *(dev.input.as_mut_ptr().cast()) };
        input.control_mut().set_add_context_flag(0); // A0: slot (kept from copy)
        input.control_mut().set_add_context_flag(dci as usize); // A_dci: new EP
        {
            // Only extend Context Entries to cover the new endpoint; the rest of
            // the slot context stays as the controller wrote it.
            input.device_mut().slot_mut().set_context_entries(dci);
        }
        {
            let e = input.device_mut().endpoint_mut(dci as usize);
            e.set_endpoint_type(context::EndpointType::InterruptIn);
            e.set_max_packet_size(hid.ep_max);
            e.set_tr_dequeue_pointer(int_pa);
            e.set_dequeue_cycle_state();
            e.set_error_count(3);
        }

        let input_pa = dma_addr((&raw const dev.input) as u64);
        let mut cmd = command::ConfigureEndpoint::new();
        cmd.set_input_context_pointer(input_pa);
        cmd.set_slot_id(slot);
        let data = command::Allowed::ConfigureEndpoint(cmd).into_raw();
        match self.command(data) {
            Some(cc) if matches!(cc.completion_code(), Ok(event::CompletionCode::Success)) => dci,
            Some(cc) => {
                kprintln!("[usb] configure_endpoint → {:?}", cc.completion_code());
                0
            }
            None => 0,
        }
    }

    /// Bring a HID boot device fully online: read its config, SET_CONFIGURATION,
    /// add its interrupt endpoint, SET_PROTOCOL(boot). Returns `(dci, protocol)`
    /// ready for interrupt polling.
    pub fn configure_hid(&mut self, di: usize, port: u8, speed: u8) -> Option<(u8, u8)> {
        let hid = self.read_config(di)?;
        // xHCI enumeration order (spec 4.3.5): the Configure Endpoint *command*
        // (controller side) precedes the SET_CONFIGURATION *request* (device
        // side). QEMU tolerates the reverse; the real VL805 returns Context
        // State Error if SET_CONFIGURATION ran first.
        let dci = self.configure_endpoint(di, port, speed, &hid);
        if dci == 0 {
            return None;
        }
        if !self.set_configuration(di, hid.config_value) {
            kprintln!("[usb] set_configuration failed");
            return None;
        }
        if !self.set_protocol_boot(di, hid.iface) {
            kprintln!("[usb] set_protocol(boot) failed");
            return None;
        }
        kprintln!(
            "[usb] HID configured: slot {} ep {:#04x} dci {} ({})",
            self.hid[di].slot,
            hid.ep_addr,
            dci,
            if hid.protocol == 1 { "keyboard" } else { "mouse" }
        );
        Some((dci, hid.protocol))
    }

    /// Enumerate every connected root port, recursing through any hubs, and
    /// configure each HID boot device (keyboard + mouse) for interrupt polling.
    /// Returns the number of device slots claimed (hubs included).
    pub fn enumerate_all(&mut self) -> usize {
        for port in 1..=self.ports {
            if self.n_hid >= MAX_HID {
                break;
            }
            let p = self
                .regs
                .port_register_set
                .read_volatile_at((port - 1) as usize)
                .portsc;
            if !p.current_connect_status() {
                continue;
            }
            if !self.port_reset(port) {
                kprintln!("[usb] port {}: reset failed", port);
                continue;
            }
            let speed = self.port_speed(port);
            // A device on a root port: route string 0, no parent TT.
            self.enumerate_device(port, 0, speed, 0, 0, 0);
        }
        self.n_hid
    }

    /// Bring one device online: Enable Slot → Address Device → read its device
    /// descriptor, then either configure it as a HID boot device or, if it is a
    /// hub (class 9), recurse into its downstream ports. `root_port` is the
    /// root-hub port the path starts at; `route` is the xHCI route string;
    /// `tt_slot`/`tt_port` name the Transaction-Translator hub (0 = direct);
    /// `depth` is the device's hub tier (0 = on a root port).
    fn enumerate_device(
        &mut self,
        root_port: u8,
        route: u32,
        speed: u8,
        tt_slot: u8,
        tt_port: u8,
        depth: u8,
    ) {
        if self.n_hid >= MAX_HID {
            return;
        }
        let Some(slot) = self.enable_slot() else {
            kprintln!("[usb] enable slot failed (port {} route {:#x})", root_port, route);
            return;
        };
        // Claim the slot now (the device — a hub especially — persists past this
        // call); the transfer methods read its addressing from self.hid[di].
        let di = self.n_hid;
        self.hid[di] = HidDev::new();
        self.hid[di].slot = slot;
        self.hid[di].port = root_port;
        self.hid[di].route = route;
        self.hid[di].speed = speed;
        self.hid[di].tt_hub_slot = tt_slot;
        self.hid[di].tt_port = tt_port;
        self.n_hid += 1;
        kprintln!(
            "[usb] device slot {} (root port {} route {:#x} speed {})",
            slot, root_port, route, speed
        );
        if !self.address_device(di, root_port, speed) {
            return;
        }
        self.get_device_descriptor(di);
        if self.hid[di].class == 9 {
            self.hid[di].protocol = 9; // hub: tracked, never polled as HID
            self.enumerate_hub(di, root_port, speed, depth);
        } else if let Some((dci, protocol)) = self.configure_hid(di, root_port, speed) {
            self.hid[di].dci = dci;
            self.hid[di].protocol = protocol;
            self.queue_interrupt(di);
        }
    }

    /// Drive a hub: configure it as a hub, power its downstream ports, and
    /// enumerate the device on each connected port (recursively).
    fn enumerate_hub(&mut self, di: usize, root_port: u8, speed: u8, depth: u8) {
        let Some(config_value) = self.read_config_value(di) else {
            kprintln!("[usb] hub: read config failed");
            return;
        };
        let Some(nbr) = self.hub_get_descriptor(di) else {
            kprintln!("[usb] hub: get descriptor failed");
            return;
        };
        // Configure Endpoint (mark slot as a hub) before SET_CONFIGURATION.
        if !self.configure_hub(di, speed, nbr) {
            return;
        }
        if !self.set_configuration(di, config_value) {
            kprintln!("[usb] hub: set_configuration failed");
            return;
        }
        kprintln!("[usb] hub slot {}: {} downstream ports", self.hid[di].slot, nbr);
        // Power every port, then wait the worst-case power-good time.
        for p in 1..=nbr {
            self.hub_set_port_feature(di, p, 8); // PORT_POWER
        }
        crate::arch::aarch64::delay_ms(100);
        let hub_slot = self.hid[di].slot;
        let parent_route = self.hid[di].route;
        let (parent_tt_slot, parent_tt_port) =
            (self.hid[di].tt_hub_slot, self.hid[di].tt_port);
        for p in 1..=nbr {
            if self.n_hid >= MAX_HID {
                break;
            }
            let st = self.hub_get_port_status(di, p);
            let connected = st & 0x1 != 0;
            kprintln!("[usb] hub port {}: status={:#06x} connected={}", p, st & 0xffff, connected);
            if !connected {
                continue;
            }
            self.hub_clear_port_feature(di, p, 16); // C_PORT_CONNECTION
            self.hub_set_port_feature(di, p, 4); // PORT_RESET
            crate::arch::aarch64::delay_ms(60); // reset + recovery
            let st2 = self.hub_get_port_status(di, p);
            self.hub_clear_port_feature(di, p, 20); // C_PORT_RESET
            if st2 & 0x2 == 0 {
                kprintln!("[usb] hub port {}: not enabled after reset (status={:#06x})", p, st2 & 0xffff);
                continue;
            }
            // PORT_LOW_SPEED (bit 9) / PORT_HIGH_SPEED (bit 10); else full.
            let dev_speed = if st2 & (1 << 9) != 0 {
                2
            } else if st2 & (1 << 10) != 0 {
                3
            } else {
                1
            };
            // Append this downstream port to the route string at our tier.
            let route = parent_route | ((p as u32 & 0xf) << (4 * depth));
            // A full/low-speed device behind this high-speed hub uses *our* TT;
            // otherwise inherit whatever TT (if any) we sit behind.
            let (tt_slot, tt_port) = if dev_speed < 3 && speed == 3 {
                (hub_slot, p)
            } else {
                (parent_tt_slot, parent_tt_port)
            };
            self.enumerate_device(root_port, route, dev_speed, tt_slot, tt_port, depth + 1);
        }
    }

    /// Mark a hub's device slot as a Hub via an **Evaluate Context** command
    /// with a **from-scratch** Input Slot Context (A0 only). This mirrors
    /// circle's `CXHCIUSBDevice::EnableHubFunction`
    /// (`references/circle/lib/usb/xhciusbdevice.cpp`), the working reference
    /// for this exact VL805: it sets `Slot.Hub`, `Slot.NumberOfPorts` and (for
    /// a high-speed hub) `Slot.TTT` and issues **Evaluate Context**, not
    /// Configure Endpoint. On this controller Configure Endpoint runs a strict
    /// Context-State precondition against the *output* device context, which the
    /// VL805 never writes coherently (the RAM slot context reads garbage —
    /// slot state 0 — even after a successful Address Device, the
    /// XHCI_EP_CTX_BROKEN_DCS quirk), so it always returned Context State Error.
    /// Evaluate Context does not run that check, so it succeeds. The VL805
    /// applies the Hub / Number-of-Ports / TT fields on Evaluate Context even
    /// though xHCI §4.6.7 nominally restricts it to Max Exit Latency /
    /// Interrupter Target — circle relies on exactly this behaviour.
    ///
    /// The RAM device context is garbage, so — unlike circle, which memcpys it
    /// as the input base — we rebuild the slot context from known-good values
    /// (route 0, the hub's speed + root port, context_entries = 1 for EP0). The
    /// controller keeps EP0 (dci 1) internally, so A0 alone (no endpoint
    /// contexts) is enough.
    fn configure_hub(&mut self, di: usize, speed: u8, nbr_ports: u8) -> bool {
        let slot = self.hid[di].slot;
        let port = self.hid[di].port;
        // SAFETY: single-CPU; DEV is ours.
        let dev = unsafe { &mut (*(&raw mut DEV))[di] };
        dev.input.iter_mut().for_each(|x| *x = 0);
        // SAFETY: `Input32Byte` is `#[repr(C)]` over the same u32s as `input`.
        let input: &mut context::Input32Byte =
            unsafe { &mut *(dev.input.as_mut_ptr().cast()) };
        input.control_mut().set_add_context_flag(0); // A0: slot context
        {
            let s = input.device_mut().slot_mut();
            s.set_context_entries(1);
            s.set_root_hub_port_number(port);
            s.set_speed(speed);
            s.set_hub();
            s.set_number_of_ports(nbr_ports);
            // A high-speed hub provides the Transaction Translator. Single-TT,
            // think time 0 (8 FS bit times) is the safe default.
            if speed == 3 {
                s.set_tt_think_time(0);
            }
        }
        let input_pa = dma_addr((&raw const dev.input) as u64);
        let mut cmd = command::EvaluateContext::new();
        cmd.set_input_context_pointer(input_pa);
        cmd.set_slot_id(slot);
        let data = command::Allowed::EvaluateContext(cmd).into_raw();
        match self.command(data) {
            Some(cc) if matches!(cc.completion_code(), Ok(event::CompletionCode::Success)) => true,
            Some(cc) => {
                kprintln!("[usb] configure_hub → {:?}", cc.completion_code());
                false
            }
            None => {
                kprintln!("[usb] configure_hub: no completion (timeout)");
                false
            }
        }
    }

    /// Hub class GET_DESCRIPTOR (type 0x29). Returns `bNbrPorts`.
    fn hub_get_descriptor(&mut self, di: usize) -> Option<u8> {
        // SAFETY: single-CPU; DEV is ours.
        let buf_pa = unsafe { dma_addr((&raw const (*(&raw const DEV))[di].buf) as u64) };
        // bmRequestType=0xA0 (in, class, device), bRequest=6, wValue=0x2900.
        if !self.control(di, [0xA0, 6, 0x00, 0x29, 0, 0, 0, 0], buf_pa, 8, true) {
            return None;
        }
        // SAFETY: single-CPU; DEV is ours. [2] = bNbrPorts.
        Some(unsafe { (*(&raw const DEV))[di].buf[2] })
    }

    /// Hub class SET_FEATURE on a downstream port (PORT_POWER=8, PORT_RESET=4).
    fn hub_set_port_feature(&mut self, di: usize, port: u8, feature: u8) -> bool {
        // bmRequestType=0x23 (out, class, other), bRequest=3 (SET_FEATURE).
        self.control(di, [0x23, 3, feature, 0, port, 0, 0, 0], 0, 0, false)
    }

    /// Hub class CLEAR_FEATURE on a downstream port (C_PORT_CONNECTION=16, …).
    fn hub_clear_port_feature(&mut self, di: usize, port: u8, feature: u8) -> bool {
        // bmRequestType=0x23 (out, class, other), bRequest=1 (CLEAR_FEATURE).
        self.control(di, [0x23, 1, feature, 0, port, 0, 0, 0], 0, 0, false)
    }

    /// Hub class GET_STATUS on a downstream port. Returns
    /// `wPortStatus | wPortChange << 16`.
    fn hub_get_port_status(&mut self, di: usize, port: u8) -> u32 {
        // SAFETY: single-CPU; DEV is ours.
        let buf_pa = unsafe { dma_addr((&raw const (*(&raw const DEV))[di].buf) as u64) };
        // bmRequestType=0xA3 (in, class, other), bRequest=0 (GET_STATUS).
        if !self.control(di, [0xA3, 0, 0, 0, port, 0, 0, 0], buf_pa, 4, true) {
            return 0;
        }
        // SAFETY: single-CPU; DEV is ours.
        let b = unsafe { &(*(&raw const DEV))[di].buf };
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    /// Log each root-hub port's connect status (D1 verification target).
    pub fn scan_ports(&mut self) {
        for i in 0..self.ports as usize {
            let p = self.regs.port_register_set.read_volatile_at(i).portsc;
            kprintln!(
                "[usb] port {}: ccs={} ped={} pp={} speed={}",
                i + 1,
                p.current_connect_status() as u8,
                p.port_enabled_disabled() as u8,
                p.port_power() as u8,
                p.port_speed()
            );
        }
    }
}
