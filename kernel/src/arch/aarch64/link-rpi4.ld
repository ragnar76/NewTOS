/* link-rpi4.ld — NewTOS kernel image for Raspberry Pi 4 (BCM2711)
 *
 * RPi firmware loads `kernel8.img` (raw binary) at physical address 0x80000
 * in 64-bit mode and jumps there. We place .text.boot first so _start is the
 * very first instruction in the raw image.
 *
 * Source: Raspberry Pi Foundation boot documentation,
 *         https://www.raspberrypi.com/documentation/computers/configuration.html
 *
 * ─── Phase 05 high-half migration ──────────────────────────────────
 *
 * Same VA/LMA scheme as `link-qemu-virt.ld`: kernel symbols are
 * linked at high-half VAs (0xFFFF_0000_0000_0000 + physical), the
 * `AT()` clause pins each loadable section's LMA to the physical
 * load address. The raw `kernel8.img` is the LMA byte stream — the
 * RPi firmware copies it to physical 0x80000 and jumps to its first
 * byte (which is the LMA of `_start`).
 *
 * `_start_phys = _start - VA_OFFSET` is provided for symmetry with
 * QEMU; on RPi the firmware does not consume a structured entry
 * point so this symbol is purely diagnostic.
 */

VA_OFFSET   = 0xFFFF000000000000;
KERNEL_PHYS = 0x00080000;

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
STRAM_OS_RESERVE_PHYS = 0x400000;   /* membot */
DMA_PHYS              = 0xE00000;   /* $E00000 window, 2 MiB */

ENTRY(_start_phys)

PHDRS
{
    segment_boot_core_stack PT_LOAD FLAGS(6); /* RW low-half pre-MMU stack */
    segment_code            PT_LOAD FLAGS(5); /* RX kernel image, VA high-half */
    segment_data            PT_LOAD FLAGS(6); /* RW kernel data */
}

SECTIONS
{
    /* Pre-MMU boot stack: grows down from 0x80000 toward 0x0. Used
     * only by the boot stub before the high-half MMU jump. Stays at
     * **low-half VA = LMA** so boot.s can address it pre-MMU. */
    . = 0x00000000;
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
     * See link-qemu-virt.ld for the layout rationale. Same on RPi4. */
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

    /* ─── Phys-address aliases for boot.s pre-MMU access ─────────── */
    _start_phys              = _start              - VA_OFFSET;
    __bss_phys_start         = __bss_start         - VA_OFFSET;
    __bss_phys_end_exclusive = __bss_end_exclusive - VA_OFFSET;
    __dma_phys_start         = __dma_start         - VA_OFFSET;
    __dma_phys_end           = __dma_end           - VA_OFFSET;

    /* Phase 05 D2 — phys aliases for the Stage-1 boot page tables.
     * See link-qemu-virt.ld for rationale. */
    __boot_l0_phys = __BOOT_L0 - VA_OFFSET;
    __boot_l1_phys = __BOOT_L1 - VA_OFFSET;

    /DISCARD/ : { *(.comment) *(.note.*) *(.eh_frame*) }
}
