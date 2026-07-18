/*
 * gemtest.c -- minimal GEM client linked against crt0 + libc.a + gemlib.a.
 *
 * Purpose (build/link verification for the NewTOS gemlib port): exercise BOTH
 * retargeted GEM-trap primitives so the linked ELF contains the AES (`svc #2`,
 * selector 200) and VDI (`svc #2`, selector 115) trap sites:
 *
 *   appl_init()            -> aes()  (AES, selector 200)
 *   graf_handle()          -> aes()  (physical workstation handle)
 *   v_opnvwk()/v_clsvwk()  -> vdi()  (VDI,  selector 115)
 *   appl_exit()            -> aes()
 *
 * This is a LINK test: the values passed to v_opnvwk are conventional defaults;
 * the lead runs the real thing on the desktop. main() returns to the libc crt0,
 * which terminates the process via the Pterm trap.
 */
#include "gem.h"

int main(void)
{
	short work_in[11]  = { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2 };
	short work_out[57];
	short handle;
	short wcell, hcell, wbox, hbox;

	/* AES: register the application (selector 200). */
	appl_init();

	/* AES: obtain the physical workstation handle. */
	handle = graf_handle(&wcell, &hcell, &wbox, &hbox);

	/* VDI: open then close a virtual workstation (selector 115). */
	v_opnvwk(work_in, &handle, work_out);
	v_clsvwk(handle);

	/* AES: deregister. */
	appl_exit();

	return 0;
}
