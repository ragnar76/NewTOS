/*
 * waith-host.c -- host harness for waith-probe.c (TASK/PORT-WAITH). Built
 * with the HOST headers; the probe (mintlib headers) reports every check
 * through waith_report(). Exit status = number of failed checks; 77 = skip.
 */
#include <stdio.h>
#include <stdlib.h>

int waith_probe (void);

static int checks, fails;

void
waith_report (const char *what, long got, long want)
{
	checks++;
	if (got == want)
		printf ("PASS  %s = %ld (0x%lx)\n", what, got, (unsigned long) got);
	else {
		printf ("FAIL  %s = %ld (0x%lx), want %ld (0x%lx)\n", what, got,
			(unsigned long) got, want, (unsigned long) want);
		fails++;
	}
}

int
waith_skip (const char *why)
{
	printf ("waith-host: skipped -- %s\n", why);
	exit (77);
}

int
main (void)
{
	waith_probe ();
	printf ("waith-host: %d checks, %d failed\n", checks, fails);
	return fails > 125 ? 125 : fails;
}
