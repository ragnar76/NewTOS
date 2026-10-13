/*
 * tsrpar.c -- ARCH03 stage 2c: Ptermres (EmuTOS xtermres, bdos/proc.c:624-634:
 * shrink the TPA to blkln, keep the owned blocks, xterm). The TSR's code,
 * data and BSS stay where they are and are called after it ended -- from
 * here and from a program started later (TSRUSE); its Malloc block keeps
 * its contents; the TPA beyond blkln is back in the pool.
 *
 * Post-E9 (lead decision (a), TASK/ARCH03-3c-STATUS-UNFINISHED.MD "Offen 1"):
 * "back in the pool" is checked SEMANTICALLY -- a free block starts at
 * PD + blkln (rounded up to the pool grain: 16 bytes without protection,
 * one page with it, E16) and reaches at least to the end of the TPA part
 * TSRRES kept at its start (PD + _PgmSize). Before, the check was a layout
 * proxy ("the largest TT block shrank by < 64 KiB"), which failed when the
 * freed part became a free block of its own between the resident part and
 * TSRRES' Malloc block (first fit; the same on EmuTOS + mintlib).
 * Method: Mxalloc(S, 3) with S = PD + _PgmSize - pageup(PD + blkln), the
 * smallest size the freed part has in either grain, repeated until a block
 * lands at or above PD + blkln (first fit hands out the free blocks in
 * address order, so blocks below are taken first); then all are freed.
 *
 * Built twice: TSRPAR.NAP (protection on/off) and TSRPARS.NAP with
 * -DTSRPAR_STRICT for memprot=strict, whose expected result is the denial.
 */
#include "common.h"

static volatile unsigned int mb[8];

int main(void)
{
	char tail[16];
	long rc, l0, l1, (*fn)(long);

	l0 = (long)Mxalloc(-1L, 1);
	tail_hex(tail, (unsigned long)mb);
	rc = Pexec(0, "C:\\TSRRES.NAP", tail, 0L);
#ifdef TSRPAR_STRICT
	/* memprot=strict (post-E9 expectation, not a failure): TSRRES is a
	 * Pexec-0 child with its own space; strict gives it no rights to our
	 * PROT_P blocks (FreeMiNT MPF_STRICT, sys/k_exec.c:799-806), so its
	 * write into our mailbox faults and the crash path ends it with
	 * Pterm(-1) before it can go resident. The resident-code checks need
	 * that mailbox and are not reachable in strict mode. */
	checkhex("Pexec(TSRRES) (strict): our mailbox denied -> Pterm(-1)", (unsigned long)rc, 0xffffUL);
	check("mailbox untouched", mb[0] == 0 && mb[1] == 0);
	Cconws(fails ? "TSR: FAIL\r\n" : "TSR: PASS (strict: child denied the parent's private mailbox)\r\n");
	return fails;
#endif
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
	/* informational since post-E9: the old layout proxy */
	Cconws(l0 - l1 < 0x10000 ? "[TSRPAR] (largest block shrank < 64 KiB)\r\n"
				 : "[TSRPAR] (largest block shrank >= 64 KiB -- layout only, not checked)\r\n");
	{
		unsigned long pd = mb[1], blkln = mb[2], kept = mb[4];
		unsigned long t16 = (pd + blkln + 15) & ~15UL, tpg = (pd + blkln + 0xfffUL) & ~0xfffUL;
		unsigned long s = pd + kept > tpg ? pd + kept - tpg : 0, a = 0;
		void *got[64];
		int n = 0, i, hit = 0;
		Cconws("[TSRPAR] kept at start ");
		puthex(kept);
		Cconws(", freed part starts at ");
		puthex(t16);
		Cconws("/");
		puthex(tpg);
		Cconws(", probe size ");
		puthex(s);
		Cconws("\r\n");
		while (s && n < 64) {
			a = (unsigned long)Mxalloc((long)s, 3);
			if (a == 0 || (long)a < 0)
				break;
			got[n++] = (void *)a;
			if (a >= pd + blkln)
				break;
		}
		hit = n > 0 && (a == t16 || a == tpg);
		Cconws("[TSRPAR] first block at or above PD+blkln: ");
		puthex(a);
		Cconws(" after ");
		puthex((unsigned long)n);
		Cconws(" Mxalloc\r\n");
		for (i = 0; i < n; i++)
			Mfree(got[i]);
		check("a free block starts at PD + blkln (TPA beyond blkln back in the pool)", hit);
	}
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
