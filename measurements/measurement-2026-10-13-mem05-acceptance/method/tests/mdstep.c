/*
 * mdstep.c -- MEM05 part B (design docs/analysis/mem05-design-2026-10-13.md
 * §4.4 and QEMU test 10): mintlib's malloc(32) far beyond the old 147 605
 * wall, with the SDK's MAXHUNK = 16 MiB (heap_init.c ->
 * __newtos_malloc_maxhunk). At every step:
 *   - the OS view: blocks this process holds beyond its start = more blocks
 *     allowed at the start minus more blocks allowed now (free_mds(): Mxalloc
 *     (1, 3) until it fails, then Mfree) -- i.e. the MDs malloc took. Run with
 *     a small per-process limit (mdlimit=128 boot argument) so each probe is
 *     at most ~128 Mxalloc/Mfree (with memory protection every one is a sync
 *     over all live blocks, MEM10); 128 > the 44 hunks the model expects at
 *     10 000 000;
 *   - the program view: mintlib's own hunk list (hunks.h).
 * Model (design §4.4, malloc(32) = 56 bytes per block): 100 -> 1,
 * 1 000 -> 3, 10 000 -> 7, 147 605 -> 10, 1 000 000 -> 14, 10 000 000 -> 44.
 * Reports, does not judge: "MDSTEP: DONE". Timing via the serial stamps.
 * MDSTEP_NOPROBE: no free_mds() at the steps (timing only).
 */
#include <stdlib.h>
#include "common.h"
#include "hunks.h"

#define MAXP 4096
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

static void step(unsigned long n, long mds0, int verbose)
{
	Cconws("[MDSTEP] blocks=");
	hk_putdec(n);
	Cconws(" payload=");
	hk_putdec(n * 32);
#ifndef MDSTEP_NOPROBE
	{
		long mds = free_mds();
		Cconws(" MDs taken by malloc=");
		hk_putdec((unsigned long)(mds0 - mds));
	}
#endif
	Cconws(" Malloc(-1)=");
	puthex((unsigned long)Malloc(-1L));
	Cconws("\r\n");
	hunk_report("[MDSTEP]   mintlib", verbose);
}

int main(void)
{
	static const unsigned long steps[] = { 100, 1000, 10000, 100000, 147605, 147606,
		1000000, 2000000, 3000000, 5000000, 10000000, 0 };
	unsigned long n = 0;
	int s = 0;
	long mds0 = free_mds();

	Cconws("[MDSTEP] more blocks allowed at start: ");
	hk_putdec((unsigned long)mds0);
	Cconws("\r\n");
	hunk_report("[MDSTEP]   mintlib at start", 1);
	while (steps[s]) {
		void *p = malloc(32);
		if (!p) {
			Cconws("[MDSTEP] malloc(32) returned NULL after ");
			hk_putdec(n);
			Cconws(" blocks\r\n");
			step(n, mds0, 1);
			break;
		}
		*(volatile char *)p = 1;
		n++;
		if (n == steps[s]) {
			step(n, mds0, steps[s + 1] == 0 || n == 147605);
			s++;
		}
	}
	Cconws("MDSTEP: DONE\r\n");
	return 0;
}
