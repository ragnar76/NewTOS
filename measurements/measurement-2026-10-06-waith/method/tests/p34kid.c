/*
 * p34kid.c -- ARCH03 stage 1, child of p34par.c (started with Pexec 4).
 * Checks what the parent prepared and what proc_go sets, answers 0x34.
 */
#include "common.h"

static unsigned long parse_hex(const char *s, int n)
{
	unsigned long v = 0;
	while (n-- > 0) {
		char c = *s++;
		v = (v << 4) | (unsigned long)(c <= '9' ? c - '0' : c - 'a' + 10);
	}
	return v;
}

int main(void)
{
	unsigned long me = (unsigned long)_base;
	unsigned long parent = parse_hex(&_base->p_cmdlin[1], 8);
	void *blk;

	Cconws("[P34] child: _base=");
	puthex(me);
	Cconws(" p_parent=");
	puthex(_base->p_parent);
	Cconws(" p_hitpa=");
	puthex(_base->p_hitpa);
	Cconws("\r\n");
	checkhex("child: *(os_run) == _base", os_run_pd(), me);
	checkhex("child: p_parent == the parent's _base", _base->p_parent, parent);
	check("child: environment readable", _base->p_env != 0
	      && has_record((const char *)(unsigned long)_base->p_env, "P34=yes"));
	/* A block of our own (owned by the child, freed at its Pterm). */
	blk = (void *)Malloc(4096);
	check("child: Malloc", blk != 0);
	if (blk)
		*(volatile long *)blk = 0x1234;
	return fails ? 1 : 0x34;
}
