/*
 * setstack.S -- NewTOS/AArch64 retarget of mintlib's mintlib/setstack.S
 * (m68k, never assembled here; TASK/PROC01, ARCH03 stage 2e).
 *
 *     void _setstack(char *newsp)
 *
 * changes the stack pointer and returns to the caller on the new stack. As in
 * the m68k original, 64 bytes below newsp are left unused (its workaround for
 * trap handlers that read beyond the stack top); the m68k fake parameter for
 * the caller to pop has no AArch64 counterpart (arguments travel in
 * registers). sp is kept 16-byte aligned (AAPCS64). Same warning as the
 * original: after the call the caller's stack-based locals are gone -- the
 * user is mintlib/thread.c's startup(), which keeps what it needs in
 * registers.
 */
    .text
    .balign 4
    .globl  _setstack
    .type   _setstack, %function
_setstack:
    sub     x9, x0, #64
    and     sp, x9, #~15
    ret
    .size   _setstack, . - _setstack
