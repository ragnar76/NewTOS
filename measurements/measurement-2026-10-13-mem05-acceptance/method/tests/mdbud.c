/*
 * mdbud.c -- MEM05 QEMU test "budget exhaustion" (design §7 test 16): run
 * with mdlimit=0 (no per-process limit), so only the MEM05 page budget
 * (512 pages for MDs + block records + snapshot + F_KEEP flags) ends
 * Mxalloc(1, 3). Reports the count, checks Malloc(-1) > 0 and that freeing
 * one block makes exactly that room again, frees everything (newest first)
 * and checks that the same count can be taken a second time (the pages
 * stay; nothing leaked). The desktop afterwards must draw normally (heap not
 * starved; screendump). Verdict line "MDBUD: PASS"/"MDBUD: FAIL".
 */
#include "common.h"

#define MAXB 100000
static void *blk[MAXB];

static void putdec(unsigned long v)
{
	char b[24];
	int i = 23;
	b[i] = 0;
	do { b[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	Cconws(&b[i]);
}

static long fill(void)
{
	long n = 0;
	while (n < MAXB) {
		long a = Mxalloc(1L, 3);
		if (a == 0 || a < 0)
			break;
		blk[n++] = (void *)a;
	}
	return n;
}

int main(void)
{
	long n, n2, i, a;
	n = fill();
	Cconws("[MDBUD] Mxalloc(1,3) successes until 0: ");
	putdec((unsigned long)n);
	Cconws(" Malloc(-1)=");
	puthex((unsigned long)Malloc(-1L));
	Cconws("\r\n");
	check("stopped before the probe's own cap", n < MAXB);
	check("thousands of blocks", n > 10000);
	check("Malloc(-1) > 0 at budget exhaustion", Malloc(-1L) > 0);
	Mfree(blk[n - 1]);
	a = Mxalloc(1L, 3);
	check("one freed -> one more", a > 0);
	blk[n - 1] = (void *)a;
	checkhex("and not two", (unsigned long)Mxalloc(1L, 3), 0);
	for (i = n - 1; i >= 0; i--)
		Mfree(blk[i]);
	n2 = fill();
	Cconws("[MDBUD] second round: ");
	putdec((unsigned long)n2);
	Cconws("\r\n");
	checkhex("second round takes the same number", (unsigned long)n2, (unsigned long)n);
	for (i = n2 - 1; i >= 0; i--)
		Mfree(blk[i]);
	Cconws(fails ? "MDBUD: FAIL\r\n" : "MDBUD: PASS\r\n");
	return fails;
}
