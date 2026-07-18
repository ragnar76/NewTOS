/*
 * _gc_vq_vgdos.c -- gemlib vq_vgdos() (GDOS id / version probe), retargeted for
 * NewTOS/AArch64.
 *
 * Pristine m68k:
 *      moveq   #-2,d0
 *      trap    #2
 *      -> return (short)d0 == -2 ? (short)-2 : d0   (the GDOS id, or -2 if none)
 *
 * Faithful transliteration to the NewTOS register GEM trap (`svc #2`, selector
 * in x0): issue the real probe and derive the result from the returned x0.
 *
 * NOTE: as with vq_gdos(), NewTOS's GEM dispatcher does not implement a GDOS
 * return protocol -- selector -2 is a no-op that leaves x0 = 0, so this returns
 * 0. See _gc_vq_gdos.c for the full rationale. No stub: the trap is really
 * issued and the documented arithmetic applied to its result.
 */
#include "gem_vdiP.h"

#ifdef __GNUC__
long vq_vgdos(void)
{
	register long x0 __asm__("x0") = -2;

	__asm__ __volatile__ (
		"svc	#2"
		: "+r"(x0)
		:
		: "memory", "cc"
	);
	return (short)x0 == -2 ? (short)x0 : x0;
}
#endif
