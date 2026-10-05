/*
 * vfork.S -- NewTOS/AArch64 retarget of mintlib's unix/vfork.S (m68k, never
 * assembled here; TASK/PROC01, ARCH03 stage 2e). Same two branches, same
 * contract:
 *
 *   * MiNT (`__mint != 0`): Pvfork (GEMDOS 0x113) -- the kernel returns in
 *     the child (0) and in the parent (pid). Not taken on NewTOS, which keeps
 *     `__mint` at 0 (mintlib/globals.c) and answers 0x113 with EINVFN; kept
 *     so the file is the port of the whole m68k routine.
 *   * TOS: tfork(newprog, save) (mintlib/thread.c) = Pexec(5) + Pexec(4) on
 *     code of this program, in the caller's address space. The child runs
 *     `.Lnewprog`, which restores the registers vfork was called with and
 *     returns 0 into vfork's caller -- the child continues there, on the
 *     caller's stack, while the parent sleeps in Pexec(4) (TOS Pexec is
 *     synchronous); when the child ends, tfork returns its pid to the
 *     parent, which restores the same registers and returns the pid.
 *
 * What the retarget changes, and why:
 *   * m68k vfork pops its return address (into a1) because the child would
 *     otherwise overwrite it on the shared stack. AArch64 returns through
 *     x30, which is saved with the other callee-saved registers (x19-x30,
 *     d8-d15, sp) -- the m68k routine saves d2-d7/a1-a6.
 *   * m68k code reaches its locals through the frame pointer a6, so the
 *     child could run its continuation on any stack. AArch64 code addresses
 *     locals through sp, so the child has to resume on the caller's sp. For
 *     the child to be free to use the stack below that sp, the parent's own
 *     call of tfork runs on a separate stack (`vfork_stack`): nothing of the
 *     parent lives below the caller's sp while the child runs.
 *   * Like the m68k version (a static `L_vfsav`), the save area and the
 *     separate stack are static: a child that calls vfork again before it
 *     exec's or exits is not supported (the original has the same limit).
 *
 * Errors: a negative tfork/Pvfork result -e sets errno = e and returns -1.
 * Trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1, id in
 * x8, result in x0.
 */

#define GEMDOS_PVFORK       0x113

#define VF_X19      0
#define VF_X21      16
#define VF_X23      32
#define VF_X25      48
#define VF_X27      64
#define VF_X29      80
#define VF_SP       96
#define VF_D8       104
#define VF_D10      120
#define VF_D12      136
#define VF_D14      152
#define VF_SIZE     168

#define VF_STACK    16384

    .macro ADR_GLOBAL reg, name
    adrp    \reg, \name
    add     \reg, \reg, :lo12:\name
    .endm

    .macro SAVE_REGS base
    stp     x19, x20, [\base, #VF_X19]
    stp     x21, x22, [\base, #VF_X21]
    stp     x23, x24, [\base, #VF_X23]
    stp     x25, x26, [\base, #VF_X25]
    stp     x27, x28, [\base, #VF_X27]
    stp     x29, x30, [\base, #VF_X29]
    mov     x10, sp
    str     x10, [\base, #VF_SP]
    stp     d8, d9,   [\base, #VF_D8]
    stp     d10, d11, [\base, #VF_D10]
    stp     d12, d13, [\base, #VF_D12]
    stp     d14, d15, [\base, #VF_D14]
    .endm

    .macro RESTORE_REGS base
    ldp     x19, x20, [\base, #VF_X19]
    ldp     x21, x22, [\base, #VF_X21]
    ldp     x23, x24, [\base, #VF_X23]
    ldp     x25, x26, [\base, #VF_X25]
    ldp     x27, x28, [\base, #VF_X27]
    ldp     x29, x30, [\base, #VF_X29]
    ldr     x10, [\base, #VF_SP]
    mov     sp, x10
    ldp     d8, d9,   [\base, #VF_D8]
    ldp     d10, d11, [\base, #VF_D10]
    ldp     d12, d13, [\base, #VF_D12]
    ldp     d14, d15, [\base, #VF_D14]
    .endm

    .lcomm  vfork_save, VF_SIZE
    .lcomm  vfork_stack, VF_STACK

    .text
    .balign 4

    .globl  vfork
    .type   vfork, %function
    .globl  __vfork
    .type   __vfork, %function
vfork:
__vfork:
    ADR_GLOBAL x9, __mint
    ldr     w9, [x9]
    cbz     w9, .Ltos                   /* go do the TOS thing               */
    mov     x8, #GEMDOS_PVFORK          /* MiNT Pvfork()                     */
    svc     #1
    cmp     x0, #0
    b.lt    .Lerr
    ret

.Ltos:
    ADR_GLOBAL x9, vfork_save
    SAVE_REGS x9                        /* save registers (x30 = our return) */
    ADR_GLOBAL x10, vfork_stack
    add     x10, x10, #VF_STACK
    and     sp, x10, #~15               /* the parent's tfork: its own stack */
    adr     x0, .Lnewprog
    mov     x1, x9
    bl      tfork                       /* tfork(newprog, save)              */
    ADR_GLOBAL x9, vfork_save
    RESTORE_REGS x9                     /* restore registers and our sp      */
    cmp     x0, #0                      /* fork went OK?                     */
    b.lt    .Lerr                       /* no -- error                       */
    ret                                 /* return the pid to the caller      */

.Lerr:
    neg     w0, w0
    ADR_GLOBAL x9, errno
    str     w0, [x9]                    /* save error code in errno          */
    mov     x0, #-1                     /* return -1                         */
    ret

/*
 * .Lnewprog: here is where the child starts executing (tfork's startup calls
 * it as func(arg), arg = the save area in x0). Restore the registers, sp
 * included, return 0 -- back to vfork's caller, as the child.
 */
.Lnewprog:
    mov     x9, x0
    RESTORE_REGS x9
    mov     x0, #0                      /* child always returns 0 from vfork */
    ret
    .size   vfork, . - vfork
    .size   __vfork, . - __vfork
