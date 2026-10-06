/*
 * atexitp.c -- post-E9 point 2: the driver. Runs ATEXITK.NAP (Pexec 0),
 * checks its exit code (main's return value through exit()) and reads back
 * C:\ATEXIT.TXT: the text the child left in an unflushed stdio buffer and
 * the text its atexit handler wrote must both be there, in that order.
 * Ends with "ATEXIT: PASS" / "ATEXIT: FAIL".
 */
#include "common.h"

static int same(const char *a, const char *b)
{
	while (*a && *a == *b) { a++; b++; }
	return *a == *b;
}

int main(void)
{
	static char buf[128];
	long rc, fh, n;

	(void)Fdelete("C:\\ATEXIT.TXT");
	rc = Pexec(0, "C:\\ATEXITK.NAP", "\0", 0L);
	Cconws("\r\n");
	checkhex("ATEXITK exit code = main's return (via exit)", (unsigned long)rc, 7);
	fh = Fopen("C:\\ATEXIT.TXT", 0);
	if (fh < 0) {
		check("C:\\ATEXIT.TXT exists", 0);
	} else {
		n = Fread((short)fh, sizeof buf - 1, buf);
		Fclose((short)fh);
		buf[n > 0 ? n : 0] = 0;
		Cconws("[ATEXITP] file: \"");
		Cconws(buf);
		Cconws("\"\r\n");
		check("unflushed stdio data + atexit handler output reached the file",
		      same(buf, "main-body;atexit-handler;"));
	}
	Cconws(fails ? "ATEXIT: FAIL\r\n" : "ATEXIT: PASS\r\n");
	return fails;
}
