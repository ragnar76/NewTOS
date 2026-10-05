/*
 * crt0.S -- NewTOS C runtime startup for libc-linked programs (`.NCP`).
 *
 * This is the libc variant of sdk/crt0.S: identical TPA/SP setup, plus the
 * two steps mintlib's own startup (startup/crt0.S + crtinit.c, neither of
 * which builds for AArch64) would take before main:
 *
 *   __libc_init_heap(basepage, p_hitpa)  -- sets mintlib's `_base` (the
 *       BASEPAGE pointer mintlib's crt0 stores at start, startup/crt0.S:69)
 *       and points the malloc arena at the free TPA region
 *       (sdk/libc/newtos-arch/heap_init.c);
 *   __libc_parseargs(basepage)           -- builds argc/argv from the
 *       BasePage command tail p_cmdlin, the argument half of crtinit.c's
 *       parseargs() (sdk/libc/newtos-arch/cmdargs.c).
 *
 * Programs that do not link libc keep using the plain sdk/crt0.S, which has
 * no such dependency.
 *
 * TPA layout (tos-api/src/bdos/process/loader.rs, sdk/newtos.ld): the program
 * is a position-independent executable (ELF ET_DYN, ARCH03 stage 0d) linked
 * at 0x1000; the loader streams it into the TPA and applies its
 * R_AARCH64_RELATIVE relocations for the TPA's own pool address B (ARCH03
 * stage 1, VA = PA). Relative to B:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x1000, p_dbase)       PT_LOAD text (.text/.rodata/.rela.dyn) = p_tbase/p_tlen
 *     [p_dbase, p_bbase)      PT_LOAD data (.dynamic/.got/.data)      = p_dbase/p_dlen
 *     [p_bbase, _end)         .bss (zeroed by the loader)             = p_bbase/p_blen
 *     [_end, p_hitpa)         free TPA: heap grows up, stack grows down
 *
 * On entry the loader has eret'd to EL0 at _start (linked at 0x1000), set
 * SP_EL0 to the TPA top and put the BasePage address in x0 (every launch
 * path: Pexec, the kernel launcher, the scheduler's first dispatch). We take
 * the BasePage from x0 -- never from a fixed address, because from stage 1 on
 * it sits at the start of the program's own TPA block -- and re-establish SP
 * from its p_hitpa (32-bit LONG at offset 0x04) rather than trust the entry
 * SP.
 *
 * Trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, arguments in x0.., result in x0. Pterm = 0x4C.
 */

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    /* SP = basepage->p_hitpa (the mapped TPA top), offset 4 of the BasePage
       the loader passes in x0. x19/x20 keep the two values across the calls
       below; nothing has to be restored, because _start never returns. */
    mov     x19, x0             /* x19 = BasePage address (loader: x0) */
    ldr     w20, [x19, #4]      /* w20 = p_hitpa (zero-extended into x20) */
    mov     sp, x20

    /* __libc_init_heap(basepage, p_hitpa) — sets mintlib's `_base` and points
       the malloc arena at the free TPA region. Needs a valid SP (set above). */
    mov     x0, x19
    mov     x1, x20
    bl      __libc_init_heap

    /* Set up argc/argv for main(int argc, char **argv).
     *
     * The launcher (kernel enter_el0_with_resume_capture) sets only ELR /
     * SP_EL0 / the TTBR0 tables — it does NOT pass argc/argv in registers, so
     * without this main() would inherit garbage: argc = the leftover x0 from
     * the void __libc_init_heap (~4 MiB, i.e. >= 2) and argv = 0, so
     * `if (argc > 1) ... argv[1]` faults dereferencing (char**)0 + 1.
     *
     * A program launched with an argument — a file dragged onto its icon or
     * onto a desktop shortcut — gets that argument in the BasePage command
     * tail p_cmdlin. mintlib builds argv from it in crtinit.c parseargs(),
     * which is not compiled here (m68k asm elsewhere in the same file); the
     * argument half of it is reimplemented in
     * sdk/libc/newtos-arch/cmdargs.c as __libc_parseargs(), which publishes
     * __libc_argc / __libc_argv and returns argc. With an empty tail that is
     * argc == 1 and argv == { "", NULL } — the TOS "program name not
     * available" convention (crtinit.c's `*arg++ = ""`), i.e. exactly what
     * this crt0 used to hardcode. main's own return value (w0) is the exit
     * code. */
    mov     x0, x19
    bl      __libc_parseargs            /* x0 = argc */
    adrp    x1, __libc_argv
    add     x1, x1, :lo12:__libc_argv
    ldr     x1, [x1]                    /* x1 = argv */
    bl      main

    /* Pterm(main's return): GEMDOS 0x4C, rc in x0, svc #1. Never returns. */
    mov     x8, #0x4C
    svc     #1

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
