/*
 * crt0-ilp32.S -- NewTOS C runtime startup for ILP32 libc-linked programs
 * (`.NCP`, ADR-020 Stage 2). The ILP32 sibling of sdk/libc/crt0.S.
 *
 * Identical logic to the LP64 crt0 (set SP from the BasePage p_hitpa, init the
 * libc heap, build a { "", NULL } argv, call main, then Pterm(main's rc)); the
 * ONLY difference is the width of the argv pointer table: on ILP32
 * `char **argv` is a table of 4-byte pointers, so __crt0_argv uses `.word`
 * (4-byte) entries aligned to 4, not `.quad`/8 as in the LP64 crt0. `.word sym`
 * emits an R_AARCH64_P32_ABS32 that `aarch64-elf-ld -m aarch64elf32` resolves
 * statically (no-PIE). The heap-init call (commit 530d3a4 argc/argv fix) and the
 * empty-argv[0] "program name unavailable" TOS convention are carried over
 * unchanged.
 *
 * TPA layout (tos-api/src/bdos/process/loader.rs), low-half VA 0x0:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x100, image_end)      PT_LOAD image (.text/.rodata/.data)
 *     [image_end, __bss_end)  .bss (zeroed by the loader)
 *     [__bss_end, p_hitpa)    free TPA: heap grows up, stack grows down
 *
 * Trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, arguments in x0.., result in x0. Pterm = 0x4C.
 */

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    /* SP = basepage->p_hitpa (the mapped TPA top), a 32-bit LONG at basepage
       offset 4, read through a zero base register (no dependency on entry SP
       or x0). */
    mov     x1, #0
    ldr     w9, [x1, #4]        /* w9 = p_hitpa (zero-extended into x9) */
    mov     sp, x9

    /* Initialise the libc heap arena: __libc_init_heap(p_hitpa). p_hitpa is in
       x9; pass it in x0. The call needs a valid SP (set above). */
    mov     x0, x9
    bl      __libc_init_heap

    /* argc = 1, argv = { "", NULL } (see LP64 crt0 for the rationale: the
       kernel launcher passes no argc/argv, and mintlib's m68k parseargs never
       links). On ILP32 the argv table is 4-byte pointers. */
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

    /* Minimal argv for a no-argument native launch: { "", NULL }. ILP32
       pointers are 4-byte, so the table is `.word` aligned to 4; the linker
       resolves the ABS32 relocations to absolute VAs (static, no-PIE). */
    .section .data.crt0, "aw", @progbits
    .align  2
    .global __crt0_argv
__crt0_argv:
    .word   __crt0_argv0                /* argv[0] = "" */
    .word   0                           /* argv[1] = NULL (terminator) */
__crt0_argv0:
    .byte   0                           /* the empty string "" */
