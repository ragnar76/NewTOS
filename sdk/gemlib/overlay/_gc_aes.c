/*
 * _gc_aes.c -- gemlib AES trap primitive, retargeted for NewTOS/AArch64.
 *
 * Pristine gemlib issued the classic Atari AES trap on the m68k:
 *      move.w  #200,d0        ; AES selector
 *      move.l  pb,d1          ; AES parameter block (AESPB *)
 *      trap    #2
 *
 * NewTOS uses a REGISTER svc ABI for the shared GEM trap, verified from the
 * kernel dispatcher:
 *   - kernel/src/arch/aarch64/exceptions.rs (handle_svc, GEM branch):
 *         TrapClass::Gem => match args.regs[0] {
 *             115 => vdi::dispatch(args.regs[1]),
 *             200 => aes::dispatch(args.regs[1]),
 *         }
 *     i.e. GEM = `svc #2`, x0 = selector (200 = AES), x1 = parameter block.
 *   - tos-api/src/aes/mod.rs (Aespb::from_raw): x1 points at the classic
 *     AESPB of six consecutive native pointers (control, global, intin,
 *     intout, addrin, addrout) -- exactly the LP64 `AESPB` struct layout.
 *   - The GEM trap returns nothing in a register (the handler leaves x0 = 0);
 *     all AES results come back through the parameter-block arrays.
 *
 * The kernel SVC entry saves/restores x1..x29 and only overwrites x0, so this
 * helper clobbers nothing but x0 and memory. The C signature/name is unchanged
 * so every a_*.c binding (via the AES_TRAP macro in gem_aesP.h) links to it.
 */
#include "gem_aesP.h"

#ifdef __GNUC__
void aes(AESPB *pb)
{
	register long x0 __asm__("x0") = 200;		/* AES selector */
	register long x1 __asm__("x1") = (long)pb;	/* &AESPB */

	__asm__ __volatile__ (
		"svc	#2"
		: "+r"(x0)		/* selector in, result (0) out */
		: "r"(x1)		/* parameter block */
		: "memory", "cc"
	);
	(void)x0;
}
#endif
