/*
 * mdprobe.c -- MEM05 adaptation of the post-E9 probe
 * (measurements/measurement-2026-10-06-post-e9/method/tests/mdprobe.c,
 * copied here as mdprobe-orig.c).
 *
 * Unchanged: mintlib's malloc(32) in one program (GEMDOS Malloc hunks of
 * 8..32 KiB, the CURRENT libc -- MAXHUNK is still 32 KiB, MEM05 part B is
 * not in), steps reported with Malloc(-1).
 * Changed for MEM05 (design §6 "MDPROBE"):
 *  - MAXP 512 -> 20000: free_mds() counts how many more GEMDOS blocks THIS
 *    process may take (Mxalloc(1, 3) until it fails). With the per-process
 *    limit (default 8192, mdlimit=) that is "limit - blocks held", no longer
 *    a system-wide MD count; MAXP is above the default limit so the limit,
 *    not the probe, ends the count.
 *  - free_mds() only at the start and at the end (each call is thousands of
 *    Mxalloc/Mfree; with memory protection each costs a sync over all live
 *    blocks -- the documented known limit, MEM10).
 *  - the series goes far beyond the old 147 605 wall: up to 3 000 000 blocks
 *    (96 MB payload, ~5 100 hunks), below the 8192 limit.
 * Reports, does not judge: "MDPROBE: DONE". Timing via the serial log.
 */
#include <stdlib.h>
#include "common.h"

#define MAXP 20000
static void *held[MAXP];

static long free_mds(void)
{
	long n = 0, i;
	for (n = 0; n < MAXP; n++) {
		held[n] = (void *)Mxalloc(1L, 3);
		if (held[n] == 0 || (long)held[n] < 0)
			break;
	}
	/* newest first: each Mfree finds its block at the head of the list */
	for (i = n - 1; i >= 0; i--)
		Mfree(held[i]);
	return n;
}

static void putdec(unsigned long v)
{
	char b[24];
	int i = 23;
	b[i] = 0;
	do { b[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	Cconws(&b[i]);
}

static void report(const char *what, unsigned long blocks, unsigned long size, long mds0, int probe)
{
	Cconws("[MDPROBE] ");
	Cconws(what);
	Cconws(" blocks=");
	putdec(blocks);
	Cconws(" payload=");
	putdec(blocks * size);
	if (probe) {
		long mds = free_mds();
		Cconws(" more blocks allowed=");
		putdec((unsigned long)mds);
		Cconws(" taken by malloc=");
		putdec((unsigned long)(mds0 - mds));
	}
	Cconws(" Malloc(-1)=");
	puthex((unsigned long)Malloc(-1L));
	Cconws("\r\n");
}

static unsigned long run(unsigned long size, unsigned long limit, long mds0)
{
	static const unsigned long steps[] = { 100, 1000, 10000, 100000, 147605, 147606, 200000,
		500000, 1000000, 2000000, 3000000, 0 };
	unsigned long n = 0;
	int s = 0;
	for (;;) {
		void *p = malloc(size);
		if (!p) {
			Cconws("[MDPROBE] malloc(");
			putdec(size);
			Cconws(") returned NULL after ");
			putdec(n);
			Cconws(" blocks\r\n");
			break;
		}
		*(volatile char *)p = 1;
		n++;
		if (steps[s] && n == steps[s]) {
			report("step", n, size, mds0, 0);
			s++;
		}
		if (n >= limit)
			break;
	}
	report("end", n, size, mds0, 1);
	return n;
}

int main(void)
{
	long mds0 = free_mds();
	Cconws("[MDPROBE] more blocks allowed at start: ");
	putdec((unsigned long)mds0);
	Cconws("\r\n");
	run(32, 3000000, mds0);
	Cconws("MDPROBE: DONE\r\n");
	return 0;
}
