/*
 * mdacc.c -- MEM05 part B, design QEMU test 15: a libc ACCESSORY (C:\ACC\
 * MDACC.NAC) that mallocs a lot in the acc_main context. heap_init.c sets
 * MAXHUNK = 16 MiB at the very start of __libc_crtinit, before the
 * accessory branch (p_parent == 0 -> acc_main), so an accessory's malloc
 * must double its hunks up to 16 MiB as a program's does. malloc(32) x
 * 147 605 (the old wall), then the hunk sequence (hunks.h) and the GEMDOS
 * blocks taken (free_mds() delta; run with mdlimit=128 so the probe stays
 * short). Model (design §4.4): 10 hunks, 8 KiB .. 4 MiB. All blocks are
 * freed again, then the accessory registers and waits like PPACC.
 * Prints "MDACC: PASS" when the hunk count is the model's 10 and every hunk
 * size is 8 KiB << i (by address order, so checked as a set), else FAIL.
 */
#include <stdlib.h>
#include <gem.h>
#include "common.h"
#include "hunks.h"

extern short _app;

#define MAXP 4096
static void *held[MAXP];
#define N 147605UL
static void *blk[N];

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
	long mds0, mds1;
	unsigned long i, n, seen = 0;
	const struct mh_chunk *c;
	short id, msg[8];

	check("an accessory (_app == 0)", _app == 0);
	checkhex("p_parent == 0", ((const BASEPAGE *)_base)->p_parent, 0);
	mds0 = free_mds();
	for (i = 0; i < N; i++) {
		blk[i] = malloc(32);
		if (!blk[i])
			break;
		*(volatile char *)blk[i] = 1;
	}
	checkhex("malloc(32) x 147605 without NULL", i, N);
	mds1 = free_mds();
	Cconws("[MDACC] GEMDOS blocks taken by malloc=");
	hk_putdec((unsigned long)(mds0 - mds1));
	Cconws("\r\n");
	n = hunk_report("[MDACC] mintlib", 1);
	checkhex("hunks (model 10)", n, 10);
	checkhex("GEMDOS blocks = hunks", (unsigned long)(mds0 - mds1), n);
	for (c = _mchunk_free_list.next; c; c = c->next)
		if (c->valid == MH_VAL_BORDER) {
			unsigned long sz = *(const long *)(c + 1) + MH_BORDER_EXTRA, k;
			for (k = 0; k < 10; k++)
				if (sz == (8192UL << k))
					seen |= 1UL << k;
		}
	checkhex("hunk sizes = 8 KiB << 0..9 (bit set)", seen, 0x3ff);
	while (i > 0)
		free(blk[--i]);
	hunk_report("[MDACC] mintlib after free", 0);
	Cconws(fails ? "MDACC: FAIL\r\n" : "MDACC: PASS\r\n");
	id = appl_init();
	menu_register(id, "  MDACC probe");
	for (;;)
		evnt_mesag(msg);
	return 0;
}
