/*
 * crt0.S -- NewTOS C runtime startup for libc-linked programs (`.NCP`).
 *
 * This is the libc variant of sdk/crt0.S: identical TPA/SP setup, plus one
 * extra step -- it calls __libc_init_heap(p_hitpa) before main so mintlib's
 * malloc arena is pointed at the free TPA region (see
 * sdk/libc/newtos-arch/heap_init.c). Programs that do not link libc keep
 * using the plain sdk/crt0.S, which has no such dependency.
 *
 * TPA layout (tos-api/src/bdos/process/loader.rs), low-half VA 0x0:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x100, image_end)      PT_LOAD image (.text/.rodata/.data)
 *     [image_end, __bss_end)  .bss (zeroed by the loader)
 *     [__bss_end, p_hitpa)    free TPA: heap grows up, stack grows down
 *
 * On entry the loader has eret'd to EL0 at _start (linked at 0x100) and set
 * SP_EL0 to the TPA top. We re-establish SP from the BasePage's p_hitpa
 * (32-bit LONG at basepage offset 0x04, read through a zero base register)
 * so we do not depend on the entry SP or x0.
 *
 * Trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, arguments in x0.., result in x0. Pterm = 0x4C.
 */

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    /* SP = basepage->p_hitpa (the mapped TPA top), read at VA 0 offset 4. */
    mov     x1, #0
    ldr     w9, [x1, #4]        /* w9 = p_hitpa (zero-extended into x9) */
    mov     sp, x9

    /* Initialise the libc heap arena: __libc_init_heap(p_hitpa). p_hitpa is
       in x9; pass it in x0. The call needs a valid SP (set above). */
    mov     x0, x9
    bl      __libc_init_heap

    /* Set up argc/argv for main(int argc, char **argv).
     *
     * The launcher (kernel enter_el0_with_resume_capture) sets only ELR /
     * SP_EL0 / the TTBR0 tables — it does NOT pass argc/argv in registers, and
     * mintlib's crtinit.c parseargs() (which would build argv from the
     * BasePage command-line tail p_cmdlin) is m68k assembly that does not
     * assemble for AArch64, so it is never linked. Without this, main()
     * inherits garbage: argc = the leftover x0 from the void __libc_init_heap
     * (~4 MiB, i.e. >= 2) and argv = 0, so `if (argc > 1) ... argv[1]` faults
     * dereferencing (char**)0 + 1.
     *
     * NewTOS launches GEM apps from the desktop with no command-line
     * arguments, so the correct value is argc == 1 with a NUL-terminated argv
     * whose argv[0] is "" — the TOS "program name not available" convention
     * (matching mintlib crtinit.c's `*arg++ = ""`). main's own return value
     * (w0) is the exit code. */
    mov     x0, #1                      /* argc = 1 */
    adrp    x1, __crt0_argv
    add     x1, x1, :lo12:__crt0_argv   /* argv = { "", NULL } */
    bl      main

    /* Pterm(main's return): GEMDOS 0x4C, rc in x0, svc #1. Never returns. */
    mov     x8, #0x4C
    svc     #1

    /* Safety net: Pterm does not return, but guard against a runaway PC. */
1:  b       1b
    .size   _start, . - _start

    /* Minimal argv for a no-argument native launch: { "", NULL }. The empty
     * argv[0] signals "program name not available" (TOS convention). Pointers
     * are 8-byte (LP64) — the C program's `char **argv` ABI; the linker
     * resolves the .quad relocations to absolute VAs (static, no-PIE). */
    .section .data.crt0, "aw", @progbits
    .align  3
    .global __crt0_argv
__crt0_argv:
    .quad   __crt0_argv0                /* argv[0] = "" */
    .quad   0                           /* argv[1] = NULL (terminator) */
__crt0_argv0:
    .byte   0                           /* the empty string "" */
