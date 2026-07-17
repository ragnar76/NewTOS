/*
 * newtos.h -- NewTOS native C SDK, Stage 1 (ADR-018 `.NCP` console programs).
 *
 * CLEAN-ROOM. Every declaration here is written from the extracted TOS API
 * specification in `docs/api/` (which is itself derived from EmuTOS by the
 * Phase-00 source-analysis protocol). Nothing in this file is copied from
 * mintlib, gemlib, or any Atari header. The trap ABI is taken from the NewTOS
 * kernel dispatcher `kernel/src/arch/aarch64/exceptions.rs`.
 *
 * ---------------------------------------------------------------------------
 * Trap ABI (kernel/src/arch/aarch64/exceptions.rs, tos-api/src/trap.rs)
 * ---------------------------------------------------------------------------
 *   GEMDOS : svc #1     (TrapClass::Gemdos = 1)
 *   GEM    : svc #2     (VDI selector 115 / AES selector 200)
 *   BIOS   : svc #13    (TrapClass::Bios  = 13)
 *   XBIOS  : svc #14    (TrapClass::Xbios = 14)
 *
 *   - the function id goes in x8
 *   - integer / pointer arguments go in x0, x1, x2, ... (left to right)
 *   - the result is a sign-extended i32 returned in x0 (BIOS Getbpb returns a
 *     full 64-bit pointer; that is out of Stage-1 scope)
 *
 * The kernel SVC handler saves and restores x1..x29 across the trap and only
 * overwrites x0 with the result, so the inline-asm helpers below clobber
 * nothing but memory and the x0 result register.
 *
 * ---------------------------------------------------------------------------
 * Stage 1 scope
 * ---------------------------------------------------------------------------
 * The named wrappers below cover only the calls the Stage-1 "hello" program
 * needs, each backed by a spec entry in docs/api/:
 *
 *   Cconws  GEMDOS 0x09  docs/api/gemdos/console/09-cconws.md
 *   Cconin  GEMDOS 0x01  docs/api/gemdos/console/01-cconin.md
 *   Pterm   GEMDOS 0x4C  docs/api/gemdos/process/4c-pterm.md
 *   Pterm0  GEMDOS 0x00  docs/api/gemdos/process/00-pterm0.md
 *
 * The generic gemdos()/bios()/xbios() marshaling helpers implement the raw
 * trap ABI; named BIOS/XBIOS/AES/VDI wrappers arrive in Stage 2 (AES/VDI) and
 * Stage 3 (libc), each against its own spec entry.
 */
#ifndef NEWTOS_H
#define NEWTOS_H

/*
 * TOS `LONG` is the 32-bit machine word of the m68k. On AArch64 the kernel
 * returns a sign-extended i32 in x0, so a native `long` (64-bit) faithfully
 * holds it. Pointers are passed as full 64-bit values.
 */
typedef long NLONG;

/* ------------------------------------------------------------------------- */
/* Raw GEMDOS trap (svc #1): id in x8, args in x0.., result in x0.           */
/* ------------------------------------------------------------------------- */

static inline NLONG gemdos0(NLONG fn)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0");
    __asm__ volatile("svc #1"
                     : "=r"(r0)
                     : "r"(x8)
                     : "memory");
    return r0;
}

static inline NLONG gemdos1(NLONG fn, NLONG a0)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    __asm__ volatile("svc #1"
                     : "+r"(r0)
                     : "r"(x8)
                     : "memory");
    return r0;
}

static inline NLONG gemdos2(NLONG fn, NLONG a0, NLONG a1)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    register NLONG r1 __asm__("x1") = a1;
    __asm__ volatile("svc #1"
                     : "+r"(r0)
                     : "r"(x8), "r"(r1)
                     : "memory");
    return r0;
}

/* ------------------------------------------------------------------------- */
/* Raw BIOS trap (svc #13) and XBIOS trap (svc #14). Mechanism only for      */
/* Stage 1; named wrappers land with their spec entries in later stages.     */
/* ------------------------------------------------------------------------- */

static inline NLONG bios1(NLONG fn, NLONG a0)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    __asm__ volatile("svc #13"
                     : "+r"(r0)
                     : "r"(x8)
                     : "memory");
    return r0;
}

static inline NLONG bios2(NLONG fn, NLONG a0, NLONG a1)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    register NLONG r1 __asm__("x1") = a1;
    __asm__ volatile("svc #13"
                     : "+r"(r0)
                     : "r"(x8), "r"(r1)
                     : "memory");
    return r0;
}

static inline NLONG xbios1(NLONG fn, NLONG a0)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    __asm__ volatile("svc #14"
                     : "+r"(r0)
                     : "r"(x8)
                     : "memory");
    return r0;
}

/* ------------------------------------------------------------------------- */
/* Named GEMDOS wrappers (Stage 1).                                          */
/* ------------------------------------------------------------------------- */

/*
 * Cconws (GEMDOS 0x09) -- write a NUL-terminated string to the console with
 * tab expansion. NewTOS returns the byte count actually written (deliberate
 * divergence D-009; EmuTOS's `xconws` is void).
 * Spec: docs/api/gemdos/console/09-cconws.md
 */
static inline NLONG Cconws(const char *s)
{
    return gemdos1(0x09, (NLONG)s);
}

/*
 * Cconin (GEMDOS 0x01) -- read one character from the console, blocking.
 * Returns a packed LONG: bits 0-7 ASCII (0 for non-ASCII keys), bits 16-23
 * the IKBD scancode. Echoes the character. A typed Ctrl-C terminates the
 * process and does not return.
 * Spec: docs/api/gemdos/console/01-cconin.md
 */
static inline NLONG Cconin(void)
{
    return gemdos0(0x01);
}

/*
 * Pterm (GEMDOS 0x4C) -- terminate the process, handing the 16-bit exit code
 * `rc` back to the parent's Pexec (zero-extended into the LONG). Does not
 * return.
 * Spec: docs/api/gemdos/process/4c-pterm.md
 */
__attribute__((noreturn)) static inline void Pterm(int rc)
{
    gemdos1(0x4C, (NLONG)rc);
    __builtin_unreachable();
}

/*
 * Pterm0 (GEMDOS 0x00) -- terminate with exit code 0 (Pterm(0)). Does not
 * return.
 * Spec: docs/api/gemdos/process/00-pterm0.md
 */
__attribute__((noreturn)) static inline void Pterm0(void)
{
    gemdos0(0x00);
    __builtin_unreachable();
}

#endif /* NEWTOS_H */
