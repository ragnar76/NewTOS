/*
 * tsrres.c -- ARCH03 stage 2c: a program that stays resident (Ptermres).
 * Started by TSRPAR with the address of TSRPAR's mailbox in its command tail
 * (8 hex digits). It publishes the address of tsr_fn, its PD, its blkln and
 * a Malloc'd block there (post-E9: and the TPA length its start-up kept,
 * mintlib's _PgmSize, crtinit.c:222), then Ptermres(blkln, 0x31) with blkln = basepage
 * through the end of its BSS (p_bbase + p_blen - p_lowtpa: in NewTOS the
 * image starts at p_lowtpa + 0x1000, not + 0x100).
 */
#include "common.h"

static long bias = 7;			/* .data */
static const long step = 1000;		/* .rodata */
static long counter;			/* .bss */
extern unsigned long _PgmSize;		/* mintlib globals.c: TPA kept at start */

long tsr_fn(long x)
{
	counter++;
	return x * 3 + bias + counter * step;
}

int main(void)
{
	volatile unsigned int *mb = (volatile unsigned int *)tail_value(_base->p_cmdlin);
	unsigned long blkln = (_base->p_bbase + _base->p_blen - _base->p_lowtpa + 15) & ~15UL;
	unsigned int *blk = (unsigned int *)Mxalloc(64, 1);

	if (!mb) {
		Cconws("TSRRES: no mailbox\r\n");
		return 1;
	}
	blk[0] = 0x75AA5A75;
	mb[0] = (unsigned int)(unsigned long)tsr_fn;
	mb[1] = (unsigned int)(unsigned long)_base;
	mb[2] = (unsigned int)blkln;
	mb[3] = (unsigned int)(unsigned long)blk;
	mb[4] = (unsigned int)_PgmSize;
	Cconws("[TSRRES] Ptermres(blkln=");
	puthex(blkln);
	Cconws(", 0x31), tsr_fn at ");
	puthex((unsigned long)tsr_fn);
	Cconws("\r\n");
	Ptermres(blkln, 0x31);
	return 2;	/* not reached */
}
