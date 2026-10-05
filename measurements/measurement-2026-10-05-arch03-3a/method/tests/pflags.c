/*
 * pflags.c -- ARCH03 stage 2a (E8b): the program reads back the PRGFLAGS its
 * ELF note carries, and checks the pool the loader chose by them.
 *
 * Built twice by build-s2-tests.sh: PFLAGS1.NCP with PRGFLAGS=0x10000004
 * (PF_TTRAMMEM only, quantum bits set -- no PF_TTRAMLOAD: TPA and environment
 * must come from ST-RAM, EmuTOS alloc_env/alloc_tpa, bdos/proc.c:425,460-502)
 * and PFLAGS2.NCP = a copy patched with `cargo xtask prgflags ... 0x7`
 * (PF_STANDARD: TT-RAM preferred). The expected value comes in the command
 * tail as 8 hex digits (NEWDESK #Z cannot pass one, so each binary also
 * carries it in WANT_FLAGS; the tail wins when present).
 */
#include "common.h"

#ifndef WANT_FLAGS
#define WANT_FLAGS 0x7UL
#endif

int main(void)
{
	unsigned long want = WANT_FLAGS;
	unsigned long st, tt, top, me = (unsigned long)_base;
	int i;

	if (_base->p_cmdlin[0] == 8) {
		want = 0;
		for (i = 1; i <= 8; i++) {
			char c = _base->p_cmdlin[i];
			want = (want << 4) | (unsigned long)(c <= '9' ? c - '0' : (c | 0x20) - 'a' + 10);
		}
	}
	Cconws("[PFLAGS] p_flags=");
	puthex(_base->p_reserved /* p_flags @0x28 */);
	Cconws(" p_lowtpa=");
	puthex(_base->p_lowtpa);
	Cconws(" p_env=");
	puthex(_base->p_env);
	Cconws("\r\n");
	checkhex("p_flags == the note's value", _base->p_reserved /* p_flags @0x28 */, want);

	/* phystop ($42E) is the top of ST-RAM; TT-RAM lies above it on both
	   boards. A probe block in each pool confirms the line. */
	st = (unsigned long)Mxalloc(16, 0);
	tt = (unsigned long)Mxalloc(16, 1);
	top = *(volatile unsigned int *)0x42EUL;
	Cconws("[PFLAGS] phystop=");
	puthex(top);
	Cconws(" ST block=");
	puthex(st);
	Cconws(" TT block=");
	puthex(tt);
	Cconws("\r\n");
	check("ST block < phystop <= TT block", st != 0 && tt != 0 && st < top && tt >= top);
	if (want & 2) {
		check("PF_TTRAMLOAD: TPA in TT-RAM", me >= top);
		check("PF_TTRAMLOAD: environment in TT-RAM", _base->p_env >= top);
	} else {
		check("no PF_TTRAMLOAD: TPA in ST-RAM", me < top);
		check("no PF_TTRAMLOAD: environment in ST-RAM", _base->p_env < top);
	}
	/* Malloc follows PF_TTRAMMEM (EmuTOS xmalloc, bdos/umem.c:116). */
	{
		unsigned long m = (unsigned long)Malloc(16);
		Cconws("[PFLAGS] Malloc block=");
		puthex(m);
		Cconws("\r\n");
		if (want & 4)
			check("PF_TTRAMMEM: Malloc from TT-RAM", m >= top);
		else
			check("no PF_TTRAMMEM: Malloc from ST-RAM", m < top);
	}
	Cconws(fails ? "PFLAGS: FAIL\r\n" : "PFLAGS: PASS\r\n");
	return fails;
}
