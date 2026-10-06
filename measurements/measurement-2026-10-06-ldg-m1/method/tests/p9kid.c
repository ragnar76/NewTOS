/*
 * p9kid.c -- TASK/PORT09 case suite, CHILD. Prints what the libc start-up
 * (sdk/libc/crt0.S -> heap_init.c -> newtos-arch/cmdargs.c) made of its
 * environment block and command tail -- argc/argv, environ,
 * program_invocation_name, the raw p_env records after the start-up -- and
 * checks the expectation of the case named by argv[1] (p9par.c). Returns
 * 0x42 when every check holds.
 */
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include "common.h"

extern char **environ;
/* mintlib/program-name.c (declared by <errno.h> only under __USE_GNU) */
extern char *program_invocation_name;
extern char *program_invocation_short_name;

static int streq(const char *a, const char *b)
{
	return a && b && strcmp(a, b) == 0;
}

static int env_is(const char *const *want)
{
	int k;

	if (!environ)
		return 0;
	for (k = 0; want[k]; k++)
		if (!streq(environ[k], want[k]))
			return 0;
	return environ[k] == NULL;
}

int main(int argc, char **argv)
{
	const char *p = (const char *)(unsigned long)_base->p_env;
	const char *r;
	int k;

	Cconws("[P9KID] argc ");
	puthex((unsigned long)argc);
	Cconws("\r\n");
	for (k = 0; k <= argc; k++) {
		Cconws("[P9KID] argv[");
		puthex((unsigned long)k);
		Cconws("] ");
		if (argv[k]) {
			Cconws("\"");
			Cconws(argv[k]);
			Cconws("\"\r\n");
		} else
			Cconws("NULL\r\n");
	}
	for (k = 0; environ && environ[k]; k++) {
		Cconws("[P9KID] environ \"");
		Cconws(environ[k]);
		Cconws("\"\r\n");
	}
	Cconws("[P9KID] program_invocation_name \"");
	Cconws(program_invocation_name);
	Cconws("\" short \"");
	Cconws(program_invocation_short_name);
	Cconws("\"\r\n");
	for (r = p; r && *r; ) {
		Cconws("[P9KID] p_env rec \"");
		Cconws(r);
		Cconws("\"\r\n");
		while (*r) r++;
		r++;
	}

	if (argc >= 2 && streq(argv[1], "alpha-one")) {
		static const char *const e[] = { "PATH=C:\\", NULL };
		Cconws("[P9KID] case argv14\r\n");
		checkhex("argc", (unsigned long)argc, 15);
		check("argv[0] = C:\\P9KID.NAP", streq(argv[0], "C:\\P9KID.NAP"));
		check("argv[12] = lima-twelve", argc > 12 && streq(argv[12], "lima-twelve"));
		check("argv[13] = mike thirteen has blanks (one argument)",
		      argc > 13 && streq(argv[13], "mike thirteen has blanks"));
		check("argv[14] = november-fourteen-is-the-last-argument",
		      argc > 14 && streq(argv[14], "november-fourteen-is-the-last-argument"));
		check("argv[15] = NULL", argc == 15 && argv[15] == NULL);
		check("environ = {PATH=C:\\} (cut at ARGV=)", env_is(e));
		check("getenv(PATH) = C:\\", streq(getenv("PATH"), "C:\\"));
		check("getenv(ARGV) = NULL", getenv("ARGV") == NULL);
		check("program_invocation_short_name = P9KID.NAP",
		      streq(program_invocation_short_name, "P9KID.NAP"));
		check("p_env block cut: no ARGV= record left", p && !has_record(p, "ARGV="));
	} else if (argc >= 2 && streq(argv[1], "case-null")) {
		static const char *const e[] = { NULL };
		Cconws("[P9KID] case null\r\n");
		checkhex("argc", (unsigned long)argc, 4);
		check("argv[0] = C:\\P9KID.NAP", streq(argv[0], "C:\\P9KID.NAP"));
		check("argv[2] = \"\" (ARGV=NULL:2)", argc > 2 && streq(argv[2], ""));
		check("argv[3] = last", argc > 3 && streq(argv[3], "last"));
		check("environ empty", env_is(e));
	} else if (argc >= 2 && streq(argv[1], "case-tail")) {
		static const char *const e[] = { "FOO=bar", "P9=yes", NULL };
		Cconws("[P9KID] case tail\r\n");
		checkhex("argc", (unsigned long)argc, 4);
		check("argv[0] = \"\" (no ARGV=)", streq(argv[0], ""));
		check("argv[2..3] = x y", argc == 4 && streq(argv[2], "x") && streq(argv[3], "y"));
		check("environ = {FOO=bar, P9=yes}", env_is(e));
		check("getenv(P9) = yes", streq(getenv("P9"), "yes"));
		check("program_invocation_name untouched (mintlib default)",
		      streq(program_invocation_name, "unknown application"));
	} else if (argc >= 2 && streq(argv[1], "case-empty")) {
		static const char *const e[] = { NULL };
		Cconws("[P9KID] case empty\r\n");
		checkhex("argc", (unsigned long)argc, 2);
		check("environ empty (block {0,0})", env_is(e));
		check("getenv(PATH) = NULL", getenv("PATH") == NULL);
	} else if (argc >= 2 && streq(argv[1], "case-inherit")) {
		static const char *const e[] = { "PATH=C:\\", NULL };
		Cconws("[P9KID] case inherit\r\n");
		checkhex("argc", (unsigned long)argc, 2);
		check("environ = parent's joined {PATH=C:\\}", env_is(e));
	} else {
		check("known case in argv[1]", 0);
	}
	return fails ? 1 : 0x42;
}
