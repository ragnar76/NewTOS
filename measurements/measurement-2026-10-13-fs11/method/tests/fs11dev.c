/*
 * fs11dev.c -- TASK/FS11 probe: Fread/Fwrite on the character-device handles
 * through the BIOS device table (docs/api/gemdos/file/3f-fread.md and
 * 40-fwrite.md, "NewTOS host tests" R1-R32, W1-W27), and mintlib getchar()/fgets() on
 * stdin over the new line read.
 *
 * Part A needs no input: AUX/PRN are the documented absent devices (Bconin 0
 * without blocking, Bconout 0), so Fread on -2/-3/2/3 returns NUL bytes and
 * Fwrite the short count 0; the HUGE / length guards; the BIOS/GEMDOS
 * pass-throughs and Midiws.
 *
 * Part B reads the keyboard; the keys come from the scripted IKBD injector of
 * the scratch "keys" kernel (method/fs11-schedule.py), in this order:
 *   'a'                  -> Fread(-1, 1)   = 1, "a"           (conin)
 *   'h' 'i' Return       -> Fread(-1, 80)  = 2, "hi"          (cgets)
 *   'o' BS 'k' Return    -> Fread(0, 80)   = 1, "k"           (stdin, BS edit)
 * Part C is mintlib over stdin:
 *   'a' 'b' Return       -> getchar() = 'a', fgets() = "b\n"
 *   'c' 'o' 'p' Return   -> fgets() = "cop\n"
 * NewTOS' isatty shim (sdk/libc/newtos-arch/isatty.c) never classifies the
 * tty, so __open_stat[].flags stays 0 and mintlib's __read takes its RAW
 * path (_console_read_byte -> Crawcin, unix/read.c:173-217) instead of
 * Fread -- found by the first run of this probe (serial/smoke-on-...).
 * Part C therefore sets the flags upstream mintlib's own __isatty would set
 * for a tty (CRMOD|ECHO, mintlib/isatty.c:55-57) before reading, so the
 * cooked TOS path runs: Fread(0, size), then mintlib appends the CR the line
 * read does not store and maps it to '\n' (unix/read.c:147-169).
 */
#include <stdio.h>
#include <string.h>
#include "common.h"

/* mintlib/lib.h (internal): the tty status table, indexed by fd + 3. */
struct __open_file { short status; short flags; short check_eagain; };
extern struct __open_file __open_stat[];
#define CRMOD 0x0001
#define ECHO_FLAG 0x0004

#define EIHNDL (-37L)

static char buf[256];

static int all_zero(const char *p, long n)
{
	long i;
	for (i = 0; i < n; i++)
		if (p[i] != 0)
			return 0;
	return 1;
}

static void showbytes(const char *what, const char *p, long n)
{
	long i;
	Cconws("  ");
	Cconws(what);
	Cconws(" bytes:");
	for (i = 0; i < n && i < 16; i++) {
		Cconws(" ");
		puthex((unsigned char)p[i]);
	}
	Cconws("\r\n");
}

static void part_a(void)
{
	long r;
	short h;

	Cconws("FS11 part A (no input)\r\n");
	/* Fwrite: absent AUX/PRN refuse the first byte -> short count 0. */
	checkhex("Fwrite(-2,4,ATZ)", (unsigned long)Fwrite(-2, 4L, "ATZ\r"), 0);
	checkhex("Fwrite(2,4,ATZ)", (unsigned long)Fwrite(2, 4L, "ATZ\r"), 0);
	checkhex("Fwrite(-3,3,abc)", (unsigned long)Fwrite(-3, 3L, "abc"), 0);
	checkhex("Fwrite(3,3,abc)", (unsigned long)Fwrite(3, 3L, "abc"), 0);
	checkhex("Fwrite(-2,0)", (unsigned long)Fwrite(-2, 0L, "x"), 0);
	checkhex("Fwrite(-3,-1)", (unsigned long)Fwrite(-3, -1L, "x"), (unsigned long)-1L);
	checkhex("Fwrite(-1,-5)", (unsigned long)Fwrite(-1, -5L, "x"), (unsigned long)-5L);
	checkhex("Fwrite(-1,7,con)", (unsigned long)Fwrite(-1, 7L, "[con]\r\n"), 7);
	checkhex("Fwrite(1,7,std)", (unsigned long)Fwrite(1, 7L, "[std]\r\n"), 7);
	checkhex("Fwrite(-4,1)", (unsigned long)Fwrite(-4, 1L, "x"), (unsigned long)EIHNDL);

	/* Fread: absent AUX/PRN deliver NULs and never block. */
	for (h = -3; h <= 3; h++) {
		if (h == -1 || h == 0 || h == 1)
			continue;
		memset(buf, 0xAA, sizeof buf);
		r = Fread(h, 1L, buf);
		checkhex(h == -3 ? "Fread(-3,1)" : h == -2 ? "Fread(-2,1)" : h == 2 ? "Fread(2,1)" : "Fread(3,1)",
			 (unsigned long)r, 1);
		check("  ... byte is NUL", buf[0] == 0);
		memset(buf, 0xAA, sizeof buf);
		r = Fread(h, 80L, buf);
		checkhex(h == -3 ? "Fread(-3,80)" : h == -2 ? "Fread(-2,80)" : h == 2 ? "Fread(2,80)" : "Fread(3,80)",
			 (unsigned long)r, 80);
		check("  ... 80 NULs", all_zero(buf, 80));
	}
	/* Guards (no device touched, no input consumed). */
	checkhex("Fread(-2,0x10000)", (unsigned long)Fread(-2, 0x10000L, buf), 0);
	checkhex("Fread(-3,0x8000)", (unsigned long)Fread(-3, 0x8000L, buf), 0);
	checkhex("Fread(-1,0)", (unsigned long)Fread(-1, 0L, buf), 0);
	checkhex("Fread(-1,-1)", (unsigned long)Fread(-1, -1L, buf), 0);
	checkhex("Fread(-1,0xFFFF)", (unsigned long)Fread(-1, 0xFFFFL, buf), 0);
	checkhex("Fread(-4,1)", (unsigned long)Fread(-4, 1L, buf), (unsigned long)EIHNDL);

	/* Pass-throughs to the same table entries. */
	checkhex("Cauxin", (unsigned long)Cauxin(), 0);
	checkhex("Cauxout", (unsigned long)trap_1_ww((short)0x04, (short)'x') /* Cauxout: osbind casts to void */, 0);
	checkhex("Cauxis", (unsigned long)Cauxis(), 0);
	checkhex("Cauxos", (unsigned long)Cauxos(), 0);
	checkhex("Cprnout", (unsigned long)Cprnout('x'), 0);
	checkhex("Cprnos", (unsigned long)Cprnos(), 0);
	checkhex("Bconin(1)", (unsigned long)Bconin(1), 0);
	checkhex("Bconstat(1)", (unsigned long)Bconstat(1), 0);
	checkhex("Bconout(0,x)", (unsigned long)Bconout(0, 'x'), 0);
	checkhex("Bcostat(0)", (unsigned long)Bcostat(0), 0);
	checkhex("Bcostat(2)", (unsigned long)Bcostat(2), (unsigned long)-1L);
	checkhex("Bcostat(3)", (unsigned long)Bcostat(3), 0);
	checkhex("Bcostat(4)", (unsigned long)Bcostat(4), 0);
	checkhex("Bconout(3,x)", (unsigned long)Bconout(3, 'x'), 0);
	checkhex("Cconos", (unsigned long)Cconos(), (unsigned long)-1L);
	Midiws(2, "abc");
	check("Midiws(2,abc) returned", 1);
}

static void part_b(void)
{
	long r;

	Cconws("FS11 part B (keys)\r\n");
	memset(buf, 0xAA, sizeof buf);
	r = Fread(-1, 1L, buf);
	checkhex("Fread(-1,1)", (unsigned long)r, 1);
	checkhex("  ... byte", (unsigned char)buf[0], 'a');
	Cconws("\r\n");

	memset(buf, 0xAA, sizeof buf);
	r = Fread(-1, 80L, buf);
	checkhex("Fread(-1,80)", (unsigned long)r, 2);
	check("  ... \"hi\"", r == 2 && buf[0] == 'h' && buf[1] == 'i');
	showbytes("Fread(-1,80)", buf, 4);
	Cconws("\n");

	memset(buf, 0xAA, sizeof buf);
	r = Fread(0, 80L, buf);
	checkhex("Fread(0,80) with BS", (unsigned long)r, 1);
	check("  ... \"k\"", r == 1 && buf[0] == 'k');
	showbytes("Fread(0,80)", buf, 4);
	Cconws("\n");
}

static void part_c(void)
{
	int c;
	char *s;

	Cconws("FS11 part C (mintlib, cooked tty flags)\r\n");
	__open_stat[0 + 3].status = 1;	/* FH_ISATTY */
	__open_stat[0 + 3].flags = CRMOD | ECHO_FLAG;
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
	/* mintlib's _main does Fforce(2, -1) when argv[0] is empty
	 * (main.c:168-169), so stderr = CON then. Put handle 2 back to its
	 * default (AUX, console.c:60-67) so part A sees the default mapping. */
	Cconws("argv[0] empty (mintlib Fforce(2,-1) done): ");
	Cconws(argc < 1 || !argv[0] || !argv[0][0] ? "yes\r\n" : "no\r\n");
	checkhex("Fforce(2,-2)", (unsigned long)Fforce(2, -2), 0);
	part_a();
	Cconws(fails ? "FS11A: FAIL\r\n" : "FS11A: PASS\r\n");
#ifndef FS11_NO_KEYS
	{
		int before = fails;
		part_b();
		Cconws(fails != before ? "FS11B: FAIL\r\n" : "FS11B: PASS\r\n");
		before = fails;
		part_c();
		Cconws(fails != before ? "FS11C: FAIL\r\n" : "FS11C: PASS\r\n");
	}
#endif
	Cconws(fails ? "FS11DEV: FAIL\r\n" : "FS11DEV: PASS\r\n");
	return fails;
}
