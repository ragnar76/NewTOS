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
    .stack (NOLOAD) : ALIGN(2M)
    {
        __stack_guard_start = .;
        . = . + 2M;
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

    /* ─── Phys-address aliases for boot.s pre-MMU access ─────────── */
    _start_phys              = _start              - VA_OFFSET;
    __bss_phys_start         = __bss_start         - VA_OFFSET;
    __bss_phys_end_exclusive = __bss_end_exclusive - VA_OFFSET;

    /* Phase 05 D2 — phys aliases for the Stage-1 boot page tables.
     * See link-qemu-virt.ld for rationale. */
    __boot_l0_phys = __BOOT_L0 - VA_OFFSET;
    __boot_l1_phys = __BOOT_L1 - VA_OFFSET;

    /DISCARD/ : { *(.comment) *(.note.*) *(.eh_frame*) }
}
