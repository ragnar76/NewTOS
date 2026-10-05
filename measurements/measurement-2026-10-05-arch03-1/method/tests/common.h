/* common.h -- tiny serial-console helpers for the ARCH03 stage-1 probes. */
#include <mint/basepage.h>
#include <mint/osbind.h>

static void puthex(unsigned long v)
{
	static const char d[] = "0123456789abcdef";
	char b[11];
	int i;

	b[0] = '0';
	b[1] = 'x';
	for (i = 0; i < 8; i++)
		b[2 + i] = d[(v >> ((7 - i) * 4)) & 0xf];
	b[10] = '\0';
	Cconws(b);
}

static int fails;

static void check(const char *what, int ok)
{
	Cconws(ok ? "  ok   " : "  FAIL ");
	Cconws(what);
	Cconws("\r\n");
	if (!ok)
		fails++;
}

static void checkhex(const char *what, unsigned long got, unsigned long want)
{
	Cconws(got == want ? "  ok   " : "  FAIL ");
	Cconws(what);
	Cconws(" got=");
	puthex(got);
	Cconws(" want=");
	puthex(want);
	Cconws("\r\n");
	if (got != want)
		fails++;
}

/* The current PD the classic way: *(_sysbase->os_run) (OSHEADER +0x28). */
static unsigned long os_run_pd(void)
{
	unsigned long sysbase = *(volatile unsigned int *)0x4F2UL;
	unsigned long cell = *(volatile unsigned int *)(sysbase + 0x28);
	return *(volatile unsigned int *)cell;
}

static int has_record(const char *env, const char *rec)
{
	const char *p = env;
	while (*p) {
		const char *a = p, *b = rec;
		while (*a && *a == *b) { a++; b++; }
		if (*a == '\0' && *b == '\0')
			return 1;
		while (*p) p++;
		p++;
	}
	return 0;
}
