/*
 * mdprobe.c -- post-E9: how many GEMDOS memory descriptors does mintlib's
 * malloc (GEMDOS Malloc hunks of 8..32 KiB, mintlib/malloc.c:26-27,64-95,
 * unix/sbrk.c:54-73, ARCH03 E9) take for many small blocks in ONE program,
 * and does it run out of descriptors before it runs out of memory?
 *
 * free_mds(): the number of further GEMDOS blocks the system can hand out
 * right now = Mxalloc(1, 3) until it fails (each success takes one MD; the
 * pool has ~1 GiB, so only the descriptors can run out), then Mfree them
 * all. Measured before, during and after malloc'ing 32-byte blocks (up to
 * 10^6 = 32 MiB payload) in steps. Reports, does not judge: "MDPROBE: DONE".
 */
#include <stdlib.h>
#include "common.h"

#define MAXP 512
static void *held[MAXP];

static long free_mds(void)
{
	long n = 0, i;
	for (n = 0; n < MAXP; n++) {
		held[n] = (void *)Mxalloc(1L, 3);
		if (held[n] == 0 || (long)held[n] < 0)
			break;
	}
	for (i = 0; i < n; i++)
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

static void report(const char *what, unsigned long blocks, unsigned long size, long mds0)
{
	long mds = free_mds();
	Cconws("[MDPROBE] ");
	Cconws(what);
	Cconws(" blocks=");
	putdec(blocks);
	Cconws(" payload=");
	putdec(blocks * size);
	Cconws(" free MDs=");
	putdec((unsigned long)mds);
	Cconws(" used by malloc=");
	putdec((unsigned long)(mds0 - mds));
	Cconws(" Malloc(-1)=");
	puthex((unsigned long)Malloc(-1L));
	Cconws("\r\n");
}

static unsigned long run(unsigned long size, unsigned long limit, long mds0)
{
	static const unsigned long steps[] = { 100, 1000, 10000, 50000, 100000, 200000, 500000, 1000000, 0 };
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
			report("step", n, size, mds0);
			s++;
		}
		if (n >= limit)
			break;
	}
	report("end", n, size, mds0);
	return n;
}

int main(void)
{
	long mds0 = free_mds();
	Cconws("[MDPROBE] free MDs at start: ");
	putdec((unsigned long)mds0);
	Cconws("\r\n");
	/* 32-byte blocks up to 32 MiB payload */
	run(32, 1000000, mds0);
	Cconws("MDPROBE: DONE\r\n");
	return 0;
}
