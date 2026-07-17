/*
 * crt0.S -- NewTOS Stage-1 C runtime startup (ADR-018 `.NCP`).
 *
 * The NewTOS program loader (tos-api/src/bdos/process/{loader,mmu,arena}.rs)
 * maps exactly ONE 4 KiB page for the process, at low-half VA 0x0:
 *
 *     [0x000, 0x100)  BasePage (populated by the loader)
 *     [0x100, 0x1000) PT_LOAD image (.text/.rodata/.data/.bss) + the C stack
 *
 * The whole page is mapped user-read/write/execute, so the stack may live in
 * it. On entry the loader has:
 *
 *   - eret'd to EL0 at e_entry (this `_start`, linked at 0x100);
 *   - put the BasePage VA (0x0) in x0 (the NewTOS AArch64 process ABI);
 *   - set SP_EL0 to USER_STACK_TOP_EXCLUSIVE (0x0000_FFFF_FFFF_F000).
 *
 * That default stack top is NOT mapped under the current single-page loader
 * (mmu::map_page only installs the VA-0x0 page), so we MUST repoint SP into
 * the mapped TPA page before the first stack access. We set SP to 0x1000, the
 * exclusive top of the page; the stack then grows downward within
 * [0x100, 0x1000). A Stage-1 program's `main` uses only a few dozen bytes of
 * stack, well clear of its (tiny) code+data.
 *
 * The trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, argument in x0, result in x0. Pterm = 0x4C.
 */

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    /* Point SP at the top of the mapped TPA page (grows down within it). */
    mov     x9, #0x1000
    mov     sp, x9

    /*
     * x0 held the BasePage VA on entry; Stage-1 `main` takes no arguments,
     * so we discard it. Call main; its return value (w0) is the exit code.
     */
    bl      main

    /* Pterm(main's return): GEMDOS 0x4C, rc in x0, svc #1. Never returns. */
    mov     x8, #0x4C
    svc     #1

    /* Safety net: Pterm does not return, but guard against a runaway PC. */
1:  b       1b
    .size   _start, . - _start
