/*
 * crt0.S -- NewTOS native C runtime startup (ADR-018 `.NCP` / `.NAP`).
 *
 * The NewTOS program loader (tos-api/src/bdos/process/{loader,mmu,arena}.rs)
 * maps a multi-page Transient Program Area (TPA_SIZE, 4 MiB today) for the
 * process, at low-half VA 0x0:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x100, image_end)      PT_LOAD image (.text/.rodata/.data, multi-page)
 *     [image_end, __bss_end)  .bss (zeroed by the loader)
 *     [__bss_end, p_hitpa)    free TPA: heap grows up, stack grows down
 *
 * Every TPA page is mapped user-read/write/execute. On entry the loader has:
 *
 *   - eret'd to EL0 at e_entry (this `_start`, linked at 0x100);
 *   - set SP_EL0 to the mapped TPA top (= the basepage's p_hitpa);
 *   - put the BasePage VA (0x0) in x0 on the SVC-Pexec launch path.
 *
 * We (re)establish SP from the basepage's `p_hitpa` field rather than trust
 * the entry SP: the basepage lives at a fixed VA 0, so we read p_hitpa
 * directly (independent of x0, which the kernel-launch path does not set),
 * and SP then tracks whatever TPA top the loader chose. p_hitpa is a 32-bit
 * TOS `LONG` at basepage offset 0x04; the loader guarantees it is 4-KiB /
 * 16-byte aligned. The loader zeroes the whole TPA (including .bss), so crt0
 * does not zero BSS.
 *
 * The trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, argument in x0, result in x0. Pterm = 0x4C.
 */

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    /*
     * SP = basepage->p_hitpa (the mapped TPA top). The basepage is always at
     * VA 0, so read offset 0x04 through a zero base register — no dependency
     * on x0, and no stack access before SP is valid.
     */
    mov     x1, #0
    ldr     w9, [x1, #4]        /* w9 = p_hitpa (zero-extended into x9) */
    mov     sp, x9

    /*
     * x0 holds the BasePage VA on the SVC-Pexec launch path; Stage-1 `main`
     * takes no arguments, so it is ignored. Call main; its return value (w0)
     * is the exit code.
     */
    bl      main

    /* Pterm(main's return): GEMDOS 0x4C, rc in x0, svc #1. Never returns. */
    mov     x8, #0x4C
    svc     #1

    /* Safety net: Pterm does not return, but guard against a runaway PC. */
1:  b       1b
    .size   _start, . - _start
