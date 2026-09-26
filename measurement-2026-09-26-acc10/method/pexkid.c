/*
 * pexkid.c -- PORT05 reproducer, CHILD half.
 *
 * The program PEXPAR.NAP launches with Pexec(PE_LOADGO) from EL0.  It is a GEM
 * application like QED: it appl_init()s and then BLOCKS in an AES wait, which
 * is the moment the cooperative scheduler looks for another runnable context.
 *
 * Nothing here is clever on purpose -- the point of the run is what the KERNEL
 * does while this program is blocked, not what this program computes.
 *
 * Two builds come out of this one file:
 *
 *   PEXKID.NAP   the faithful GEM application: appl_init ... appl_exit, Pterm.
 *   PEXKIDX.NAP  -DNO_APPL_EXIT, i.e. everything except the appl_exit().
 *
 * The pair exists because `appl_exit()` runs into a SEPARATE, PRE-EXISTING
 * defect that has nothing to do with the launch path under test: with desk
 * accessories loaded it posts AC_CLOSE and then blocks in
 * `wait_for_accs(AP_ACCLOSE)`, which sleeps on the WAIT_INPUT wake class
 * (tos-api/src/aes/acc.rs:164) -- a class only the idle pass drives, and only
 * when a key or a pointer movement arrives.  In a headless QEMU run no input
 * ever arrives, so the program never gets the CPU back.  Measured identically
 * for a KERNEL-launched program (serial/control-kernel-launch-no-input.log),
 * i.e. with no Pexec anywhere in the picture.
 */

#include <mint/osbind.h>   /* Cconws, Pterm */
#include <gem.h>           /* appl_init, evnt_timer, appl_exit */

int main(void)
{
	Cconws("[KID] 1 main entry\r\n");

	(void)appl_init();
	Cconws("[KID] 2 appl_init done\r\n");

	/* The interesting line of the whole measurement: an EL0 child of an EL0
	 * parent blocks in an AES wait. */
	Cconws("[KID] 3 entering evnt_timer(1500)\r\n");
	(void)evnt_timer(1500UL);
	Cconws("[KID] 4 evnt_timer returned\r\n");

#ifndef NO_APPL_EXIT
	appl_exit();
	Cconws("[KID] 5 appl_exit done, Pterm(0)\r\n");
#else
	Cconws("[KID] 5 (no appl_exit in this build), Pterm(0)\r\n");
#endif
	Pterm(0);
	return 0;
}
