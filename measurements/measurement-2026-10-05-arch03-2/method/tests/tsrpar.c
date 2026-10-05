/*
 * tsrpar.c -- ARCH03 stage 2c: Ptermres (EmuTOS xtermres, bdos/proc.c:624-634:
 * shrink the TPA to blkln, keep the owned blocks, xterm). The TSR's code,
 * data and BSS stay where they are and are called after it ended -- from
 * here and from a program started later (TSRUSE); its Malloc block keeps
 * its contents; the TPA beyond blkln is back in the pool.
 */
#include "common.h"

static volatile unsigned int mb[4];

int main(void)
{
	char tail[16];
	long rc, l0, l1, (*fn)(long);

	l0 = (long)Mxalloc(-1L, 1);
	tail_hex(tail, (unsigned long)mb);
	rc = Pexec(0, "C:\\TSRRES.NAP", tail, 0L);
	checkhex("Pexec(TSRRES) = its Ptermres code", (unsigned long)rc, 0x31);
	Cconws("[TSRPAR] tsr_fn=");
	puthex(mb[0]);
	Cconws(" PD=");
	puthex(mb[1]);
	Cconws(" blkln=");
	puthex(mb[2]);
	Cconws("\r\n");
	if (rc != 0x31 || !mb[0]) {
		Cconws("TSR: FAIL\r\n");
		return 1;
	}
	fn = (long (*)(long))(unsigned long)mb[0];
	checkhex("resident code, 1st call (data+bss+rodata alive)", (unsigned long)fn(5), 1022);
	checkhex("resident code, 2nd call (its BSS counter kept)", (unsigned long)fn(5), 2022);
	checkhex("its Malloc block kept its contents", *(volatile unsigned int *)(unsigned long)mb[3],
		 0x75AA5A75);
	l1 = (long)Mxalloc(-1L, 1);
	Cconws("[TSRPAR] TT largest block before ");
	puthex(l0);
	Cconws(" after ");
	puthex(l1);
	Cconws("\r\n");
	check("TPA beyond blkln back in the pool (< 64 KiB kept)", l0 - l1 < 0x10000);
	/* E15 (stage 2d): resident memory is nobody's to free. */
	checkhex("Mfree(the TSR's PD) = EIMBA", (unsigned long)Mfree((void *)(unsigned long)mb[1]),
		 (unsigned long)-40L);
	checkhex("Mfree(the TSR's Malloc block) = EIMBA", (unsigned long)Mfree((void *)(unsigned long)mb[3]),
		 (unsigned long)-40L);
	tail_hex(tail, (unsigned long)fn);
	rc = Pexec(0, "C:\\TSRUSE.NAP", tail, 0L);
	checkhex("a later program calls the resident code (exit 0x55)", (unsigned long)rc, 0x55);
	Cconws(fails ? "TSR: FAIL\r\n" : "TSR: PASS\r\n");
	return fails;
}
