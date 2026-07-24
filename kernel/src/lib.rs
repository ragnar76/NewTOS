//! NewTOS kernel (library crate root).
//!
//! The kernel is split into `src/lib.rs` (this file: modules, library API)
//! and `src/main.rs` (the boot binary that pulls the lib in and exposes
//! the `kernel_main` symbol referenced by the boot stub).

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![allow(internal_features)]

extern crate alloc;

#[global_allocator]
static GLOBAL_ALLOCATOR: bios::machine::heap::KernelAllocator =
    bios::machine::heap::KernelAllocator;

// Compile-time enforcement: exactly one board feature must be selected.
#[cfg(all(feature = "board-rpi4", feature = "board-qemu-virt"))]
compile_error!(
    "kernel: features `board-rpi4` and `board-qemu-virt` are mutually exclusive."
);

#[cfg(not(any(feature = "board-rpi4", feature = "board-qemu-virt")))]
compile_error!(
    "kernel: exactly one of `board-rpi4` or `board-qemu-virt` must be enabled."
);

pub mod arch;
pub mod bios;
pub mod console;
pub mod drivers;
pub mod hal;
pub mod host_env;
pub mod panic;
pub mod selftest;
pub mod selftest_d3;
pub mod selftest_d6;
pub mod selftest_d7;
pub mod selftest_ec4;
pub mod selftest_time;
pub mod selftest_pexec;
pub mod selftest_pexec_d4d5;
pub mod bench;
pub mod fbcon;
pub mod ikbd;
pub mod keymap;
pub mod shell;
pub mod selftest_aes;
pub mod selftest_bios;
pub mod selftest_console;
pub mod selftest_cookie;
pub mod selftest_objc;
pub mod selftest_sysvars;
pub mod selftest_upcall;
pub mod selftest_sleepwake;
pub mod selftest_sleepwake_switch;
pub mod selftest_etv_term;
pub mod aes_demo;
pub mod desk;
pub mod launch;
pub mod selftest_ikbd;
pub mod selftest_vdi;
pub mod selftest_raster;
pub mod selftest_fbcon;
pub mod selftest_xbios;
pub mod sync;
pub mod initinfo;
#[cfg(feature = "board-rpi4")]
pub mod selftest_emmc;
pub mod vdi_backend;
pub mod vdi_client;

// TEMPORARY debug tooling — NOT for product history. Gated behind
// `dbg_qedtrace::QED_SVC_TRACE`; remove the module + its exception hooks to
// drop it entirely.
pub mod dbg_qedtrace;

// ---------------------------------------------------------------------------
// kprintf! / kprintln! (Phase 03 D7) — formatter macros into the boot
// console. Safe to call from any context (panic, IRQ, pre/post MMU) as
// long as `console::CONSOLE` has been initialized at least once. The
// PL011 driver writes byte-by-byte through MMIO with no internal
// locking, so the macros are non-blocking and recursion-safe; bytes
// from concurrent callers may interleave but neither caller will hang.
// ---------------------------------------------------------------------------

/// Formatted write to the kernel console without a trailing newline.
///
/// Equivalent to `core::write!(UartWriter(&CONSOLE), …)`. The
/// formatter result is discarded — `UartWriter::write_str` cannot
/// fail.
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use ::core::fmt::Write as _;
        let _ = ::core::write!(
            $crate::hal::uart::UartWriter(&$crate::console::CONSOLE),
            $($arg)*
        );
    }};
}

/// Formatted write to the kernel console with a trailing newline.
#[macro_export]
macro_rules! kprintln {
    () => {{ $crate::kprint!("\n"); }};
    ($($arg:tt)*) => {{
        use ::core::fmt::Write as _;
        let _ = ::core::writeln!(
            $crate::hal::uart::UartWriter(&$crate::console::CONSOLE),
            $($arg)*
        );
    }};
}

/// Friendly board name for boot output.
#[cfg(feature = "board-rpi4")]
pub const BOARD_NAME: &str = "rpi4";
/// Friendly board name for boot output.
#[cfg(feature = "board-qemu-virt")]
pub const BOARD_NAME: &str = "qemu-virt";

/// Rust kernel entry. Called by the AArch64 boot stub once the stack is
/// set and BSS is zeroed.
///
/// This is a thin naked shim that performs the SPSel = 0 switch
/// required by Phase 03 D6 (stack guard) before any Rust function
/// runs:
///
/// 1. While SPSel is still 1 (boot-stub default), repoint SP_EL1 at
///    the dedicated emergency exception stack via `mov sp, x_emerg`.
///    The boot stub's pre-MMU stack is now abandoned — no Rust code
///    has executed on it past the BSS-zero loop.
/// 2. Program SP_EL0 with the in-image main kernel stack top using
///    the system-register write (allowed at EL1 regardless of SPSel).
/// 3. `MSR SPSel, #0` (register form) — flip the active stack to
///    SP_EL0. From here on, every exception taken from EL1 enters via
///    the "Current EL with SP0" vector group and auto-promotes SPSel
///    to 1, so handlers run on SP_EL1 = emergency stack.
/// 4. Tail-call [`kernel_main_rust`].
///
/// The shim is `#[unsafe(naked)]` so that no compiler-generated
/// prologue interferes with the precise sequencing.
///
/// `x0` carries the device-tree blob pointer from the boot stub
/// (Phase 03 D8). The shim deliberately preserves `x0` so it reaches
/// `kernel_main_rust` as the AAPCS64 first argument.
///
/// Note on `MSR SP_EL1, Xt`: that encoding is `UNDEFINED` at EL1
/// (ARM ARM C5.2.20 — SP_EL1 is writable as a system register only
/// from EL2 or higher). The only legal way to set SP_EL1 from EL1 is
/// `mov sp, …` while SPSel = 1, which is what step 1 does.
///
/// # Safety
///
/// Must be called exactly once, from the AArch64 boot stub
/// (`arch::aarch64::boot`), as the very first Rust function on the
/// boot CPU. Pre-conditions enforced by the boot stub, not by this
/// function:
///
/// - SPSel = 1 (active SP is SP_EL1) on entry, so the `mov sp, x_emerg`
///   in step 1 actually programs SP_EL1 (the only legal SP_EL1 write
///   path at EL1).
/// - `.bss` has been zeroed.
/// - `x0` holds the device-tree-blob physical address (or `0`).
///
/// The function never returns and is sound only as the kernel
/// entry point; calling it from anywhere else would clobber the
/// active stack and switch SP banks unexpectedly.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_main() -> ! {
    core::arch::naked_asm!(
        // x0 = DTB phys (from boot stub) — preserved end-to-end.
        // Step 1: SP_EL1 := emergency exception stack top.
        "ldr   x11, ={emerg_top}",
        "mov   sp, x11",
        // Step 2: SP_EL0 := main kernel stack top.
        "ldr   x10, ={stack_top}",
        "msr   sp_el0, x10",
        // Step 3: switch the active SP to SP_EL0.
        "mov   x12, #0",
        "msr   spsel, x12",
        // Step 4: enter Rust. x0 still carries the DTB pointer.
        "b     {rust}",
        stack_top = sym __stack_top_exclusive,
        emerg_top = sym __emerg_stack_top_exclusive,
        rust      = sym kernel_main_rust,
    )
}

unsafe extern "C" {
    static __stack_top_exclusive: u8;
    static __emerg_stack_top_exclusive: u8;
}

/// Rust-level kernel entry. Tail-called by the [`kernel_main`] naked
/// shim with `x0 = device-tree-blob phys addr` (or `0` if the firmware
/// did not provide one).
///
/// Phase 03 D8: the DTB pointer feeds `bios::machine::layout::discover` so the
/// memory map reflects the actual installed DRAM rather than the
/// compile-time board default.
extern "C" fn kernel_main_rust(dtb_phys: usize) -> ! {
    use crate::hal::uart::Uart;

    console::CONSOLE.init();
    kprintln!("NewTOS booting on {}", BOARD_NAME);
    kprintln!("Phase 02 reached.");

    let dtb = if dtb_phys == 0 { None } else { Some(dtb_phys) };
    if let Some(p) = dtb {
        kprintln!("[mm] DTB at 0x{:016x}", p);
    } else {
        kprintln!("[mm] no DTB — using compiled-in DRAM size");
    }

    // Discover the physical memory map and dump it for visibility.
    // Pure function — safe to call this early (no MMIO, no allocation).
    let map = bios::machine::layout::discover(dtb);
    print_memory_map(&map);

    // Replace the Stage-1 boot tables (1-GiB blocks, both TTBRs
    // identical, set up by `arch::aarch64::boot::_start`) with the
    // full Stage-2 mapping and clear TTBR0_EL1 so any low-half VA
    // access faults from this point on (Phase 05 D3-bis / D4).
    // SAFETY: boot CPU only, single-threaded; the static page
    // tables are mutated and TTBR0/TTBR1 swapped inside.
    unsafe { arch::aarch64::mmu::reinstall(&map) };
    kprintln!("[mm] MMU reinstalled (high-half via TTBR1, TTBR0 deferred to Phase 06).");

    // Initialise the physical frame allocator (Phase 03 D4).
    bios::machine::frames::init(&map);
    print_frame_stats();
    smoke_test_frame_allocator();

    // Bring up the kernel heap on top of the frame allocator
    // (Phase 03 D5). After this point `alloc::{Box, Vec, String}` is
    // legal kernel-side.
    bios::machine::heap::init();
    print_heap_stats("after init");
    smoke_test_heap();
    print_heap_stats("after smoke");

    // Bring up the framebuffer (if any). Done before the BDOS pools
    // claim the remaining frames so the framebuffer's contiguous run is
    // reserved up front. The screen is drawn later via the VDI.
    init_framebuffer();

    // Bring up the BDOS memory pools (Phase 04 D6). The two MPBs
    // are seeded with whatever frames remain free in each pool
    // after the kernel heap reservation. From this point on the
    // frame allocator must not hand out frames that overlap these
    // ranges — `claim_remaining_pool` enforces this by allocating
    // the entire remaining contiguous run before publishing it to
    // BDOS.
    bring_up_bdos_pools();
    print_bdos_pool_stats("after init");

    // Install exception vectors and the TOS-API host before any SVC.
    arch::aarch64::exceptions::init();
    host_env::install();

    // Seed the NewTOS cookie jar (system-feature registry, read via the
    // Ssystem trap). Version 0x0000_0100 = NewTOS 0.1.0.
    tos_api::cookie::seed_defaults(0x0000_0100);

    // Bring up the GIC, enable the PL011 RX interrupt, and unmask IRQs.
    drivers::gic::init();
    console::CONSOLE.enable_rx_interrupt();
    drivers::gic::enable_irqs();

    // Bring up the microSD card and mount its partitions before the boot
    // screen, so the drive map is populated when initinfo renders it.
    // board-rpi4 / QEMU raspi4b only (no block device on aarch64-virt).
    #[cfg(feature = "board-rpi4")]
    // SAFETY: boot CPU; VideoCore firmware has released the card.
    if unsafe { drivers::bcm2711::emmc::init() } {
        tos_api::bios::mount();
        // Select the keyboard layout from the card (KEYBOARD.KBD override or
        // the #K line in NEWDESK.INF), before the console/desktop reads keys.
        keymap::init_from_disk();
    }

    // Bring up the USB host stack (keyboard/mouse) — PHASE-16. On QEMU-virt
    // this finds the xHCI on the PCIe bus and reports its ports; on real
    // Pi-4 it is a no-op until the VL805 firmware path (D7).
    drivers::usb::init(dtb_phys);

    initinfo::run();

    // Development self-tests are no longer invoked during the normal
    // boot — the system goes straight to the console. They remain in the
    // `selftest*` modules and can be re-enabled here when debugging
    // (e.g. `selftest_vdi::run_phase12_vdi_demo()` draws the Phase-12
    // drawing-surface demo verified by QMP screendump,
    // `selftest_aes::run_phase13_aes_test()` for the AES lifecycle, or
    // `selftest_sysvars::run()` to prove the B1 per-process low-memory
    // sysvars/OSHEADER/$5A0 area — docs/analysis/bios-audit/
    // b1-lowmem-feasibility.md, Option 1 — is populated in a loaded EL0 TPA:
    // checks _sysbase/os_version/_p_cookies + the Setexc($5A0/4,-1) idiom).

    // Keep the initinfo boot screen visible briefly before the desktop
    // takes over (EmuTOS shows its boot screen the same way).
    arch::aarch64::delay_ms(2000);

    // Start the GEM desktop on the HDMI framebuffer (EmuTOS deskmain): it
    // draws the menu bar, drive/trash/printer icons and the grey desktop,
    // then dispatches AES events. Mouse interaction works once the IKBD
    // mouse is wired (UART2); in QEMU it renders and waits. The time-driven
    // AES `aes_demo::showcase()` and the serial `shell::run()` remain
    // available for debugging.
    // Switch the screen to the framebuffer console first, so any boot /
    // AUTO-program console output (Cconws) is visible on HDMI — EmuTOS shows
    // its AUTO programs on the console before the GUI.
    fbcon::activate();

    // Run the AUTO folder (\AUTO\*.PRG) before the GUI — EmuTOS' autoexec.
    // No-op until the boot drive has an \AUTO\ folder with programs; an
    // invalid/empty .PRG fails the load gracefully (no fault).
    launch::run_auto_folder();

    desk::run();

    // The desktop loop returns only on Shutdown.
    kprintln!("[desk] shutdown — halting");
    arch::aarch64::halt();
}

/// Pretty-print the physical memory map on the boot console.
fn print_memory_map(map: &bios::machine::layout::MemoryMap) {
    kprintln!("[mm] memory map:");
    print_range("kernel", &map.kernel);
    print_range("ST-RAM", &map.stram);
    print_range("TT-RAM", &map.ttram);
    for (i, r) in map.mmio.iter().enumerate() {
        kprintln!(
            "[mm]   MMIO {:02}: 0x{:016x}..0x{:016x}",
            i, r.start, r.end,
        );
    }
}

fn print_range(label: &str, r: &bios::machine::layout::PhysRange) {
    kprintln!(
        "[mm]   {:<6}: 0x{:016x}..0x{:016x} ({} bytes)",
        label, r.start, r.end, r.len(),
    );
}

/// Bring up the board framebuffer (if present). The screen content is
/// drawn later through the VDI (`initinfo`), not here — this just gets a
/// linear buffer published so [`hal::framebuffer`] / the VDI backend can
/// use it.
#[cfg(feature = "board-qemu-virt")]
fn init_framebuffer() {
    use crate::hal::framebuffer::Framebuffer;
    // SAFETY: boot CPU, single-threaded, frame allocator is up.
    let up = unsafe { drivers::qemu_virt::ramfb::init() };
    if !up {
        kprintln!("[fb] ramfb unavailable (pass `-device ramfb`) — no graphics");
        return;
    }
    let fb = &drivers::qemu_virt::ramfb::FRAMEBUFFER;
    kprintln!("[fb] ramfb up: {}x{}", fb.width(), fb.height());
}

/// RPi4 framebuffer from the VideoCore mailbox property interface (see
/// docs/hardware/mailbox-framebuffer.md). On real hardware the later VDI
/// drawing is the first life sign (no serial).
#[cfg(feature = "board-rpi4")]
fn init_framebuffer() {
    use crate::hal::framebuffer::Framebuffer;
    // SAFETY: boot CPU, single-threaded, MMU + heap are up.
    let up = unsafe { drivers::bcm2711::framebuffer::init() };
    if !up {
        kprintln!("[fb] VideoCore mailbox framebuffer unavailable — no graphics");
        return;
    }
    let fb = &drivers::bcm2711::framebuffer::FRAMEBUFFER;
    kprintln!(
        "[fb] VideoCore fb up: {}x{} stride {}px",
        fb.width(),
        fb.height(),
        fb.stride_px(),
    );
}

/// Dump per-pool frame counts.
fn print_frame_stats() {
    use crate::bios::machine::frames::{stats, Pool};
    for (label, pool) in [("ST-RAM", Pool::Stram), ("TT-RAM", Pool::Ttram)] {
        let s = stats(pool);
        kprintln!(
            "[mm] frames {}: total={} used={} free={} ({} B free)",
            label, s.total, s.used, s.free(), s.free_bytes(),
        );
    }
}

/// Allocate one frame from each pool, verify the address, free it.
fn smoke_test_frame_allocator() {
    use crate::bios::machine::frames::{alloc_frame, free_frame, stats, Pool};
    for (label, pool) in [("ST-RAM", Pool::Stram), ("TT-RAM", Pool::Ttram)] {
        let before = stats(pool).used;
        let f = alloc_frame(pool).expect("frame alloc failed");
        kprintln!("[mm] alloc {} frame -> 0x{:016x}", label, f.pa());
        free_frame(f);
        let after = stats(pool).used;
        debug_assert!(before == after, "frame leak in smoke test");
    }
}

/// Print kernel-heap accounting with a short label.
fn print_heap_stats(label: &str) {
    let s = bios::machine::heap::stats();
    kprintln!(
        "[mm] heap {}: total={} used={} free={} largest={} blocks={}",
        label, s.total, s.used, s.free(), s.largest_free, s.free_blocks,
    );
}

/// Exercise the kernel heap: a `Box`, a growing `Vec`, and a 1000-cycle
/// alloc/free loop. Verifies leak-free operation (used must return to
/// the post-init baseline) and that the allocator survives churn.
fn smoke_test_heap() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    let baseline_used = bios::machine::heap::stats().used;

    {
        let b = Box::new(0xDEAD_BEEF_u64);
        debug_assert_eq!(*b, 0xDEAD_BEEF_u64);
    }
    {
        let mut v: Vec<u32> = Vec::new();
        for i in 0..1024u32 {
            v.push(i);
        }
        debug_assert_eq!(v.len(), 1024);
        debug_assert_eq!(v[1023], 1023);
    }
    for i in 0..1000u32 {
        let v: Vec<u8> = Vec::with_capacity(((i * 7) & 0x3FF) as usize + 1);
        core::hint::black_box(v);
    }

    let after_used = bios::machine::heap::stats().used;
    debug_assert!(
        after_used == baseline_used,
        "heap leak in smoke test"
    );
    kprintln!("[mm] heap smoke OK");
}

/// Phase 04 D6 — claim every still-free frame in each pool as one
/// contiguous run and hand the resulting byte ranges to the BDOS
/// memory subsystem. From this point on, BDOS owns the bytes; the
/// kernel frame allocator only tracks them as "permanently used".
///
/// Heap-pre-allocation (Phase 03 D5) takes its 2 MiB from TT-RAM
/// before this runs, so ST-RAM is contiguous-from-start and TT-RAM
/// is contiguous-from-`heap_end`. Both runs satisfy
/// `alloc_frames_contig`.
fn bring_up_bdos_pools() {
    use bios::machine::frames::{alloc_frames_contig, stats, Pool, FRAME_SIZE};

    let claim = |pool: Pool| -> Option<(u64, u64)> {
        let s = stats(pool);
        let count = s.free();
        if count == 0 {
            return None;
        }
        let start = alloc_frames_contig(pool, count)
            .expect("frames: remaining-pool allocation must succeed (contiguous run)");
        Some((start.pa(), (count as u64) * FRAME_SIZE))
    };

    let (stram_start, stram_len) = claim(Pool::Stram).unwrap_or((0, 0));
    let (ttram_start, ttram_len) = claim(Pool::Ttram).unwrap_or((0, 0));

    // SAFETY: this is the boot CPU; no traps have run yet, so no
    // other code can be inside the BDOS layer. The two ranges come
    // from the frame allocator and are disjoint by construction.
    unsafe {
        tos_api::bdos::mem::init::init(stram_start, stram_len, ttram_start, ttram_len)
            .expect("bdos::mem::init failed (likely AddressOutOfRange — Phase 05 needed)");
    }

    kprintln!(
        "[mm] BDOS pools: ST-RAM 0x{:08x}+{} TT-RAM 0x{:08x}+{}",
        stram_start, stram_len, ttram_start, ttram_len,
    );
}

/// Phase 04 D13 — dump BDOS pool accounting on the boot console.
///
/// Calls [`tos_api::bdos::mem::stats::pool_stats`] for both pools
/// and prints a one-line snapshot per pool. Cheap O(n) walk over
/// `mp_mfl` + `mp_mal`; safe to call any time after
/// `bring_up_bdos_pools` has run.
fn print_bdos_pool_stats(label: &str) {
    use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
    for (name, kind) in [("ST-RAM", PoolKind::Stram), ("TT-RAM", PoolKind::Ttram)] {
        let s = pool_stats(kind);
        kprintln!(
            "[mm] BDOS {} {}: total={} free={} used={} largest_free={} (free_mds={} alloc_mds={})",
            name, label,
            s.total_bytes(), s.free_bytes, s.used_bytes, s.largest_free,
            s.free_blocks, s.allocated_blocks,
        );
    }
}
