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
 *     [0x1000, p_dbase)       PT_LOAD `text` (R-X): .text + .rodata
 *                             = the basepage's p_tbase / p_tlen
 *     [p_dbase, p_bbase)      PT_LOAD `data` (RW-): (.init_array +) .data
 *                             = p_dbase / p_dlen (file bytes)
 *     [p_bbase, _end)         .bss, the NOLOAD tail of `data` (zeroed by
 *                             the loader) = p_bbase / p_blen
 *     [_end, p_hitpa)         free TPA: heap grows up, stack grows down
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
 * Two PT_LOAD segments (TASK/T05), the usual ELF shape: `text` (R-X) holds
 * .text + .rodata, `data` (RW-) holds .data with .bss as its NOLOAD tail. The
 * loader's `populate_basepage` classifies segments by their flags -- the
 * first executable one is p_tbase/p_tlen, the first writable non-executable
 * one is p_dbase/p_dlen with p_bbase/p_blen = its memsz - filesz -- so this
 * layout is what makes all six basepage segment fields non-zero, the
 * contract EmuTOS `bdos/kpgmld.c:123-145` fills from the PRG header (p_dbase
 * = p_tbase + p_tlen, p_bbase = p_dbase + p_dlen). The segments are laid out
 * back to back (no page padding between them), which keeps those two
 * equalities exact and the file size unchanged; the loader still maps every
 * TPA page UserRwExec (loader.rs, "Deliberate simplifications"), so the flags
 * only feed the basepage today. The 16-byte ALIGN at each segment's tail is
 * INSIDE the last section on purpose: it makes p_tbase + p_tlen == p_dbase
 * and p_bbase + p_blen == _end hold to the byte.
 *
 * .init_array / .fini_array are placed explicitly, before .bss: as orphans
 * lld put them AFTER .bss, which turned the whole BSS into file-backed zeros
 * (memsz == filesz, p_blen 0). crt0 does not run them (mintlib's init_clock
 * is the one entry today); placing them is a layout decision, not a promise.
 *
 * crt0's `.text.boot` is placed first so `_start` lands at 0x1000 = e_entry.
 * The linker exports `__bss_start` / `__bss_end` / `_end` so crt0 / the libc
 * can find the BSS and heap; the loader zeroes the BSS, so crt0 need not.
 */

ENTRY(_start)

PHDRS
{
    /* R-X: .text + .rodata -> p_tbase / p_tlen. */
    text PT_LOAD FLAGS(5);
    /* RW-: (.init_array +) .data, .bss as the NOLOAD tail
       -> p_dbase / p_dlen, p_bbase / p_blen. */
    data PT_LOAD FLAGS(6);
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
        . = ALIGN(16);         /* text segment ends 16-aligned: p_dbase = p_tbase + p_tlen */
    } :text

    /* Constructor / destructor tables, kept out of the BSS tail (see the
       header). Nothing runs them today. */
    .init_array : {
        *(.init_array .init_array.*)
    } :data
    .fini_array : {
        *(.fini_array .fini_array.*)
    } :data

    .data : {
        *(.data .data.*)
        . = ALIGN(16);         /* data file part ends 16-aligned: p_bbase = p_dbase + p_dlen */
    } :data

    /* BSS is NOLOAD (no file bytes); the loader zeroes [__bss_start,
       __bss_end) as part of zeroing the whole TPA. It is the memsz-only tail
       of the `data` segment, so p_bbase + p_blen == _end. */
    __bss_start = .;
    .bss (NOLOAD) : {
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
    } :data
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
