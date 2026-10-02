/*
 * spawnxxx stuff for MiNT/TOS; written by Eric R. Smith, and
 * placed in the public domain
 */

/* NewTOS/AArch64 (TASK/PORT10): spawnle/spawnl walked their variadic
 * arguments by casting the va_list to `char **' -- on m68k a va_list IS a
 * pointer into the caller's stack, where the arguments lie as a NULL-
 * terminated array of 4-byte pointers. An AAPCS64 va_list is a structure
 * (register save areas + stack pointer), so the cast does not compile and
 * would not mean anything if it did. Here the same arguments are fetched
 * with va_arg into a stack array of the same shape (counted first over a
 * va_copy), and handed to _spawnve exactly as before. spawnve/spawnv are
 * unchanged. */

#include <stdarg.h>
#include <process.h>
#include "lib.h"


int
spawnve(int mode, const char *path, char *const *argv, char *const *envp)
{
	return _spawnve(mode, path, argv, envp);
}

int
spawnv(int mode, const char *path, char *const *argv)
{
	return _spawnve(mode, path, argv, environ);
}

/* Number of arguments up to (not including) the terminating NULL. */
static size_t
count_args(va_list args)
{
	va_list scan;
	size_t n = 0;

	va_copy(scan, args);
	while (va_arg(scan, char *) != NULL)
		n++;
	va_end(scan);
	return n;
}

int
spawnle(int mode, const char *path, ...)
{
	va_list args;
	char *const *envp;
	size_t n, i;
	int	r;

	va_start(args, path);
	n = count_args(args);
	{
		char *argv[n + 1];

		for (i = 0; i < n; i++)
			argv[i] = va_arg(args, char *);
		argv[n] = va_arg(args, char *);	/* the terminating NULL */
		/* the environment follows the NULL, as in the m68k
		 * `*(envp+1)' */
		envp = va_arg(args, char *const *);
		r = _spawnve(mode, path, argv, envp);
	}
	va_end(args);
	return r;
}

int
spawnl(int mode, const char *path, ...)
{
	va_list args;
	size_t n, i;
	int r;

	va_start(args, path);
	n = count_args(args);
	{
		char *argv[n + 1];

		for (i = 0; i <= n; i++)
			argv[i] = va_arg(args, char *);
		r = _spawnve(mode, path, argv, environ);
	}
	va_end(args);
	return r;
}
