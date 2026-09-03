/*
 * chaintest.c -- prove the AES shell loop executes a `shel_write` launch, and
 * that the command tail survives the trip.
 *
 * NewTOS stage S3 (docs/analysis/desktop-userspace-plan.md), acceptance item 2:
 * "a .NAP that does shel_write(SHW_EXEC, ...) and then Pterm starts the next
 * program -- instead of being silently ignored".  Before the shell loop existed
 * the request was recorded and never consumed (audit finding P6-005).
 *
 * The program is deliberately tiny and non-interactive so it can run headless
 * in QEMU.  It runs in two phases, distinguished by its own command tail, which
 * it reads back with shel_read():
 *
 *   phase 1 (started with no tail, e.g. from the .INF `#Z` line)
 *       -> shel_write(SHW_EXEC, isgem=0, "C:\CHAINTST.NAP", "\x07-x test")
 *          i.e. chain to itself WITH a seven-character tail, then Pterm.
 *   phase 2 (started with that tail)
 *       -> report the tail it received and chain to C:\HELLO.NCP, then Pterm.
 *
 * So one boot exercises: shel_write -> sh_ldapp (NORMAL_APP arm) -> sh_find ->
 * the launch, the length-prefixed command tail (`tail[0]` = character count,
 * desk/deskpro.c:56-57), and finally the shell loop's return to the built-in
 * desktop when the last program ends.
 */

#include <mint/osbind.h>   /* Cconws, Pterm */
#include <gem.h>           /* appl_init, appl_exit, shel_read, shel_write */

#define SHW_EXEC 1

int main(void)
{
	char cmd[128];
	char tail[128];
	char self[128];
	short i;

	(void)appl_init();

	for (i = 0; i < 128; i++) { cmd[i] = 0; tail[i] = 0; }
	(void)shel_read(cmd, tail);

	Cconws("CHAINTEST: shel_read cmd=[");
	Cconws(cmd);
	Cconws("] tail=[");
	Cconws(tail);
	Cconws("]\r\n");

	if (tail[0] == '\0') {
		/* Phase 1: chain to ourselves with a seven-character tail. */
		self[0] = 7;                 /* Atari command tail: length byte first */
		self[1] = '-'; self[2] = 'x'; self[3] = ' ';
		self[4] = 't'; self[5] = 'e'; self[6] = 's'; self[7] = 't';
		self[8] = '\0';
		Cconws("CHAINTEST: shel_write(SHW_EXEC, C:\\CHAINTST.NAP, \"-x test\")\r\n");
		(void)shel_write(SHW_EXEC, 0, 1, "C:\\CHAINTST.NAP", self);
	} else {
		/* Phase 2: the tail arrived; hand over to a plain console program. */
		self[0] = 0;
		self[1] = '\0';
		Cconws("CHAINTEST: shel_write(SHW_EXEC, C:\\HELLO.NCP)\r\n");
		(void)shel_write(SHW_EXEC, 0, 1, "C:\\HELLO.NCP", self);
	}

	appl_exit();
	Pterm(0);
	return 0;
}
