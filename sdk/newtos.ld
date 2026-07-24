/*
 * newtos.ld -- linker script for NewTOS native programs (`.NCP` / `.NAP`).
 *
 * Produces an ELF the NewTOS multi-page-TPA loader accepts. The loader
 * (tos-api/src/bdos/process/{elf,loader,mmu,arena}.rs) maps a per-process
 * Transient Program Area of TPA_SIZE (4 MiB today) at low-half VA 0x0:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x400, 0x670)          classic low-memory system area (sysvars +
 *                             OSHEADER + cookie jar) — populated per-process
 *                             by the loader (`populate_sysvars`, B1 / Option 1,
 *                             docs/analysis/bios-audit/b1-lowmem-feasibility.md).
 *                             `_sysbase`@0x4F2 and `_p_cookies`@0x5A0 live here.
 *     [0x1000, image_end)     PT_LOAD image: .text/.rodata/.data (may span
 *                             many pages)
 *     [image_end, bss_end)    .bss (zeroed by the loader)
 *     [bss_end, p_hitpa)      free TPA: heap grows up, stack grows down
 *
 * The image base moved from 0x100 to 0x1000 (B1, Option 1): the classic TOS
 * low-memory system variables live at absolute [0x400, 0x800) and must not
 * overlap the app image, so the image now starts at the first page above the
 * reserved low region. The BasePage stays at VA 0 (crt0 reads `p_hitpa`@0x04
 * through the null base), so crt0/user_entry are unchanged.
 *
 * Loader constraints:
 *   - ELFCLASS64, ELFDATA2LSB, ET_EXEC, EM_AARCH64, e_version = 1.
 *   - e_phentsize = 56 (guaranteed for ELF64). One or more PT_LOAD segments.
 *   - Every PT_LOAD must satisfy
 *         0x1000 <= p_vaddr  and  p_vaddr + p_memsz <= TPA_SIZE - 64 KiB
 *     (the top of the TPA is reserved for the downward-growing stack).
 *   - e_entry must be inside the image (>= 0x1000).
 *
 * We lay everything out in one RWX load segment starting at 0x1000 (the loader
 * maps every TPA page UserRwExec). crt0's `.text.boot` is placed first so
 * `_start` lands at 0x1000 = e_entry. The linker exports `__bss_start` /
 * `__bss_end` / `_end` so crt0 / a future libc can find the BSS and heap; the
 * loader zeroes the BSS, so crt0 need not.
 */

ENTRY(_start)

PHDRS
{
    /* One RWX PT_LOAD; the loader maps every TPA page UserRwExec, and the
       in-TPA stack/heap need it writable. */
    text PT_LOAD FLAGS(7);
}

SECTIONS
{
    /* Start at the first page above the reserved low region (BasePage @0,
       classic sysvars/OSHEADER/cookie-jar in [0x400, 0x670)). B1/Option 1. */
    . = 0x1000;

    .text : {
        *(.text.boot)          /* crt0 _start first -> e_entry = 0x1000 */
        *(.text .text.*)
    } :text

    .rodata : {
        *(.rodata .rodata.*)
    } :text

    .data : {
        *(.data .data.*)
    } :text

    /* BSS is NOLOAD (no file bytes); the loader zeroes [__bss_start,
       __bss_end) as part of zeroing the whole TPA. */
    . = ALIGN(16);
    __bss_start = .;
    .bss (NOLOAD) : {
        *(.bss .bss.*)
        *(COMMON)
    } :text
    . = ALIGN(16);
    __bss_end = .;

    /* First free byte of the TPA — where a heap allocator starts. */
    _end = .;

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
