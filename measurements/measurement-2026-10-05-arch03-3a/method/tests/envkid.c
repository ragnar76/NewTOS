/*
 * envkid.c -- MEM03 acceptance, CHILD: read p_env record by record, the way
 * any TOS startup does (mintlib crtinit.c parseargs, TeraDesk environm.c),
 * and answer 0x42 when the parent's records all arrived. Before ARCH03 stage
 * 1 the first read faulted (p_env was a physical address the child had not
 * mapped, TASK/MEM03.md).
 */
#include "common.h"

int main(void)
{
	const char *p = (const char *)(unsigned long)_base->p_env;
	const char *r;

	Cconws("[MEM03] child: p_env=");
	puthex((unsigned long)_base->p_env);
	Cconws("\r\n");
	if (p == 0) {
		Cconws("[MEM03] child: no environment\r\n");
		return 1;
	}
	for (r = p; *r; ) {
		Cconws("[MEM03] child: rec \"");
		Cconws(r);
		Cconws("\"\r\n");
		while (*r) r++;
		r++;
	}
	check("PATH=C:\\ arrived", has_record(p, "PATH=C:\\"));
	check("ARGV= arrived", has_record(p, "ARGV="));
	check("an argument with blanks arrived", has_record(p, "mike thirteen has blanks"));
	return fails ? 1 : 0x42;
}
