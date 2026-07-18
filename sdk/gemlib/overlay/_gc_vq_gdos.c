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
 * NOTE (deferred semantic surface): NewTOS's GEM dispatcher
 * (kernel/src/arch/aarch64/exceptions.rs) handles only selectors 115 (VDI) and
 * 200 (AES); any other selector (including the -2 GDOS probe) is a no-op that
 * leaves x0 = 0. NewTOS therefore does NOT implement a GDOS-presence return
 * protocol on the GEM trap. With x0 = 0 the classic `(short)x0 != -2` test
 * evaluates true, so this reports "GDOS present". A NewTOS-defined -2 return in
 * the kernel dispatcher would be required to make GDOS detection meaningful;
 * that is a kernel/tos-api decision, not a library one. gemlib's native VDI
 * font path does not depend on vq_gdos, and neither appl_* nor v_opnvwk/
 * v_clsvwk call it.
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
