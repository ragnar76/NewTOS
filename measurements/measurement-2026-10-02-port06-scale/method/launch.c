/*
 * launch.c -- measurement helper for the PORT06 scaling step, NOT part of
 * the port. Started by #Z autorun as C:\LAUNCH.NAP; reads one command line
 * from C:\LAUNCH.TXT and runs it with system() (mintlib's TOS branch:
 * system_hack -> spawnvp -> Pexec(0) with a command tail, TASK/PORT10). That
 * is how a typed start parameter reaches DOOM: through the basepage command
 * tail and crt0's argv, as from the desktop's parameter box -- which a
 * headless QEMU (no keyboard on raspi4b) cannot fill in. The current
 * directory is set to C:\DOOM first, as the desktop does for a program it
 * starts (DOOM looks for its WAD in ".").
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <mint/osbind.h>

int main(void)
{
	char line[128];
	FILE *f = fopen("C:\\LAUNCH.TXT", "r");
	int rc;

	if (!f || !fgets(line, sizeof(line), f)) {
		printf("LAUNCH: cannot read C:\\LAUNCH.TXT\n");
		return 1;
	}
	fclose(f);
	line[strcspn(line, "\r\n")] = 0;
	Dsetdrv(2);
	Dsetpath("\\DOOM");
	printf("LAUNCH: system(\"%s\")\n", line);
	rc = system(line);
	printf("LAUNCH: system() returned %d\n", rc);
	return 0;
}
