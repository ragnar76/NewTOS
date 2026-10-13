/*
 * e9mem.c -- ARCH03 E9 probe (libc start-up): the TPA is the largest free
 * block (EmuTOS alloc_tpa, bdos/proc.c:460-502), crt0/__libc_crtinit shrinks
 * it to program + stack like mintlib's _crtinit (crtinit.c:154-247), malloc
 * then grows through GEMDOS Malloc (_split_mem, unix/sbrk.c:54-73), and the
 * program can still start a child, whose TPA is the largest block left.
 * Started without a tail it reports, checks and Pexec(0)s itself with the
 * tail "kid"; the child reports and checks the same.
 */
#include <stdlib.h>
#include "common.h"

extern unsigned long _PgmSize;
extern long _stksize;
extern short _split_mem;
extern void *_heapbase;

static void kv(const char *k, unsigned long v)
{
	Cconws(" ");
	Cconws(k);
	Cconws("=");
	puthex(v);
}

int main(void)
{
	unsigned long pd = (unsigned long)_base, tpa = _base->p_hitpa - _base->p_lowtpa;
	int kid = _base->p_cmdlin[0] == 3;
	const char *me = kid ? "[E9MEM kid]" : "[E9MEM]";
	unsigned long sp = (unsigned long)&pd;
	long st, tt, any;
	char *big;

	st = (long)Mxalloc(-1L, 0);
	tt = (long)Mxalloc(-1L, 1);
	any = (long)Malloc(-1L);
	Cconws(me);
	kv("PD", pd);
	kv("TPA(p_hitpa-p_lowtpa)", tpa);
	kv("kept(_PgmSize)", _PgmSize);
	kv("_stksize", (unsigned long)_stksize);
	kv("Mxalloc(-1,ST)", (unsigned long)st);
	kv("Mxalloc(-1,TT)", (unsigned long)tt);
	kv("Malloc(-1)", (unsigned long)any);
	Cconws("\r\n");

	check("p_lowtpa == PD", _base->p_lowtpa == pd);
	check("the TPA was larger than the old fixed 4 MiB", tpa > 0x400000UL);
	check("crtinit kept image + MINKEEP stack (< 1 MiB)", _PgmSize > 0x10000UL && _PgmSize < 0x100000UL);
	checkhex("Mshrink(PD, kept) == E_OK (the block IS that long)", (unsigned long)Mshrink(_base, _PgmSize), 0);
	checkhex("Mshrink(PD, kept + 4 KiB) == EGSBF (cannot grow)", (unsigned long)Mshrink(_base, _PgmSize + 0x1000),
		 (unsigned long)-67L);
	check("the stack is in the kept block", sp > pd && sp < pd + _PgmSize);
	check("mintlib default model: _split_mem, no own heap", _split_mem == 1 && _heapbase == 0);
	check("the rest went back: Malloc(-1) > TPA - kept - 1 MiB", (unsigned long)any + 0x100000UL > tpa - _PgmSize);
	big = malloc(6UL * 1024 * 1024);
	Cconws(me);
	kv("malloc(6 MiB)", (unsigned long)big);
	Cconws("\r\n");
	check("malloc(6 MiB) succeeds", big != 0);
	check("...from GEMDOS Malloc, outside the kept block",
	      (unsigned long)big >= pd + _PgmSize || (unsigned long)big + 6UL * 1024 * 1024 <= pd);
	if (big) {
		big[0] = 1;
		big[6UL * 1024 * 1024 - 1] = 2;
		free(big);
	}
	if (!kid) {
		long pre = (long)Malloc(-1L);	/* mintlib keeps freed hunks: measure here */
		long rc = Pexec(0, "C:\\E9MEM.NAP", "\003kid", 0L);
		checkhex("a child starts after the Mshrink and passes", (unsigned long)rc, 0);
		Cconws(me);
		kv("Malloc(-1) after the child", (unsigned long)Malloc(-1L));
		Cconws("\r\n");
		checkhex("everything back after the child", (unsigned long)Malloc(-1L), (unsigned long)pre);
	}
	Cconws(me);
	Cconws(fails ? " E9MEM: FAIL\r\n" : " E9MEM: PASS\r\n");
	return fails;
}
