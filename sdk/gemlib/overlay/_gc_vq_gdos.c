/*
 * _gc_vq_gdos.c -- gemlib vq_gdos() (GDOS presence probe), retargeted for
 * NewTOS/AArch64.
 *
 * Pristine m68k:
 *      moveq   #-2,d0
 *      trap    #2
 *      cmp.w   #-2,d0 ; sne d0 ; ext.w d0   -> 1 if GDOS present, else 0
 *
 * This is a faithful transliteration of that primitive to the NewTOS register
 * GEM trap (`svc #2`, selector in x0): it issues the real trap with the -2
 * probe selector and derives the result from the returned x0 -- no hardcoded
 * value, no ignored input (the function takes none).
 *
 * NOTE: NewTOS's GEM dispatcher (kernel/src/arch/aarch64/exceptions.rs)
 * dispatches selectors 115 (VDI) and 200 (AES); for every other selector --
 * including this -2 GDOS probe -- it returns x0 unchanged, exactly like
 * EmuTOS' `_vditrap`, whose non-VDI path is a bare `rte`
 * (references/emutos/vdi/vdi_asm.S:40-49: "Just like Atari TOS, we leave d0
 * unchanged, indicating the absence of GDOS if this is called with -2 in the
 * low-order word of d0."). The classic `(short)x0 != -2` test therefore
 * evaluates false and this reports "no GDOS", the correct answer on a system
 * without GDOS. (Before the V16 repair, 2026-09-12, the dispatcher returned a
 * hardwired 0 here and this function wrongly reported "GDOS present".)
 */
#include "gem_vdiP.h"

#ifdef __GNUC__
short vq_gdos(void)
{
	register long x0 __asm__("x0") = -2;		/* GDOS probe selector */

	__asm__ __volatile__ (
		"svc	#2"
		: "+r"(x0)
		:
		: "memory", "cc"
	);
	return (short)x0 != -2;
}
#endif
