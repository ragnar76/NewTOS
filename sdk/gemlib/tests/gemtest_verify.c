/*
 * gemtest_verify.c -- the observable sibling of gemtest.c (ADR-020 Stage 3).
 *
 * gemtest.c is a silent LINK test. This variant makes the SAME GEM sequence
 * OBSERVABLE on the console so a QEMU boot can confirm the ILP32 AES/VDI
 * parameter-block reads landed on the right arrays: it prints appl_init()'s
 * application id and v_opnvwk()'s workstation handle. If the kernel read the
 * ILP32 4-byte block pointers as 8-byte (the pre-Stage-3 bug), the `global`/
 * `intout` pointers would be garbage and appl_init would fault or return a wild
 * id; a valid id + handle here is the positive proof.
 *
 * Built for ILP32 by build-gemlib-ilp32.sh (pass this file as the client) and
 * auto-run from C:\AUTO\ so its output appears on the serial console before the
 * desktop starts. printf() comes from the ported ILP32 mintlib.
 *
 * HISTORICAL, 2026-09-04: the ILP32 build path was retired (ADR-020 addendum in
 * docs/MEMORY.md), so build-gemlib-ilp32.sh no longer exists and this file has
 * no builder. Kept as the Stage-3 evidence it is; it is NOT an LP64 test -- its
 * output strings assert an ILP32 result. Retarget or delete it together with
 * the runtime ABI switch (Abi::Ilp32), not before. It is a TARGET program (it
 * traps into the AES), not a host test, so sdk/run-host-tests.sh does not
 * run it either -- by design, not by omission (TASK/T06).
 */
#include "gem.h"
#include <stdio.h>

int main(void)
{
	short work_in[11]  = { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2 };
	short work_out[57];
	short handle;
	short wcell, hcell, wbox, hbox;
	short apid;

	/* AES: register the application (selector 200). appl_init() returns
	   aes_intout[0], which the kernel set to the pid -- reading it requires
	   the intout block pointer to have been read at the ILP32 width. */
	apid = appl_init();
	printf("GEMTEST-ILP32: appl_init -> apid=%d\r\n", apid);

	/* AES: physical workstation handle (reads intin/intout). */
	handle = graf_handle(&wcell, &hcell, &wbox, &hbox);
	printf("GEMTEST-ILP32: graf_handle -> handle=%d cell=%dx%d box=%dx%d\r\n",
	       (int)handle, (int)wcell, (int)hcell, (int)wbox, (int)hbox);

	/* VDI: open a virtual workstation (selector 115). The returned handle
	   comes back through the contrl block pointer; xres/yres via intout. */
	v_opnvwk(work_in, &handle, work_out);
	printf("GEMTEST-ILP32: v_opnvwk -> handle=%d xres=%d yres=%d\r\n",
	       (int)handle, (int)work_out[0], (int)work_out[1]);

	/* VDI: close it again. */
	v_clsvwk(handle);
	printf("GEMTEST-ILP32: v_clsvwk done\r\n");

	/* AES: deregister. */
	appl_exit();
	printf("GEMTEST-ILP32: appl_exit done -- ABI=ILP32 param-block reads OK\r\n");

	return 0;
}
