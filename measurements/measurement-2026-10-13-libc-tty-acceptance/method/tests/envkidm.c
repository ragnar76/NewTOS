/*
 * envkidm.c -- crt0-main replacement for envkidj.c (MEM03 child). Since the
 * libc crt0 runs mintlib's `_main` (TASK/LIBC-MAIN), `__libc_main` rewrites
 * PATH in environ to the Unix form (mintlib/main.c:268-288: malloc'ed copy,
 * _path_dos2unx, unx2dos.c:217; with no MiNT cookie and no UNIXMODE r<c>,
 * _rootdir = 0 and `C:\` becomes `/dev/c/`). envkidj.c compared getenv("PATH")
 * with "C:\" and so fails by design. This child checks the same facts as
 * envkidj.c, PATH in both views:
 *   - getenv("PATH") == "/dev/c/" (mintlib's _main conversion), and
 *   - the raw block p_env still starts with "PATH=C:\" (the parent's
 *     explicit environment; parseargs joined nothing here, _main never
 *     writes into the block),
 * plus argc/argv from ARGV= as before. Answers 0x42 when everything arrived.
 */
#include "common.h"
#include <string.h>
#include <stdlib.h>

int main(int argc, char **argv, char **envp)
{
	const char *path = getenv("PATH");
	const char *raw = (const char *)(unsigned long)_base->p_env;

	Cconws("[MEM03M] child: p_env=");
	puthex((unsigned long)_base->p_env);
	Cconws(" argc=");
	puthex((unsigned long)argc);
	Cconws(" PATH=");
	Cconws(path ? path : "(null)");
	Cconws("\r\n");
	check("PATH=/dev/c/ in environ (getenv, mintlib _main conversion)",
	      path != 0 && strcmp(path, "/dev/c/") == 0);
	check("raw p_env block starts with PATH=C:\\",
	      raw != 0 && strcmp(raw, "PATH=C:\\") == 0);
	check("argc == 3", argc == 3);
	check("argv[1] == alpha-one", argc > 1 && strcmp(argv[1], "alpha-one") == 0);
	check("an argument with blanks arrived (argv[2])",
	      argc > 2 && strcmp(argv[2], "mike thirteen has blanks") == 0);
	(void)envp;
	return fails ? 1 : 0x42;
}
