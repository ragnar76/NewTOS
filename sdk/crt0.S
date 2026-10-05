/*
 * crt0.S -- NewTOS native C runtime startup (ADR-018 `.NCP` / `.NAP` / `.NAC`).
 *
 * The NewTOS program loader (tos-api/src/bdos/process/{loader,reloc,mmu,
 * arena}.rs) gives the process a Transient Program Area: the LARGEST free
 * block of the pool its PRGFLAGS choose, exactly as EmuTOS alloc_tpa does
 * (bdos/proc.c:460-502; ARCH03 E9). The program is a position-independent
 * executable (ELF ET_DYN, ARCH03 stage 0d) linked at 0x1000 (sdk/newtos.ld);
 * the loader streams its segments into the TPA and applies its
 * R_AARCH64_RELATIVE relocations for the TPA's own pool address B (VA = PA).
 * Relative to B:
 *
 *     [0x000, 0x100)          BasePage (populated by the loader)
 *     [0x100, 0x1000)         reserved (zeroed) -- our start-up stack
 *     [0x1000, p_dbase)       PT_LOAD text = p_tbase/p_tlen
 *     [p_dbase, p_bbase)      PT_LOAD data = p_dbase/p_dlen
 *     [p_bbase, _end)         .bss (zeroed by the loader) = p_bbase/p_blen
 *     [_end, p_hitpa)         the rest of the largest free block
 *
 * Like every TOS start-up code, this one gives back what it does not need,
 * so the program does not hold all of memory and can start others (mintlib
 * crtinit.c _crtinit, with its default _stksize = MINKEEP; this plain runtime
 * has no heap, so it keeps the image plus a MINKEEP stack):
 *
 *     m = (_end - B) + MINKEEP;  Mshrink(0, B, m);  SP = B + m
 *
 * A desk accessory arrives with its TPA already shrunk to the image: the AES
 * side loads it and Mshrinks it to "code+data+bss plus basepage" before
 * anything runs (EmuTOS pgmld, util/gemdos.c:29-47; NewTOS kernel/src/accs.rs).
 * Its p_hitpa still names the old TPA top (Mshrink never touches the
 * basepage), so the stack must not go there. mintlib tells an accessory by
 * its m68k entry registers (startup/crt0.S:46-57: a0 = basepage and
 * p_parent == 0); NewTOS passes the basepage in x0 on every launch path, so
 * this code tells it by that very state instead: the Mshrink that would keep
 * the stack cannot GROW the block (EGSBF, -67). Then it does what mintlib's
 * _acc_main does (crtinit.c:112-131): Malloc(MINKEEP) and run on that.
 *
 * Until the stack is chosen no stack is used at all; the svc handler runs on
 * the kernel stack. x0 carries the BasePage address on entry (the loader:
 * Pexec, the kernel launcher for the desktop / AUTO / #Z, the scheduler's
 * first dispatch of an accessory).
 *
 * The trap ABI (kernel/src/arch/aarch64/exceptions.rs): GEMDOS = svc #1,
 * function id in x8, argument in x0.., result in x0. Mshrink = 0x4A,
 * Malloc = 0x48, Pterm = 0x4C.
 */

/* mintlib stksiz.h: keep at least this much stack. */
#define MINKEEP     0x10000
#define EGSBF       (-67)
#define ENSMEM      (-39)

    .section .text.boot, "ax", @progbits
    .global _start
    .type   _start, %function
_start:
    mov     x19, x0             /* x19 = BasePage (B) */

    /* m = round16(_end - B) + MINKEEP */
    adrp    x20, _end
    add     x20, x20, :lo12:_end
    sub     x21, x20, x19
    add     x21, x21, #15
    and     x21, x21, #~15
    add     x21, x21, #MINKEEP

    /* Mshrink(0, B, m) */
    mov     x0, #0
    mov     x1, x19
    mov     x2, x21
    mov     x8, #0x4A
    svc     #1
    cmn     x0, #(-EGSBF)       /* rc == EGSBF? */
    b.eq    .Lacc

    /* A program: the stack is the top of the part of the TPA it keeps. */
    add     x9, x19, x21
    mov     sp, x9
    b       .Lmain

.Lacc:
    /* An accessory (TPA already shrunk by the loader): mintlib _acc_main,
       s = Malloc(_stksize); SP = s + _stksize. */
    mov     x0, #MINKEEP
    mov     x8, #0x48
    svc     #1
    cbz     x0, .Lnomem
    add     x9, x0, #MINKEEP
    and     x9, x9, #~15
    mov     sp, x9

.Lmain:
    /*
     * `main` takes no arguments in a plain (non-libc) program; x0 holds the
     * BasePage address, which it may ignore. Its return value (w0) is the
     * exit code.
     */
    mov     x0, x19
    bl      main

    /* Pterm(main's return): GEMDOS 0x4C, rc in x0, svc #1. Never returns. */
    mov     x8, #0x4C
    svc     #1

.Lnomem:
    mov     x0, #ENSMEM
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
