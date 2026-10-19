/*
 * libcbuf.c -- TASK/LIBC-BUF observation probe (ticket review 2026-10-19), C:\AUTO\LIBCBUF.NCP.
 * Question: does a FILE from fopen() whose FIRST output is fprintf() end up with a 1 KiB stdio buffer
 * (vfprintf buffered_vfprintf path: __buffer == NULL -> local BUFSIZ buffer, then __bufsize = 0 ->
 * init_stream picks BUFSIZ) instead of the 32 KiB st_blksize the FS08 rule (0abc634) intends?
 * Control: a second fopen() FILE whose first output is fputs().
 * All reporting goes through Cconws (not stdio), so the probe does not disturb what it measures.
 * Ends with "LB RESULT" (observation only; no expectation is asserted).
 */
#include <mint/osbind.h>
#include <stdio.h>
#include <string.h>

static void puthex(unsigned long v)
{
	static const char d[] = "0123456789abcdef";
	char b[19];
	int i;
	b[0] = '0';
	b[1] = 'x';
	for (i = 0; i < 16; i++)
		b[2 + i] = d[(v >> ((15 - i) * 4)) & 0xf];
	b[18] = '\0';
	Cconws(b);
}

static void show(const char *tag, FILE *f)
{
	Cconws("[LB] ");
	Cconws(tag);
	Cconws(" bufsize=");
	puthex((unsigned long)f->__bufsize);
	Cconws(" buffer=");
	puthex((unsigned long)(f->__buffer != NULL));
	Cconws("\r\n");
}

int main(void)
{
	FILE *a, *b;

	Cconws("[LB] start\r\n");
	a = fopen("C:\\LB1.TXT", "w");
	if (!a) { Cconws("[LB] fopen LB1 NULL\r\n"); return 1; }
	show("A fopen", a);
	fprintf(a, "x%d", 1);
	show("A after first fprintf", a);
	fputs("yy", a);
	show("A after fputs", a);
	fclose(a);

	b = fopen("C:\\LB2.TXT", "w");
	if (!b) { Cconws("[LB] fopen LB2 NULL\r\n"); return 1; }
	show("B fopen", b);
	fputs("yy", b);
	show("B after first fputs", b);
	fprintf(b, "x%d", 1);
	show("B after fprintf", b);
	fclose(b);

	show("stdout before printf", stdout);
	printf("[LB] printf line\r\n");
	show("stdout after printf", stdout);
	Cconws("LB RESULT\r\n");
	return 0;
}
