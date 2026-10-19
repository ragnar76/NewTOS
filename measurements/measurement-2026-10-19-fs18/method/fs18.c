/*
 * fs18.c -- TASK/FS18 + TASK/FS14 probe WITH expectations: Frename as EmuTOS xrename (bdos/fsdir.c:895-1101).
 * F18.NCP runs from C:\AUTO (EL0 console program before the AES). Every check prints "[F18] ok|FAIL ...";
 * ends with "F18 RESULT fails=<n>". The card is fsck'ed (fsck_msdos -n) and listed after the run.
 *   A  same directory, open file: Fcreate C:\R18A\D.TXT, Frename -> E.TXT, Fwrite 3000, Fclose; E.TXT = 3000 bytes
 *      intact; a second file C:\R18A\KEEP.TXT goes the same way and STAYS on the card (fsck checks its chain).
 *      Fdelete E.TXT -> Dfree back to the value before the Fcreate (no leaked clusters).
 *   B  order: C:\R18O\A.TXT, B.TXT, Frename A -> C: Fsfirst lists C.TXT before B.TXT (name overwritten in place).
 *   C  across directories, open file: C:\R18B\D.TXT -> C:\R18C\E.TXT, Fwrite 3000, Fclose; E.TXT = 3000 bytes intact
 *      (NewTOS deviation, docs/COMPATIBILITY.md: the OFD follows the moved entry), C:\R18B empty; stays on the card.
 *   D  FS14: Frename(C:\R14\X.TXT, "C:\") -> EPTHNF, ".Y" -> EPTHNF; no blank entry in C:\, X.TXT still in C:\R14.
 *   E  attribute: C:\R18D\M.TXT set hidden (0x02), moved to C:\R18E\N.TXT -> Fattrib 0x22 (FA_ARCHIVE from ixclose);
 *      a same-directory rename keeps 0x02.
 *   F  error order: target exists first (EACCDN), then source (EFILNF), then target directory (EPTHNF).
 */
#include "common.h"
#include <string.h>

static void say(const char *s) { Cconws("[F18] "); Cconws(s); }

static void ckx(const char *what, long got, long want)
{
	say(got == want ? "ok   " : "FAIL "); Cconws(what);
	Cconws(" got="); puthex((unsigned long)got); Cconws(" want="); puthex((unsigned long)want);
	Cconws("\r\n");
	if (got != want) fails++;
}

static void ck(const char *what, int ok)
{
	say(ok ? "ok   " : "FAIL "); Cconws(what); Cconws("\r\n");
	if (!ok) fails++;
}

static char big[3000];
static char rd[3100];

static long slurp(const char *name)
{
	long h = Fopen(name, 0), n;
	if (h < 0) return h;
	n = Fread((short)h, sizeof(rd), rd);
	Fclose((short)h);
	return n;
}

static long free_clusters(void)
{
	_DISKINFO di;
	if (Dfree(&di, 3) != 0) return -1;
	return (long)di.b_free;
}

/* Names of glob's entries, comma-joined into out; returns the count. A blank name shows as "<>". */
static long list(const char *glob, char *out, int max)
{
	_DTA dta;
	long n = 0, r;
	out[0] = 0;
	Fsetdta(&dta);
	for (r = Fsfirst(glob, 0x37); r == 0; r = Fsnext()) {
		const char *nm = dta.dta_name[0] ? dta.dta_name : "<>";
		if ((int)(strlen(out) + strlen(nm) + 2) < max) {
			if (n) strcat(out, ",");
			strcat(out, nm);
		}
		n++;
	}
	return n;
}

static void show(const char *what, const char *glob)
{
	char buf[200];
	long n = list(glob, buf, sizeof(buf));
	say(what); Cconws(" "); Cconws(glob); Cconws(": "); Cconws(buf); Cconws(" n="); puthex((unsigned long)n);
	Cconws("\r\n");
}

static void fill(char seed)
{
	int i;
	for (i = 0; i < (int)sizeof(big); i++) big[i] = (char)(seed + i % 23);
}

static void open_rename_write(const char *tag, const char *from, const char *to, char seed)
{
	long h, n;
	fill(seed);
	h = Fcreate(from, 0);
	say(tag); Cconws(" "); Cconws(from); Cconws(" -> "); Cconws(to); Cconws("\r\n");
	ckx("  Fcreate >= 6", h >= 6, 1);
	ckx("  Frename of the open file", Frename(0, from, to), 0);
	ckx("  Fwrite 3000", Fwrite((short)h, sizeof(big), big), 3000);
	ckx("  Fclose", Fclose((short)h), 0);
	n = slurp(to);
	ck("  new name reads back 3000 bytes intact", n == 3000 && memcmp(rd, big, 3000) == 0);
	ckx("  old name gone (Fopen -> EFILNF)", Fopen(from, 0), -33);
}

int main(void)
{
	long before, h;
	char buf[200];

	Cconws("[F18] start: Frename in place / across directories (TASK/FS18, FS14)\r\n");

	/* A */
	ckx("A Dcreate C:\\R18A", Dcreate("C:\\R18A"), 0);
	before = free_clusters();
	open_rename_write("A", "C:\\R18A\\D.TXT", "C:\\R18A\\E.TXT", 'a');
	ckx("A Fdelete E.TXT", Fdelete("C:\\R18A\\E.TXT"), 0);
	ckx("A Dfree free clusters back (no leak)", free_clusters(), before);
	open_rename_write("A2", "C:\\R18A\\K.TXT", "C:\\R18A\\KEEP.TXT", 'k');
	show("A", "C:\\R18A\\*.*");

	/* B */
	ckx("B Dcreate C:\\R18O", Dcreate("C:\\R18O"), 0);
	h = Fcreate("C:\\R18O\\A.TXT", 0); Fclose((short)h);
	h = Fcreate("C:\\R18O\\B.TXT", 0); Fclose((short)h);
	ckx("B Frename A -> C", Frename(0, "C:\\R18O\\A.TXT", "C:\\R18O\\C.TXT"), 0);
	list("C:\\R18O\\*.*", buf, sizeof(buf));
	say("B order: "); Cconws(buf); Cconws("\r\n");
	ck("B order .,..,C.TXT,B.TXT", strcmp(buf, ".,..,C.TXT,B.TXT") == 0);

	/* C */
	ckx("C Dcreate C:\\R18B", Dcreate("C:\\R18B"), 0);
	ckx("C Dcreate C:\\R18C", Dcreate("C:\\R18C"), 0);
	before = free_clusters();
	open_rename_write("C", "C:\\R18B\\D.TXT", "C:\\R18C\\E.TXT", 'c');
	ckx("C C:\\R18B holds only . and ..", list("C:\\R18B\\*.*", buf, sizeof(buf)), 2);
	ckx("C Dfree: 3000 bytes in use", before - free_clusters() > 0, 1);
	show("C", "C:\\R18C\\*.*");

	/* D */
	ckx("D Dcreate C:\\R14", Dcreate("C:\\R14"), 0);
	h = Fcreate("C:\\R14\\X.TXT", 0); Fclose((short)h);
	ckx("D Frename(C:\\R14\\X.TXT, C:\\) -> EPTHNF", Frename(0, "C:\\R14\\X.TXT", "C:\\"), -34);
	ckx("D Frename(C:\\R14\\X.TXT, C:\\.Y) -> EPTHNF", Frename(0, "C:\\R14\\X.TXT", "C:\\.Y"), -34);
	list("C:\\*.*", buf, sizeof(buf));
	ck("D no blank entry in C:\\", strstr(buf, "<>") == 0);
	ckx("D X.TXT still in C:\\R14", list("C:\\R14\\*.*", buf, sizeof(buf)), 3);
	show("D", "C:\\*.*");

	/* E */
	ckx("E Dcreate C:\\R18D", Dcreate("C:\\R18D"), 0);
	ckx("E Dcreate C:\\R18E", Dcreate("C:\\R18E"), 0);
	h = Fcreate("C:\\R18D\\M.TXT", 0); Fwrite((short)h, 5, "moved"); Fclose((short)h);
	ckx("E Fattrib set hidden", Fattrib("C:\\R18D\\M.TXT", 1, 0x02), 0x02);
	ckx("E same-dir Frename M -> L", Frename(0, "C:\\R18D\\M.TXT", "C:\\R18D\\L.TXT"), 0);
	ckx("E same-dir keeps 0x02", Fattrib("C:\\R18D\\L.TXT", 0, 0), 0x02);
	ckx("E cross-dir Frename -> C:\\R18E\\N.TXT", Frename(0, "C:\\R18D\\L.TXT", "C:\\R18E\\N.TXT"), 0);
	ckx("E moved file gains FA_ARCHIVE (0x22)", Fattrib("C:\\R18E\\N.TXT", 0, 0), 0x22);
	ckx("E content", slurp("C:\\R18E\\N.TXT"), 5);

	/* F */
	ckx("F target exists before missing source dir -> EACCDN", Frename(0, "C:\\NODIR\\X.TXT", "C:\\R18O\\B.TXT"), -36);
	ckx("F missing source before missing target dir -> EFILNF", Frename(0, "C:\\NOFILE.TXT", "C:\\NODIR\\Y.TXT"), -33);
	ckx("F missing target dir -> EPTHNF", Frename(0, "C:\\R18O\\B.TXT", "C:\\NODIR\\Y.TXT"), -34);

	Cconws("F18 RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
	return 0;
}
