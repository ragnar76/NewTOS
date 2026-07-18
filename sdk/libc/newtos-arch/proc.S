/*
 * proc.S -- NewTOS/AArch64 arch shim for the low-level process-exit entry.
 *
 * mintlib provides _exit only as m68k assembly (mintlib/libc_exit.S, which
 * also does m68k profiling teardown via __mcleanup). On NewTOS _exit is just
 * the GEMDOS Pterm trap, so we reimplement it for AArch64 with the same public
 * symbols the m68k version exported:
 *
 *   libc_exit.S: __libc_exit, and weak aliases _exit, _Exit
 *
 * Trap ABI (kernel/src/arch/aarch64/exceptions.rs, matching sdk/crt0.S):
 * GEMDOS = svc #1, function id in x8, argument in x0. Pterm = 0x4C, taking the
 * exit code in x0. Pterm does not return.
 */

    .section .text, "ax", @progbits

    .global __libc_exit
    .type   __libc_exit, %function
    .global _exit
    .type   _exit, %function
    .global _Exit
    .type   _Exit, %function
__libc_exit:
_exit:
_Exit:
    /* w0 already holds the exit status (int); Pterm takes it in x0. */
    mov     x8, #0x4C
    svc     #1
    /* Pterm never returns; guard against a runaway PC. */
1:  b       1b
    .size   __libc_exit, . - __libc_exit
