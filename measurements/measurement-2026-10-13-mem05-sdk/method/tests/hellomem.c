/*
 * hellomem.c -- MEM05 part B: the memory footprint of a small libc program.
 * A hello-world (printf to the console stream, a few small mallocs), then:
 * the hunks mintlib took (hunks.h, sizes) and the GEMDOS blocks it holds
 * beyond its start (free_mds() delta, as MDSTEP). Built against the libc.a
 * before part B (HELLOOLD.NAP) and after (HELLOMEM.NAP): with MAXHUNK raised
 * only as a ceiling, both are expected to take the same single 8 KiB hunk.
 * Reports, does not judge: "HELLOMEM: DONE".
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "common.h"
#include "hunks.h"

#define MAXP 9000	/* above the default per-process limit 8192 */
static void *held[MAXP];

static long free_mds(void)
{
	long n = 0, i;
	for (n = 0; n < MAXP; n++) {
		held[n] = (void *)Mxalloc(1L, 3);
		if (held[n] == 0 || (long)held[n] < 0)
			break;
	}
	for (i = n - 1; i >= 0; i--)
		Mfree(held[i]);
	return n;
}

int main(void)
{
	long mds0 = free_mds(), mds1;
	unsigned long big0 = (unsigned long)Malloc(-1L);
	char *a, *b;

	/* what the start-up (_main: stdio set-up) already took before main */
	hunk_report("[HELLOMEM] " HELLO_TAG " mintlib at main", 1);
	Cconws("[HELLOMEM] " HELLO_TAG " more blocks allowed at main: ");
	hk_putdec((unsigned long)mds0);
	Cconws("\r\n");
	printf("Hello, world! (%s)\n", HELLO_TAG);
	fflush(stdout);
	a = malloc(100);
	b = malloc(1000);
	strcpy(a, "x");
	b[0] = a[0];
	mds1 = free_mds();
	Cconws("[HELLOMEM] " HELLO_TAG " more blocks allowed at the end: ");
	hk_putdec((unsigned long)mds1);
	Cconws(", taken in main=");
	hk_putdec((unsigned long)(mds0 - mds1));
	Cconws(" Malloc(-1) before=");
	puthex(big0);
	Cconws(" after=");
	puthex((unsigned long)Malloc(-1L));
	Cconws("\r\n");
	hunk_report("[HELLOMEM] " HELLO_TAG " mintlib", 1);
	Cconws("HELLOMEM: DONE\r\n");
	return 0;
}
