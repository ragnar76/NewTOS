/*
 * crt0.S -- NewTOS C runtime startup for libc-linked programs (`.NCP`/`.NAP`).
 *
 * This is the libc variant of sdk/crt0.S. mintlib's own startup
 * (startup/crt0.S + mintlib/crtinit.c) does not build for AArch64 (m68k
 * assembly); this file is its startup/crt0.S and newtos-arch/heap_init.c
 * (`__libc_crtinit`) its crtinit.c `_crtinit` / `_acc_main`:
 *
 *   - the TPA is the largest free block of the pool the PRGFLAGS choose
 *     (EmuTOS alloc_tpa, bdos/proc.c:460-502; ARCH03 E9);
 *   - `__libc_crtinit` sets `_base`, builds environ and argc/argv
 *     (`__libc_parseargs`, newtos-arch/cmdargs.c), applies mintlib's `_stksize` / `_heapbase`
 *     policy, Mshrinks the TPA to program + stack (crtinit.c:154-247) and
 *     returns the new stack top; an accessory -- mintlib's test,
 *     `p_parent == 0` (startup/crt0.S:52-56) -- gets a Malloc'ed stack
 *     (crtinit.c:112-131) and keeps the TPA its loader already shrank
 *     (EmuTOS pgmld);
 *   - malloc then grows through GEMDOS Malloc (`_split_mem`, unix/sbrk.c),
 *     as with mintlib on TOS;
 *   - main's return value goes to exit(), as at the end of mintlib's
 *     `__libc_main`, so atexit handlers and the stdio flush run.
 *
 * TPA layout (tos-api/src/bdos/process/loader.rs, sdk/newtos.ld), relative
 * to the TPA's pool address B (ARCH03 stage 1, VA = PA):
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x100, 0x1000)         reserved (zeroed) -- the start-up stack below
 *     [0x1000, p_dbase)       PT_LOAD text = p_tbase/p_tlen
 *     [p_dbase, p_bbase)      PT_LOAD data = p_dbase/p_dlen
 *     [p_bbase, _end)         .bss (zeroed by the loader) = p_bbase/p_blen
 *     [_end, p_hitpa)         the rest of the largest free block
 *
 * The start-up stack. mintlib runs `_crtinit` on a temporary stack at
 * p_hitpa (startup/crt0.S:70-73) and an accessory's on its command line
 * (:59). Here one place serves both: the reserved gap above the BasePage,
 * which lies in the TPA a program keeps and in the image an accessory's
 * loader keeps, whatever the PRGFLAGS and whoever shrinks the block.
 *
 * On entry the loader has eret'd to EL0 at _start (linked at 0x1000) and put
 * the BasePage address in x0 (every launch path: Pexec, the kernel launcher,
 * the scheduler's first dispatch).
 *
 * Trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, arguments in x0.., result in x0. Pterm = 0x4C.
 */

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    mov     x19, x0             /* x19 = BasePage address (loader: x0) */
    add     x9, x19, #0x1000    /* start-up stack: top of the reserved gap */
    mov     sp, x9

    /* new SP = __libc_crtinit(basepage) -- the TPA is shrunk on return. */
    mov     x0, x19
    bl      __libc_crtinit
    mov     sp, x0

    /* exit(main(__libc_argc, __libc_argv, environ)) -- how mintlib's own
     * start-up ends (mintlib/main.c:302, the last statement of
     * `__libc_main`, which _crtinit reaches as `_main`, crtinit.c:257, and
     * an accessory's _acc_main too, :147): main's return value goes to
     * exit(), which runs the atexit handlers in reverse order and closes
     * (flushes) every stdio stream before `_exit` -- Pterm, newtos-arch/
     * proc.S (stdlib/exit.c:43-59). __libc_crtinit built `environ` from the
     * environment block p_env and argc/argv from its ARGV= part or else the
     * BasePage command tail p_cmdlin (newtos-arch/cmdargs.c = crtinit.c
     * parseargs()); with no ARGV= and an empty tail that is argc == 1 and
     * argv == { "", NULL } -- the TOS "program name not available"
     * convention.
     *
     * The rest of `__libc_main` (MiNT cookie, Pdomain, UNIXMODE, stdio
     * stream modes, stderr to the console, PATH conversion) is NOT run here,
     * as it was not before: with it, every write through stdout fails, since
     * its text-mode stream set-up seeks on the console handle and the NewTOS
     * Fseek answers EIHNDL there, where EmuTOS answers 0 for every handle
     * function but read/write on a character device (bdos/bdosmain.c:
     * 571-641). Reported (post-E9); calling `_main` waits for that fix. */
    adrp    x0, __libc_argc
    add     x0, x0, :lo12:__libc_argc
    ldr     x0, [x0]
    adrp    x1, __libc_argv
    add     x1, x1, :lo12:__libc_argv
    ldr     x1, [x1]
    adrp    x2, environ
    add     x2, x2, :lo12:environ
    ldr     x2, [x2]
    bl      main
    /* exit(main's return): w0 is already the argument. Never returns. */
    bl      exit

    /* Safety net: Pterm does not return, but guard against a runaway PC. */
1:  b       1b
    .size   _start, . - _start

/*
 * PRGFLAGS note (ARCH03 E8b; format: docs/api/gemdos/process/4b-pexec.md,
 * NewTOS notes, "PRGFLAGS of a NewTOS program"). A NewTOS program has no
 * PRG header, so its PRGFLAGS long travels in this ELF note: owner "NewTOS",
 * type 1, one little-endian u32. sdk/newtos.ld keeps the section in the text
 * segment and gives it a PT_NOTE header; the loader reads it into p_flags
 * and picks the TPA/environment pool from PF_TTRAMLOAD. Default PF_STANDARD
 * (7, = no note); a build overrides it with -DNEWTOS_PRGFLAGS=<value> when
 * assembling this file (the SDK build scripts pass $PRGFLAGS), or later with
 * `cargo xtask prgflags <file> <value>`.
 */
#ifndef NEWTOS_PRGFLAGS
#define NEWTOS_PRGFLAGS 0x7     /* PF_STANDARD = PF_FASTLOAD|PF_TTRAMLOAD|PF_TTRAMMEM */
#endif
    .section .note.newtos.prgflags, "a", %note
    .balign 4
    .long   7                   /* namesz: "NewTOS\0" */
    .long   4                   /* descsz: one u32 */
    .long   1                   /* type: NT_NEWTOS_PRGFLAGS */
    .asciz  "NewTOS"
    .balign 4
    .long   NEWTOS_PRGFLAGS
