/*
 * envkid.c -- MEM03 acceptance, CHILD: read p_env record by record, the way
 * any TOS startup does (mintlib crtinit.c parseargs, TeraDesk environm.c),
 * and answer 0x42 when the parent's records all arrived. Before ARCH03 stage
 * 1 the first read faulted (p_env was a physical address the child had not
 * mapped, TASK/MEM03.md).
 */
#include <string.h>
#include "common.h"

/*
 * TASK/PORT09: the libc start-up now walks p_env as mintlib's parseargs
 * does and cuts the block in place at "ARGV=" (its 'A' becomes NUL), so
 * after main() is entered the block holds only "PATH=C:\"; the arguments
 * arrive in argv. The two record checks that looked for "ARGV=" and the
 * blank argument in the raw block are therefore checks on argv and on the
 * cut (measurement-2026-10-06-port09/RUNINFO.TXT).
 */
int main(int argc, char **argv)
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
	check("ARGV= consumed by the start-up (block cut)", !has_record(p, "ARGV="));
	check("argv[0] = C:\\ENVKID.NAP (ARGV=)", argc == 3 && strcmp(argv[0], "C:\\ENVKID.NAP") == 0);
	check("an argument with blanks arrived as argv[2]",
	      argc == 3 && strcmp(argv[2], "mike thirteen has blanks") == 0);
	return fails ? 1 : 0x42;
}
