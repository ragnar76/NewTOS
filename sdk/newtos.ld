/*
 * newtos.ld -- linker script for NewTOS Stage-1 native programs (`.NCP`).
 *
 * Produces an ELF the NewTOS loader accepts, matching the shape of the
 * hand-crafted selftest ELFs in kernel/src/selftest_pexec.rs, but generated
 * by the compiler + linker.
 *
 * Loader constraints (tos-api/src/bdos/process/{elf,loader,mmu,arena}.rs):
 *   - ELFCLASS64, ELFDATA2LSB, ET_EXEC, EM_AARCH64, e_version = 1.
 *   - One PT_LOAD segment, e_phentsize = 56 (guaranteed for ELF64).
 *   - The single 4 KiB TPA page is mapped at VA 0x0; its first 256 bytes are
 *     reserved for the BasePage, so every PT_LOAD segment must satisfy
 *         0x100 <= p_vaddr  and  p_vaddr + p_memsz <= 0x1000.
 *   - e_entry must be inside that page (>= 0x100).
 *
 * We therefore lay everything out in one RWX load segment starting at 0x100
 * (matching mmu::PageFlags::UserRwExec for the mapped page) and keep the whole
 * image well under 0x1000. crt0's `.text.boot` is placed first so `_start`
 * lands at 0x100 = e_entry.
 */

ENTRY(_start)

PHDRS
{
    /* One RWX PT_LOAD; the loader maps the page UserRwExec anyway, and the
       in-page stack needs the page writable. */
    text PT_LOAD FLAGS(7);
}

SECTIONS
{
    /* Start right after the 256-byte BasePage window. */
    . = 0x100;

    .text : {
        *(.text.boot)          /* crt0 _start first -> e_entry = 0x100 */
        *(.text .text.*)
        *(.rodata .rodata.*)
        *(.data .data.*)
    } :text

    .bss (NOLOAD) : {
        *(.bss .bss.*)
        *(COMMON)
    } :text

    /* Drop metadata the NewTOS loader neither reads nor has room for. */
    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame*)
        *(.ARM.exidx*)
        *(.ARM.attributes)
        *(.got .got.plt)
    }
}
