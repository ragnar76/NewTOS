/*
 * a__appl_yield.c -- gemlib _appl_yield() (fast AES process-switch shortcut),
 * retargeted for NewTOS/AArch64.
 *
 * Pristine m68k: a direct GEM trap with the fast-appl_yield selector, bypassing
 * the AESPB:
 *      move.w  #201,d0
 *      trap    #2
 *
 * This is the ONLY binding that issues the GEM trap directly (all others go
 * through the AES_TRAP/VDI_TRAP macros -> aes()/vdi()); it is retargeted here to
 * `svc #2` with the same selector (201) in x0, faithfully -- no stub, no
 * ignored argument (the function takes none).
 *
 * NOTE (deferred semantic surface): NewTOS's GEM dispatcher
 * (kernel/src/arch/aarch64/exceptions.rs) implements only selectors 115 (VDI)
 * and 200 (AES); the 201 fast-appl_yield shortcut is a no-op there, so
 * _appl_yield() is currently INERT on NewTOS. The regular, dispatched path
 * -- appl_yield()/mt_appl_yield(), AES opcode 17 (`17 => appl::appl_yield`) --
 * works and should be preferred. Implementing the 201 shortcut is a
 * kernel/tos-api decision, not a library one.
 */
#include "gem_aesP.h"

#ifdef __GNUC__
void _appl_yield(void)
{
	register long x0 __asm__("x0") = 201;		/* fast appl_yield selector */

	__asm__ __volatile__ (
		"svc	#2"
		: "+r"(x0)
		:
		: "memory", "cc"
	);
	(void)x0;
}
#endif
