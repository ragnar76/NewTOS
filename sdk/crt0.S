/*
 * crt0.S -- NewTOS native C runtime startup (ADR-018 `.NCP` / `.NAP`).
 *
 * The NewTOS program loader (tos-api/src/bdos/process/{loader,reloc,mmu,
 * arena}.rs) maps a multi-page Transient Program Area (TPA_SIZE, 4 MiB today)
 * for the process. The program is a position-independent executable (ELF
 * ET_DYN, ARCH03 stage 0d): it is linked at 0x1000 (sdk/newtos.ld), the
 * loader streams its segments into the TPA and applies its
 * R_AARCH64_RELATIVE relocations for the address the TPA executes at -- since
 * ARCH03 stage 1 its own pool address B (VA = PA). Relative to B:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x1000, p_dbase)       PT_LOAD text (.text/.rodata/.rela.dyn) = p_tbase/p_tlen
 *     [p_dbase, p_bbase)      PT_LOAD data (.dynamic/.got/.data)      = p_dbase/p_dlen
 *     [p_bbase, _end)         .bss (zeroed by the loader)             = p_bbase/p_blen
 *     [_end, p_hitpa)         free TPA: heap grows up, stack grows down
 *
 * Every TPA page is mapped user-read/write/execute. On entry the loader has:
 *
 *   - eret'd to EL0 at e_entry (this `_start`, linked at 0x1000);
 *   - set SP_EL0 to the mapped TPA top (= the basepage's p_hitpa);
 *   - put the BasePage address in x0 -- on every launch path (Pexec, the
 *     kernel launcher for the desktop / AUTO / #Z, the scheduler's first
 *     dispatch of an accessory).
 *
 * crt0 therefore finds the BasePage through x0, never through a fixed
 * address: from stage 1 on the basepage sits at the start of the program's
 * own TPA block, wherever that is. SP is (re)established from the basepage's
 * `p_hitpa` (a 32-bit TOS `LONG` at offset 0x04, 4-KiB / 16-byte aligned)
 * rather than trusted from entry, so it tracks whatever TPA top the loader
 * chose. The loader zeroes the whole TPA (including .bss), so crt0 does not
 * zero BSS.
 *
 * The trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, argument in x0, result in x0. Pterm = 0x4C.
 */

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    /*
     * SP = basepage->p_hitpa (the mapped TPA top), read at offset 0x04 of the
     * BasePage the loader passes in x0. No stack access before SP is valid.
     */
    ldr     w9, [x0, #4]        /* w9 = p_hitpa (zero-extended into x9) */
    mov     sp, x9

    /*
     * `main` takes no arguments in a plain (non-libc) program; x0 still
     * holds the BasePage address, which it ignores. Its return value (w0)
     * is the exit code.
     */
    bl      main

    /* Pterm(main's return): GEMDOS 0x4C, rc in x0, svc #1. Never returns. */
    mov     x8, #0x4C
    svc     #1

    /* Safety net: Pterm does not return, but guard against a runaway PC. */
1:  b       1b
    .size   _start, . - _start
