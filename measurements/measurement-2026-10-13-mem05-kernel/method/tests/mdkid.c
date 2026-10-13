/*
 * mdkid.c -- MEM05: the child of mdpar.c. Takes Malloc(16) until 0 under
 * its own owner cookie, checks Malloc(-1) > 0 at its limit, frees
 * everything (newest first) and exits with the number of successes.
 */
#include "common.h"

#define MAXB 70000
static void *blk[MAXB];

int main(void)
{
	long n = 0, i;
	while (n < MAXB) {
		long a = Malloc(16L);
		if (a == 0 || a < 0)
			break;
		blk[n++] = (void *)a;
	}
	Cconws("[MDKID] successes until 0: ");
	puthex((unsigned long)n);
	Cconws(Malloc(-1L) > 0 ? " Malloc(-1) > 0\r\n" : " Malloc(-1) <= 0 (FAIL)\r\n");
	for (i = n - 1; i >= 0; i--)
		Mfree(blk[i]);
	return (int)n;
}
