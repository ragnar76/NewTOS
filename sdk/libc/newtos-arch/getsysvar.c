/*
 * getsysvar.c -- NewTOS/AArch64 replacement for mintlib's m68k getsysvar.S.
 *
 * mintlib declares (include/mint/sysvars.h:123)
 *
 *     long get_sysvar (const volatile void *var);
 *
 * and implements it in m68k assembly (mintlib/getsysvar.S): either
 * Ssystem(S_GETLVAL, var) under MiNT, or -- when running on plain TOS --
 * Super() + a bare `movel %a3@,%d0`, i.e. a supervisor read of the 32-bit
 * word at the classic low-memory address. The .S file cannot assemble for
 * AArch64, so build-libc.sh never compiles it; this file provides the symbol.
 *
 * On NewTOS the direct read IS the right implementation, and needs no
 * Super(): the classic system-variable area is materialised per process in
 * the program's own TPA, at exactly the classic addresses, and the page is
 * mapped user-readable (tos-api/src/bdos/process/loader.rs `populate_sysvars`,
 * docs/analysis/bios-audit/b1-lowmem-feasibility.md Option 1). So
 * `get_sysvar(_hz_200)` -- 0x4ba, the 200 Hz system tick the kernel's timer
 * interrupt keeps live (tos-api/src/bios/tick.rs) -- is a plain load.
 *
 * Width: a TOS system variable is a 32-bit LONG on the wire, and the m68k
 * original reads exactly 32 bits (`movel`). On LP64 `unsigned long` is eight
 * bytes, so the read is pinned to a fixed 32-bit type instead and widened
 * without sign extension -- these cells hold counters and low addresses, and
 * zero extension keeps a pointer-valued sysvar a valid address. In the ILP32
 * build both widths coincide, so the two ABIs agree.
 */

/* Fixed-width 32-bit type: `int` is 32 bits in both the LP64 and the ILP32
 * AArch64 ABIs we build for. <stdint.h> is not on the freestanding mintlib
 * include path used for this file. */
typedef unsigned int newtos_sysvar_t;

long
get_sysvar (const volatile void *var)
{
	return (long) *(const volatile newtos_sysvar_t *) var;
}
