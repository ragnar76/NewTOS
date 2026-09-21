/*
 * setjmp.S -- NewTOS/AArch64 retarget of mintlib's mintlib/setjmp.S (m68k).
 *
 * mintlib provides setjmp/longjmp only as m68k assembly, so it was one of the
 * 24 .S files build-libc.sh never assembled (docs/sdk/07-mintlib-limits.md
 * section 3; ADR-019 "setjmp (aarch64, when pulled)"). This is the AArch64
 * version, mirroring the m68k control flow instruction for instruction and
 * exporting the same four public symbols:
 *
 *     sigsetjmp(env, savemask)   setjmp(env)
 *     siglongjmp(env, val)       longjmp(env, val)
 *
 * mintlib's <setjmp.h> maps _setjmp()/_longjmp() and the setjmp() macro onto
 * these (setjmp(jb) is sigsetjmp(jb, 1) under _GNU_SOURCE/__USE_POSIX), so no
 * further symbols exist on m68k either.
 *
 * jmp_buf layout (typedef long jmp_buf[22], 176 bytes -- overlay
 * include/setjmp.h; the m68k buffer is long[14]: retaddr, d2-d7/a2-a7, sigmask).
 * AAPCS64 callee-saved state is x19-x28, the frame pointer x29, the link
 * register x30 (the return address the m68k slot 0 holds), sp, and the low
 * 64 bits of v8-v15 (d8-d15):
 *
 *     byte  slot  contents
 *       0    0    x19          8    1    x20
 *      16    2    x21         24    3    x22
 *      32    4    x23         40    5    x24
 *      48    6    x25         56    7    x26
 *      64    8    x27         72    9    x28
 *      80   10    x29 (fp)    88   11    x30 (lr)
 *      96   12    sp
 *     104   13    d8         112   14    d9
 *     120   15    d10        128   16    d11
 *     136   17    d12        144   18    d13
 *     152   19    d14        160   20    d15
 *     168   21    saved signal mask (the m68k slot 13, offset 52)
 *
 * Signal-mask semantics, exactly as the m68k source (mintlib/setjmp.S):
 *
 *   sigsetjmp(env, savemask): env[21] = savemask. If it is 0 the mask is not
 *     saved. Otherwise the mask is read from the libc's TOS-emulation variable
 *     _sigmask (signal/initsig.c), or -- when MiNT is active, i.e. the libc's
 *     __mint (mintlib/globals.c) is non-zero -- from Psigblock(0); bit 0 is
 *     set so the stored word is never 0 ("SIGNULL is unmaskable"), then it is
 *     stored in env[21].
 *   setjmp(env): env[21] = 0, then the common register save. Returns 0.
 *   siglongjmp/longjmp(env, val): if __mint is non-zero, Psigreturn() first
 *     (ignored by MiNT outside a signal handler). If env[21] is non-zero, bit 0
 *     is cleared and the mask written back to _sigmask, plus Psigsetmask(mask)
 *     when __mint is non-zero. Then the registers are restored and the
 *     sigsetjmp call returns val, or 1 if val is 0.
 *
 * On NewTOS __mint is 0 -- it is only ever written by mintlib/crtinit.c, which
 * does not compile here (sdk/libc/expected-failures.txt) and would set it from
 * the MiNT cookie, which NewTOS does not publish -- so the three GEMDOS calls
 * (Psigblock 0x116, Psigsetmask 0x117, Psigreturn 0x11a) are never issued; the
 * paths are kept so the retarget stays a faithful mirror and needs no
 * revisiting if the libc ever runs on a MiNT-flavoured kernel. Trap ABI as in
 * proc.S: GEMDOS = svc #1, function id in x8, argument in x0, result in x0;
 * the kernel preserves x1..x29 across the trap (sdk/include/newtos.h), which
 * is what lets env and val stay in x13/x14 where the m68k code pushed a0/d0.
 *
 * The same source assembles for the macOS/arm64 host (sdk/libc/tests/
 * setjmp-host.sh): Mach-O wants a leading underscore, @PAGE/@PAGEOFF
 * relocations and no .type/.size -- the macros below select that.
 */

#ifdef __ELF__
# define SYM(name)              name
# define FUNC_BEGIN(name)       .type name, %function
# define FUNC_END(name)         .size name, . - name
#else /* Mach-O (macOS host build of the test) */
# define SYM(name)              _##name
# define FUNC_BEGIN(name)
# define FUNC_END(name)
#endif

/* ADR_GLOBAL reg, name -- reg = &name. An assembler macro, not a cpp one: on
 * Mach-O `;` starts a comment, so a two-instruction cpp macro would lose its
 * second half there. */
    .macro ADR_GLOBAL reg, name
#ifdef __ELF__
    adrp    \reg, \name
    add     \reg, \reg, :lo12:\name
#else
    adrp    \reg, _\name@PAGE
    add     \reg, \reg, _\name@PAGEOFF
#endif
    .endm

#define JB_X19      0
#define JB_X21      16
#define JB_X23      32
#define JB_X25      48
#define JB_X27      64
#define JB_X29      80
#define JB_SP       96
#define JB_D8       104
#define JB_D10      120
#define JB_D12      136
#define JB_D14      152
#define JB_SIGMASK  168

#define GEMDOS_PSIGBLOCK    0x116
#define GEMDOS_PSIGSETMASK  0x117
#define GEMDOS_PSIGRETURN   0x11a

    .text
    .balign 4

/* ---------------------------------------------------------------------------
 * int sigsetjmp(sigjmp_buf env, int savemask)      x0 = env, w1 = savemask
 * ------------------------------------------------------------------------ */
    .globl SYM(sigsetjmp)
    FUNC_BEGIN(SYM(sigsetjmp))
SYM(sigsetjmp):
    sxtw    x9, w1
    str     x9, [x0, #JB_SIGMASK]       /* save sigmask for siglongjmp?      */
    cbz     w1, .Lsetjmp_common         /* no -- common code                 */
    ADR_GLOBAL x9, _sigmask
    ldr     x10, [x9]                   /* tos emulation signal mask         */
    ADR_GLOBAL x11, __mint
    ldr     w12, [x11]                  /* see if MiNT is active             */
    cbz     w12, .Lnomint
    mov     x13, x0                     /* keep env across the trap          */
    mov     x0, #0                      /* add no signals to sigmask         */
    mov     x8, #GEMDOS_PSIGBLOCK       /* Psigblock(0) -> current mask      */
    svc     #1
    mov     x10, x0
    mov     x0, x13
.Lnomint:
    orr     x10, x10, #1                /* make it != 0 (SIGNULL unmaskable) */
    str     x10, [x0, #JB_SIGMASK]      /* save signal mask                  */
    b       .Lsetjmp_common
    FUNC_END(SYM(sigsetjmp))

/* ---------------------------------------------------------------------------
 * int setjmp(jmp_buf env)                           x0 = env
 * ------------------------------------------------------------------------ */
    .globl SYM(setjmp)
    FUNC_BEGIN(SYM(setjmp))
SYM(setjmp):
    str     xzr, [x0, #JB_SIGMASK]      /* do not restore sigmask on longjmp */
.Lsetjmp_common:
    stp     x19, x20, [x0, #JB_X19]
    stp     x21, x22, [x0, #JB_X21]
    stp     x23, x24, [x0, #JB_X23]
    stp     x25, x26, [x0, #JB_X25]
    stp     x27, x28, [x0, #JB_X27]
    stp     x29, x30, [x0, #JB_X29]     /* frame pointer + return address    */
    mov     x9, sp
    str     x9, [x0, #JB_SP]
    stp     d8, d9,   [x0, #JB_D8]
    stp     d10, d11, [x0, #JB_D10]
    stp     d12, d13, [x0, #JB_D12]
    stp     d14, d15, [x0, #JB_D14]
    mov     w0, #0                      /* return value is 0                 */
    ret
    FUNC_END(SYM(setjmp))

/* ---------------------------------------------------------------------------
 * void siglongjmp(sigjmp_buf env, int val)          x0 = env, w1 = val
 * void longjmp(jmp_buf env, int val)
 * ------------------------------------------------------------------------ */
    .globl SYM(siglongjmp)
    FUNC_BEGIN(SYM(siglongjmp))
    .globl SYM(longjmp)
    FUNC_BEGIN(SYM(longjmp))
SYM(siglongjmp):
SYM(longjmp):
    ADR_GLOBAL x11, __mint
    ldr     w12, [x11]                  /* see if MiNT is active             */
    cbz     w12, .Lno_sigreturn         /* no -- do not call sigreturn       */
    mov     x13, x0                     /* keep env, val across the trap     */
    mov     x14, x1
    mov     x8, #GEMDOS_PSIGRETURN      /* Psigreturn() (ignored if not in a */
    svc     #1                          /*   sig handler)                    */
    mov     x0, x13
    mov     x1, x14
.Lno_sigreturn:
    ldr     x10, [x0, #JB_SIGMASK]      /* want to restore sigmask?          */
    cbz     x10, .Lnorestore            /* no -- skip restore code           */
    and     x10, x10, #-2               /* andw #-2: drop the marker bit     */
    ADR_GLOBAL x9, _sigmask
    str     x10, [x9]                   /* restore tos emulation signal mask */
    ADR_GLOBAL x11, __mint
    ldr     w12, [x11]                  /* see if MiNT is active             */
    cbz     w12, .Lnorestore            /* no -- do not call sigsetmask      */
    mov     x13, x0
    mov     x14, x1
    mov     x0, x10                     /* Psigsetmask(mask)                 */
    mov     x8, #GEMDOS_PSIGSETMASK
    svc     #1
    mov     x0, x13
    mov     x1, x14
.Lnorestore:
    cmp     w1, #0
    csinc   w9, w1, wzr, ne             /* value to return: val, or 1 if 0   */
    ldp     x19, x20, [x0, #JB_X19]     /* restore saved registers           */
    ldp     x21, x22, [x0, #JB_X21]
    ldp     x23, x24, [x0, #JB_X23]
    ldp     x25, x26, [x0, #JB_X25]
    ldp     x27, x28, [x0, #JB_X27]
    ldp     x29, x30, [x0, #JB_X29]     /* ... and the saved return address  */
    ldr     x10, [x0, #JB_SP]
    mov     sp, x10
    ldp     d8, d9,   [x0, #JB_D8]
    ldp     d10, d11, [x0, #JB_D10]
    ldp     d12, d13, [x0, #JB_D12]
    ldp     d14, d15, [x0, #JB_D14]
    mov     w0, w9
    ret                                 /* returns from the sigsetjmp call   */
    FUNC_END(SYM(longjmp))
    FUNC_END(SYM(siglongjmp))
