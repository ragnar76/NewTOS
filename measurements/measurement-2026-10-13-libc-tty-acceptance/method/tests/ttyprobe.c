/*
 * ttyprobe.c -- TASK/LIBC-TTY probe: mintlib's own __isatty (mintlib/isatty.c,
 * built again since the NewTOS isatty shim was removed) on NewTOS, and
 * getchar()/fgets() on stdin WITHOUT touching __open_stat[] (FS11DEV part C of
 * measurements/measurement-2026-10-13-fs11/method/tests/fs11dev.c minus its
 * flag setting).
 *
 * Parent (no argument; GEM #Z autorun, argv[0] empty):
 *   T1  isatty(0), isatty(1), isatty(2) = 1; the status table entry of fd 0
 *       carries FH_ISATTY and CRMOD|ECHO (set by __isatty itself).
 *   T2  a raw GEMDOS file handle (Fcreate, empty, never queried) -> isatty 0;
 *       a second file with 10 bytes, positioned at 3 -> isatty 0 and the
 *       position is still 3 afterwards (the probe seeks back); an open() fd
 *       -> isatty 0.
 *   T3  Fforce(1, file) and Pexec(0) of this program as child "K"; the child
 *       answers in its exit code: 0x40 | isatty(1) | isatty(0)<<1 |
 *       isatty(2)<<2 | stdout->__linebuf<<3, and prints one line to stdout
 *       (= the file). Expected 0x46 (stdout redirected: not a tty, fully
 *       buffered; stdin/stderr still the console). Then Fforce(1, -1) puts the
 *       parent's stdout back to CON and the file must hold the child's line.
 *   T4  keys from the scratch IKBD injector (tty-schedule.py):
 *         'a' 'b' Return       -> getchar() = 'a', fgets() = "b\n"
 *         'c' 'o' 'p' Return   -> fgets() = "cop\n"
 *       with __open_stat[] as mintlib left it.
 * Control output via Cconws (console, mirrored to serial).
 */
#include <stdio.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include "common.h"

/* mintlib/lib.h (internal): the tty status table, indexed by fd + 3. */
struct __open_file { short status; short flags; short check_eagain; };
extern struct __open_file __open_stat[];
#define FH_ISATTY 1
#define CRMOD 0x0001
#define ECHO_FLAG 0x0004

#define SELF "C:\\TTYPROBE.NAP"
#define F_RAW "C:\\TTYRAW.TMP"
#define F_POS "C:\\TTYPOS.TMP"
#define F_OPEN "C:\\TTYOPEN.TMP"
#define F_KID "C:\\TTYKID.TMP"

static void showbytes(const char *what, const char *b, unsigned long n)
{
	unsigned long i;
	Cconws("  ");
	Cconws(what);
	Cconws(" bytes:");
	for (i = 0; i < n; i++) {
		static const char d[] = "0123456789abcdef";
		char x[4];
		x[0] = ' ';
		x[1] = d[((unsigned char)b[i]) >> 4];
		x[2] = d[((unsigned char)b[i]) & 0xf];
		x[3] = 0;
		Cconws(x);
	}
	Cconws("\r\n");
}

static int child(void)
{
	int code = 0x40;
	code |= isatty(1) ? 1 : 0;
	code |= isatty(0) ? 2 : 0;
	code |= isatty(2) ? 4 : 0;
	code |= stdout->__linebuf ? 8 : 0;
	printf("TTYCHILD-STDOUT code=%x\n", code);
	return code;
}

static void t1(void)
{
	Cconws("TTY T1 isatty on the standard handles\r\n");
	checkhex("isatty(0)", (unsigned long)isatty(0), 1);
	checkhex("isatty(1)", (unsigned long)isatty(1), 1);
	checkhex("isatty(2)", (unsigned long)isatty(2), 1);
	checkhex("__open_stat[fd0].status", (unsigned long)__open_stat[0 + 3].status, FH_ISATTY);
	checkhex("__open_stat[fd0].flags", (unsigned long)__open_stat[0 + 3].flags, CRMOD | ECHO_FLAG);
}

static void t2(void)
{
	long h, h1, p;
	int fd;

	Cconws("TTY T2 isatty on file handles\r\n");
	/* h1 stays open until the end, so the second Fcreate gets another
	 * handle number than the one already classified (and cached). */
	h1 = Fcreate(F_RAW, 0);
	check("Fcreate raw file >= 6", h1 >= 6);
	if (h1 >= 6)
		checkhex("isatty(raw empty file handle)", (unsigned long)isatty((int)h1), 0);
	h = Fcreate(F_POS, 0);
	check("Fcreate pos file >= 6", h >= 6);
	if (h >= 6) {
		checkhex("Fwrite 10", (unsigned long)Fwrite((short)h, 10, "0123456789"), 10);
		checkhex("Fseek(3, SET)", (unsigned long)Fseek(3, (short)h, 0), 3);
		checkhex("isatty(file handle at 3)", (unsigned long)isatty((int)h), 0);
		p = Fseek(0, (short)h, 1);
		checkhex("position after isatty", (unsigned long)p, 3);
		Fclose((short)h);
	}
	if (h1 >= 6)
		Fclose((short)h1);
	fd = open(F_OPEN, O_WRONLY | O_CREAT | O_TRUNC, 0644);
	check("open() file fd >= 6", fd >= 6);
	if (fd >= 0) {
		checkhex("isatty(open() fd)", (unsigned long)isatty(fd), 0);
		close(fd);
	}
}

static void t3(void)
{
	long h, r, n;
	char tail[4];
	char buf[80];

	Cconws("TTY T3 child with stdout Fforce'd to a file\r\n");
	h = Fcreate(F_KID, 0);
	check("Fcreate kid file >= 6", h >= 6);
	if (h < 6)
		return;
	fflush(stdout);
	checkhex("Fforce(1, file)", (unsigned long)Fforce(1, (short)h), 0);
	tail[0] = 1; tail[1] = 'K'; tail[2] = 0;
	r = Pexec(0, SELF, tail, NULL);
	checkhex("Fforce(1, -1)", (unsigned long)Fforce(1, -1), 0);
	Fclose((short)h);
	checkhex("child code (0x40|isatty1|isatty0<<1|isatty2<<2|linebuf<<3)", (unsigned long)r, 0x46);
	check("child isatty(1) after Fforce(1, file) = 0", r >= 0x40 && (r & 1) == 0);
	check("child stdout fully buffered", r >= 0x40 && (r & 8) == 0);
	h = Fopen(F_KID, 0);
	check("Fopen kid file", h >= 6);
	if (h >= 6) {
		memset(buf, 0, sizeof buf);
		n = Fread((short)h, sizeof buf - 1, buf);
		Fclose((short)h);
		check("kid file holds the child's stdout line",
		      n > 0 && strstr(buf, "TTYCHILD-STDOUT code=46") != NULL);
		showbytes("kid file", buf, n > 0 ? (unsigned long)n : 0);
	}
}

static void t4(void)
{
	char buf[80];
	char *s;
	int c;

	Cconws("TTY T4 getchar/fgets on stdin, flags untouched\r\n");
	checkhex("__open_stat[fd0].flags before reading", (unsigned long)__open_stat[0 + 3].flags,
		 CRMOD | ECHO_FLAG);
	c = getchar();
	checkhex("getchar()", (unsigned long)c, 'a');
	memset(buf, 0, sizeof buf);
	s = fgets(buf, 80, stdin);
	check("fgets #1 not NULL", s != NULL);
	check("fgets #1 = \"b\\n\"", s != NULL && strcmp(buf, "b\n") == 0);
	showbytes("fgets #1", buf, strlen(buf) + 1);

	memset(buf, 0, sizeof buf);
	s = fgets(buf, 80, stdin);
	check("fgets #2 not NULL", s != NULL);
	check("fgets #2 = \"cop\\n\"", s != NULL && strcmp(buf, "cop\n") == 0);
	showbytes("fgets #2", buf, strlen(buf) + 1);
}

int main(int argc, char **argv)
{
	int before;

	if (argc > 1 && argv[1][0] == 'K')
		return child();
	Cconws("TTYPROBE argv[0] empty: ");
	Cconws(argc < 1 || !argv[0] || !argv[0][0] ? "yes\r\n" : "no\r\n");
	before = fails; t1(); Cconws(fails != before ? "TTYT1: FAIL\r\n" : "TTYT1: PASS\r\n");
	before = fails; t2(); Cconws(fails != before ? "TTYT2: FAIL\r\n" : "TTYT2: PASS\r\n");
	before = fails; t3(); Cconws(fails != before ? "TTYT3: FAIL\r\n" : "TTYT3: PASS\r\n");
#ifndef TTY_NO_KEYS
	before = fails; t4(); Cconws(fails != before ? "TTYT4: FAIL\r\n" : "TTYT4: PASS\r\n");
#endif
	Cconws(fails ? "TTYPROBE: FAIL\r\n" : "TTYPROBE: PASS\r\n");
	return fails;
}
