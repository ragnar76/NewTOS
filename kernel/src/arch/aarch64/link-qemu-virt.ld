/* link-qemu-virt.ld — NewTOS kernel image for QEMU `aarch64-virt`
 *
 * QEMU's `-kernel` flag for the `virt` machine loads each loadable
 * ELF segment at the segment's LMA and jumps to the ELF entry point.
 * The virt machine reserves the first 128 KiB of RAM
 * (0x4000_0000 .. 0x4008_0000) for device-tree blob and firmware,
 * so we place the kernel image at 0x4008_0000 (LMA).
 *
 * Source: QEMU source `hw/arm/boot.c` (`info->loader_start + 0x80000`).
 *
 * ─── Phase 05 high-half migration ──────────────────────────────────
 *
 * Kernel symbols are linked at high-half VAs (0xFFFF_0000_0000_0000 +
 * physical), but loaded at low-half LMAs (their physical addresses).
 * The `AT()` clauses on each loadable section do the VA/LMA split.
 *
 * Pre-MMU code (boot.s) executes at LMA — every PC-relative branch
 * (B/BL) within the linked image computes the same offset whether
 * source/target are taken at LMA or at VA, because both shift by
 * the same VA_OFFSET. So `bl <high-half symbol>` reaches the right
 * machine code pre-MMU. Absolute symbol references (LDR =sym) load
 * the high-half VA — pre-MMU code that needs phys addresses (BSS
 * bounds, boot page tables) uses the `*_phys` aliases below.
 *
 * The ELF entry point is `_start_phys` (= `_start - VA_OFFSET`) so
 * QEMU jumps to the LMA of `_start`, not its high-half VA.
 */

VA_OFFSET   = 0xFFFF000000000000;
KERNEL_PHYS = 0x40080000;

/* ─── Fixed ST-RAM geometry (see bios/machine/layout.rs) ─────────────
 *
 * The GEMDOS ST-RAM pool must be a size a program can rely on, not the
 * remainder behind a kernel image that moves with every build. So the
 * OS gets a fixed budget at the bottom of the ST-RAM window and the pool
 * starts at a constant `membot`:
 *
 *   DRAM_START            .. +4 MiB   OS: boot stack, image, kernel stack
 *   membot = +4 MiB       .. +14 MiB  ST-RAM pool — exactly 10 MiB, always
 *   memtop = phystop = +14 MiB
 *   +14 MiB .. +16 MiB               .dma (Atari $E00000 register window)
 *   TTRAM_START = +16 MiB            TT-RAM pool
 *
 * `ASSERT` below turns "the kernel outgrew its budget" into a link
 * error instead of a silently smaller ST-RAM.
 */
STRAM_OS_RESERVE_PHYS = 0x40400000;   /* membot */
DMA_PHYS              = 0x40E00000;   /* $E00000 window, 2 MiB */

ENTRY(_start_phys)

PHDRS
{
    segment_boot_core_stack PT_LOAD FLAGS(6); /* RW low-half pre-MMU stack */
    segment_code            PT_LOAD FLAGS(5); /* RX kernel image, VA high-half */
    segment_data            PT_LOAD FLAGS(6); /* RW kernel data */
}

SECTIONS
{
    /* The pre-MMU boot stack lives in the 128 KiB reserved by QEMU
     * firmware below the kernel load address (0x4000_0000..0x4008_0000).
     * Stays at **low-half VA = LMA** because boot.s uses it before
     * the MMU is on. Once kernel_main runs (post-MMU at high-half),
     * the SP is repointed at the in-image `.stack` region. */
    . = 0x40000000;
    __boot_core_stack_start = .;
    . = KERNEL_PHYS;
    __boot_core_stack_end_exclusive = .;

    /* ----- kernel image: VA in high-half, LMA at KERNEL_PHYS ----- */
    . = VA_OFFSET + KERNEL_PHYS;
    __kernel_image_start = .;

    .text : AT(KERNEL_PHYS) ALIGN(4)
    {
        KEEP(*(.text.boot))
        *(.text .text.*)
    } :segment_code

    .rodata : ALIGN(8) { *(.rodata .rodata.*) } :segment_code

    /* ----- data ---------------------------------------------------- */
    .data : ALIGN(8) { *(.data .data.*) } :segment_data

    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end_exclusive = .;
    } :segment_data

    /* ----- in-image kernel stack with hardware guard (Phase 03 D6) -
     *
     * Layout (all 2 MiB-aligned):
     *   __stack_guard_start ─┐
     *                        │ 2 MiB GUARD  → mmu::build() leaves this
     *                        │                block invalid in the L2
     *                        │                table; any access faults
     *   __stack_bottom_inclusive
     *                        │ 64 KiB main kernel stack (SP_EL0)
     *   __stack_top_exclusive
     *                        │ 16 KiB emergency exception stack (SP_EL1)
     *   __emerg_stack_top_exclusive
     *
     * The guard is exactly one 2 MiB block, matching the granularity
     * of our identity page tables (we have only L0/L1/L2, no L3).
     * Section is NOLOAD so the ELF stays small (segment is RW).
     */
    .stack (NOLOAD) : ALIGN(4K)
    {
        __stack_guard_start = .;
        . = . + 4K;
        __stack_guard_end = .;
        __stack_bottom_inclusive = .;
        . = . + 64K;
        __stack_top_exclusive = .;
        . = ALIGN(16);
        __emerg_stack_bottom_inclusive = .;
        . = . + 16K;
        __emerg_stack_top_exclusive = .;
    } :segment_data

    . = ALIGN(4096);
    __kernel_image_end = .;

    ASSERT(__kernel_image_end - VA_OFFSET <= STRAM_OS_RESERVE_PHYS,
           "kernel image + stack overruns STRAM_OS_RESERVE (membot) -- see bios/machine/layout.rs")

    /* ----- controller DMA block --------------------------------------
     * A dedicated 2-MiB-aligned, 2-MiB-sized region for all xHCI DMA
     * structures, so it can be remapped Normal-NC / Device-nGnRnE on the
     * non-coherent Pi-4 PCIe (PHASE-16 D7) without touching kernel code
     * or page tables in the same block.
     *
     * It sits in the 2 MiB the ST-RAM slot already excludes from every
     * pool — the Atari hardware-register window `$E00000-$FFFFFF`
     * (layout.rs::STRAM_IO_RESERVE). That is memory no GEMDOS pool ever
     * describes, which is exactly what a permanently-owned hardware
     * buffer needs, and it keeps the 10 MiB ST-RAM pool intact.
     *
     * NOLOAD and outside `.bss`, so the boot stub zeroes it through the
     * separate `__dma_phys_*` pair (arch/aarch64/boot.rs).
     */
    . = VA_OFFSET + DMA_PHYS;
    .dma (NOLOAD) : ALIGN(2M)
    {
        __dma_start = .;
        *(.dma .dma.*)
        . = ALIGN(2M);
        __dma_end = .;
    } :segment_data

    /* ─── Phys-address aliases for boot.s pre-MMU access ───────────
     * boot.s uses these to zero BSS and to address its boot page
     * tables before the MMU is enabled (high-half VAs do not
     * resolve at that point).
     *
     * `_start_phys` is the ELF entry point: QEMU's `-kernel` jumps
     * to this LMA, where the boot stub's machine code actually
     * resides pre-MMU.
     */
    _start_phys              = _start              - VA_OFFSET;
    __bss_phys_start         = __bss_start         - VA_OFFSET;
    __bss_phys_end_exclusive = __bss_end_exclusive - VA_OFFSET;
    __dma_phys_start         = __dma_start         - VA_OFFSET;
    __dma_phys_end           = __dma_end           - VA_OFFSET;

    /* Phase 05 D2 — phys aliases for the Stage-1 boot page tables.
     * `__BOOT_L0` and `__BOOT_L1` are `#[no_mangle]` Rust statics in
     * `arch::aarch64::boot`, linked into `.bss.boot_pgtables` and
     * `.rodata.boot_pgtables` respectively. Both end up at high-half
     * VAs (their containing sections are part of the kernel image);
     * boot.s needs the LMAs to write/read them pre-MMU. */
    __boot_l0_phys = __BOOT_L0 - VA_OFFSET;
    __boot_l1_phys = __BOOT_L1 - VA_OFFSET;

    /DISCARD/ : { *(.comment) *(.note.*) *(.eh_frame*) }
}
