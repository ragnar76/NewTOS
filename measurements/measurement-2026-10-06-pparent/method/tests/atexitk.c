/*
 * atexitk.c -- post-E9 point 2: the child. It writes through a stdio FILE
 * it never closes and never flushes, registers an atexit handler that
 * writes to the same FILE, prints a stdout line without a newline, and
 * RETURNS from main with 7. With mintlib's start-up (crt0 -> _main =
 * __libc_main -> exit(main(...)), mintlib/main.c:302) exit() runs the
 * handler and flushes every stream before _exit: C:\ATEXIT.TXT then holds
 * "main-body;atexit-handler;" and the stdout tail appears. With a crt0 that
 * Pterms right after main, nothing of it reaches the file.
 */
#include <stdio.h>
#include <stdlib.h>
#include <mint/osbind.h>

static FILE *f;

static void handler(void)
{
	fputs("atexit-handler;", f);
	printf("[ATEXITK] atexit handler ran;");
}

int main(void)
{
	f = fopen("C:\\ATEXIT.TXT", "w");
	if (!f) {
		Cconws("[ATEXITK] fopen failed\r\n");
		return 1;
	}
	fputs("main-body;", f);
	if (atexit(handler) != 0) {
		Cconws("[ATEXITK] atexit failed\r\n");
		return 2;
	}
	printf("[ATEXITK] buffered stdout, no newline;");
	return 7;
}
