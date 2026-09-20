/*
 * _gc_vdi.c -- gemlib VDI trap primitive, retargeted for NewTOS/AArch64.
 *
 * Pristine gemlib issued the classic Atari VDI trap on the m68k:
 *      moveq   #115,d0        ; VDI selector (0x73)
 *      move.l  pb,d1          ; VDI parameter block (VDIPB *)
 *      trap    #2
 *
 * NewTOS uses a REGISTER svc ABI for the shared GEM trap, verified from the
 * kernel dispatcher:
 *   - kernel/src/arch/aarch64/exceptions.rs (handle_svc, GEM branch):
 *         115 => vdi::dispatch(args.regs[1])    // x0 = 115, x1 = block
 *     i.e. GEM = `svc #2`, x0 = selector (115 = VDI), x1 = parameter block.
 *   - tos-api/src/vdi/mod.rs (Vdipb::from_raw): x1 points at the five
 *     consecutive native pointers (contrl, intin, ptsin, intout, ptsout) --
 *     exactly the LP64 `VDIPB` struct layout. The dispatcher selects the
 *     function from contrl[0]; the caller has already filled contrl[] (see
 *     the VDI_TRAP_ESC C path in gem_vdiP.h).
 *   - The GEM trap returns `flip_y` (0/1) in `x0` (EmuTOS `vdi_asm.S:124-125`,
 *     zero-extended from `d0`; NewTOS kernel dispatcher,
 *     `kernel/src/arch/aarch64/exceptions.rs`, `115 =>
 *     i64::from(vdi::dispatch(...))`, corrected 2026-09-12 by the V16 fix,
 *     commit 675e311 — this comment used to say "handler leaves x0 = 0",
 *     which was true only before that fix). All other VDI results come back
 *     through the parameter-block arrays; this binding still discards `x0`
 *     (`(void)x0` below) because no current caller reads `flip_y` from the
 *     native trap.
 *
 * The kernel SVC entry saves/restores x1..x29 and only overwrites x0, so this
 * helper clobbers nothing but x0 and memory. The C signature/name is unchanged
 * so every v_*.c binding (via the VDI_TRAP macros in gem_vdiP.h) links to it.
 */
#include "gem_vdiP.h"

#ifdef __GNUC__
void vdi(VDIPB *pb)
{
	register long x0 __asm__("x0") = 115;		/* VDI selector */
	register long x1 __asm__("x1") = (long)pb;	/* &VDIPB */

	__asm__ __volatile__ (
		"svc	#2"
		: "+r"(x0)		/* selector in, flip_y (0/1) out */
		: "r"(x1)		/* parameter block */
		: "memory", "cc"
	);
	(void)x0;
}
#endif
