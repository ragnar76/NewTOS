/*
 * tsruse.c -- ARCH03 stage 2c: a program started AFTER the TSR ended calls
 * the resident code (address in the command tail). 3rd call of tsr_fn:
 * 5*3 + 7 + 3*1000 = 3022. Exit 0x55 if right.
 */
#include "common.h"

int main(void)
{
	long (*fn)(long) = (long (*)(long))tail_value(_base->p_cmdlin);
	long r;
	if (!fn)
		return 1;
	r = fn(5);
	Cconws("[TSRUSE] tsr_fn(5) = ");
	puthex((unsigned long)r);
	Cconws("\r\n");
	return r == 3022 ? 0x55 : 2;
}
