/*
 * fseekcon.c -- acceptance probe for the open item "Fseek on a console handle
 * answers EIHNDL, EmuTOS 0" (TASK/ARCH03-POSTE9, point 2). EmuTOS
 * bdos/bdosmain.c:571-641: on a character device only Fread/Fwrite, other
 * calls return 0. Calls Fseek(0, h, SEEK_CUR) on the standard handles 0..2
 * and prints the result; PASS only if all are 0.
 */
#include "common.h"

int main(void)
{
	short h;
	for (h = 0; h < 3; h++) {
		long r = Fseek(0L, h, 1);
		Cconws("  Fseek(0, h=");
		puthex((unsigned long)h);
		Cconws(", SEEK_CUR) = ");
		puthex((unsigned long)r);
		Cconws("\r\n");
		if (r != 0)
			fails++;
	}
	Cconws(fails ? "FSEEKCON: FAIL\r\n" : "FSEEKCON: PASS\r\n");
	return fails;
}
