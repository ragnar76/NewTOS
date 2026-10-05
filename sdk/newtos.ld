/*
 * newtos.ld -- linker script for NewTOS native programs (`.NCP` / `.NAP`).
 *
 * Produces a position-independent executable (ELF ET_DYN) the NewTOS
 * multi-page-TPA loader accepts (ARCH03 stage 0d; link with
 * `-pie --no-dynamic-linker -z text`, compile with `-fPIE`). The loader
 * (tos-api/src/bdos/process/{elf,reloc,loader,mmu,arena}.rs) streams the
 * segments into a per-process Transient Program Area of TPA_SIZE (4 MiB)
 * and applies the image's R_AARCH64_RELATIVE relocations for the address
 * the TPA executes at. Since ARCH03 stage 1 the TPA is a page-aligned BDOS
 * pool block at its own address B (one address space shared by every
 * process, VA = PA); a link address `v` below runs at `B + v`:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader) -- B is the
 *                             program's PD pointer
 *     [0x100, 0x1000)         reserved
 *     [0x1000, p_dbase)       PT_LOAD `text` (R-X): .text + the dynamic
 *                             metadata (.dynsym .gnu.hash .hash .dynstr
 *                             .rela.dyn) + .rodata
 *                             = the basepage's p_tbase / p_tlen
 *     [p_dbase, p_bbase)      PT_LOAD `data` (RW-): (.init_array +)
 *                             .dynamic + .got + .data
 *                             = p_dbase / p_dlen (file bytes)
 *     [p_bbase, _end)         .bss, the NOLOAD tail of `data` (zeroed by
 *                             the loader) = p_bbase / p_blen
 *     [_end, p_hitpa)         free TPA: heap grows up, stack grows down
 *
 * The classic TOS low-memory system variables (`_sysbase`@0x4F2,
 * `_p_cookies`@0x5A0, ...) are not part of the TPA: they live in the
 * process's own page 0, absolute VA [0x400, 0x800) (`populate_sysvars`,
 * B1 / Option 1, plan E3). The image starts at 0x1000 so it keeps its 4-KiB
 * page offsets relocated (adrp). crt0 finds the BasePage through x0 (the
 * loader passes its address on every launch path), never through a fixed
 * address.
 *
 * Loader constraints:
 *   - ELFCLASS64, ELFDATA2LSB, ET_DYN, EM_AARCH64, e_version = 1,
 *     e_flags = 0, no PT_INTERP. ET_EXEC is refused (EPLFMT).
 *   - Dynamic relocations: only R_AARCH64_RELATIVE, in one DT_RELA table
 *     (no DT_REL, no PLT/DT_JMPREL, no DT_NEEDED, no text relocations --
 *     `-z text` makes the linker refuse those at build time). The PT_DYNAMIC
 *     segment points the loader at the table.
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
 * PIE sections. lld creates the dynamic metadata (.dynsym, .gnu.hash, .hash,
 * .dynstr, .rela.dyn) and .dynamic / .got for every -pie link; they are named
 * here instead of left as orphans, because orphan placement put .dynamic
 * AFTER .bss -- which turns the BSS into file-backed zeros, the same failure
 * as the .init_array case below. The read-only metadata goes into `text`
 * (`.rela.dyn` must not be in a writable segment: nothing relocates the
 * table itself), .dynamic and .got into `data` ahead of .data, so .data with
 * its 16-byte tail ALIGN stays the last file-backed section. .got is KEPT
 * (it used to be discarded): with -fPIE a reference to an undefined weak or
 * an extern symbol may go through a GOT slot, and with the GOT discarded the
 * 0d toolchain spike showed such a load reading address 0 instead.
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
    /* R-X: .text + dynamic metadata + .rodata -> p_tbase / p_tlen. */
    text PT_LOAD FLAGS(5);
    /* RW-: (.init_array +) .dynamic + .got + .data, .bss as the NOLOAD tail
       -> p_dbase / p_dlen, p_bbase / p_blen. */
    data PT_LOAD FLAGS(6);
    /* The dynamic array, inside `data`: where the loader finds DT_RELA. */
    dynamic PT_DYNAMIC FLAGS(6);
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

    /* Read-only dynamic metadata lld emits for a PIE. Only .rela.dyn is read
       by the loader; the symbol/hash tables are what lld insists on
       creating for -pie (one null symbol). */
    .dynsym   : { *(.dynsym) }   :text
    .gnu.hash : { *(.gnu.hash) } :text
    .hash     : { *(.hash) }     :text
    .dynstr   : { *(.dynstr) }   :text
    .rela.dyn : { *(.rela.dyn) *(.rela.*) } :text

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

    /* The dynamic array (also the PT_DYNAMIC segment) and the GOT, ahead of
       .data so .data's tail ALIGN still ends the file-backed part. */
    .dynamic : { *(.dynamic) } :data :dynamic
    .got     : { *(.got) *(.igot) } :data
    .got.plt : { *(.got.plt) *(.igot.plt) } :data

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
    }
}
