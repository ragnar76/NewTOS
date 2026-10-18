/*
 * p4fdel.c -- TASK/PROC04 point 5 probe WITH expectations: Fdelete (and Fcreate over an existing file, Ddelete) on a
 * file that is still open. EmuTOS ixdel (bdos/fsopnclo.c:652-669): open in ANOTHER process -> EACCDN; open by the
 * caller -> ixclose, E_OK, the handle stays valid, a later Fclose = EINTRN (-65). NewTOS deviation (user decision
 * 2026-10-18, docs/COMPATIBILITY.md): after the delete nothing through that handle reaches the card -- Fwrite = EACCDN
 * (-36), Fread = 0, Fseek/Fdatime from the OFD. Spec: docs/api/gemdos/file/41-fdelete.md edge case 3, "PROC04 point 5".
 *
 *   -DKID  -> FKID.NCP (C:\FKID.NCP), the child, mode from the command tail:
 *             "F": Fdelete + Fcreate of C:\FDA.TXT, which the PARENT holds open -> both EACCDN; exit 0x40 + bits.
 *             "T": mintlib tmpfile()'s two paths replayed with stdio (tmpfile() itself does not link in the SDK):
 *                  T1 = __mint < 9 (NewTOS): fopen w+b, write, rewind, read back, fclose, remove -> all ok;
 *                  T2 = __mint >= 9: remove() of the open stream -> 0, then fwrite+fflush -> EOF (Fwrite EACCDN).
 *                  exit 0x50 when both behave so.
 *   default -> P4F.NCP (C:\AUTO\P4F.NCP, EL0 console program before the AES): the parent.
 *     A  foreign: Fcreate C:\FDA.TXT + Fwrite "parent", Pexec(0, FKID "F"), then the parent's handle still writes and
 *        closes with E_OK; content "parent!".
 *     B  own: Fcreate C:\FDB.TXT, Fwrite 5000, Fdelete -> 0, Fopen -> EFILNF, Fseek SEEK_CUR 5000, Fwrite -> EACCDN,
 *        Fread -> 0, Fseek 0, Fdatime get/set -> 0, Fclose -> EINTRN, again -> EIHNDL; Dfree free clusters back to
 *        the value before the Fcreate; then C:\FDC.TXT (5000 x 'C') created, written, closed, read back intact.
 *     C  Fcreate over the own open C:\FDD.TXT: new handle, old Fwrite EACCDN, new Fwrite/Fclose ok, old Fclose EINTRN,
 *        content "second".
 *     D  Ddelete: C:\FDE\F.TXT deleted while open, C:\FDE\G created in its slot, stale Fwrite/Fclose, both Ddelete 0.
 *     E  Pexec(0, FKID "T") -> 0x50; C:\TMP entry count (mintlib \\tmp) and Dfree unchanged afterwards.
 * Every check prints "[FD] ok|FAIL ..."; ends with "FD RESULT fails=<n>". The card is fsck'ed after the run.
 */
#include "common.h"
#include <stdio.h>
#include <string.h>

#ifdef KID

int main(int argc, char **argv)
{
	BASEPAGE *bp = _base;
	char mode = bp->p_cmdlin[0] ? bp->p_cmdlin[1] : '?';
	(void)argc; (void)argv;

	if (mode == 'F') {
		long r1 = Fdelete("C:\\FDA.TXT");
		long r2 = Fcreate("C:\\FDA.TXT", 0);
		Cconws("[FD] kid F: Fdelete="); puthex((unsigned long)r1);
		Cconws(" Fcreate="); puthex((unsigned long)r2); Cconws("\r\n");
		return 0x40 | (r1 != -36L ? 1 : 0) | (r2 != -36L ? 2 : 0);
	}
	if (mode == 'T') {
		/* mintlib tmpfile() itself does not link with the NewTOS SDK (tmpnam -> __diraccess -> getgroups ->
		 * getgrent/getpwuid, undefined), so its two paths (stdio/tmpfile.c:41-57) are replayed with stdio:
		 *  1. __mint < 9 (NewTOS, no MiNT cookie): fopen "w+b", use, fclose + remove at the end;
		 *  2. __mint >= 9: remove() right after the fopen, keep using the stream. */
		static const char data[] = "tmpfile data 0123456789";
		char back[64];
		size_t n;
		int r, fl, cl, bad = 0;
		FILE *fp = fopen("C:\\TMP\\PT1.TMP", "w+b");
		if (!fp) { Cconws("[FD] kid T: fopen 1 NULL\r\n"); return 0x51; }
		fwrite(data, 1, sizeof(data), fp);
		rewind(fp);
		memset(back, 0, sizeof(back));
		n = fread(back, 1, sizeof(data), fp);
		cl = fclose(fp);
		r = remove("C:\\TMP\\PT1.TMP");
		Cconws("[FD] kid T1 (TOS path): read back "); puthex((unsigned long)n);
		Cconws(" fclose="); puthex((unsigned long)cl); Cconws(" remove="); puthex((unsigned long)r); Cconws("\r\n");
		if (n != sizeof(data) || memcmp(back, data, sizeof(data)) != 0 || cl != 0 || r != 0) bad |= 1;
		fp = fopen("C:\\TMP\\PT2.TMP", "w+b");
		if (!fp) { Cconws("[FD] kid T: fopen 2 NULL\r\n"); return 0x52; }
		r = remove("C:\\TMP\\PT2.TMP");
		n = fwrite(data, 1, sizeof(data), fp);
		fl = fflush(fp);
		cl = fclose(fp);
		Cconws("[FD] kid T2 (MiNT path): remove="); puthex((unsigned long)r);
		Cconws(" fwrite="); puthex((unsigned long)n); Cconws(" fflush="); puthex((unsigned long)fl);
		Cconws(" fclose="); puthex((unsigned long)cl); Cconws("\r\n");
		if (r != 0 || fl != EOF) bad |= 2;
		return 0x50 | bad;
	}
	return 0x7f;
}

#else

static void say(const char *s) { Cconws("[FD] "); Cconws(s); }

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

static char big[5000];
static char rd[5100];

/* Read `name` completely into rd; returns the length or the error. */
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

static long root_count(void)
{
	_DTA dta;
	long n = 0, r;
	Fsetdta(&dta);
	for (r = Fsfirst("C:\\TMP\\*.*", 0x37); r == 0; r = Fsnext())
		n++;
	return n;
}

int main(void)
{
	long h, h2, rc, before, n;
	unsigned short dt[2];
	int i;

	Cconws("[FD] start: Fdelete on open files (TASK/PROC04 point 5)\r\n");

	/* A: foreign process */
	h = Fcreate("C:\\FDA.TXT", 0);
	ckx("A Fcreate C:\\FDA.TXT >= 6", h >= 6, 1);
	ckx("A Fwrite parent", Fwrite((short)h, 6, "parent"), 6);
	rc = Pexec(0, "C:\\FKID.NCP", "\001F", 0L);
	ckx("A kid: Fdelete + Fcreate of the parent's open file both EACCDN (exit 0x40)", rc, 0x40);
	ckx("A parent's handle still writes", Fwrite((short)h, 1, "!"), 1);
	ckx("A parent's Fclose (still listed: first close)", Fclose((short)h), 0);
	n = slurp("C:\\FDA.TXT");
	ck("A content \"parent!\"", n == 7 && memcmp(rd, "parent!", 7) == 0);
	ckx("A Fdelete after the close", Fdelete("C:\\FDA.TXT"), 0);

	/* B: own open file */
	before = free_clusters();
	for (i = 0; i < (int)sizeof(big); i++) big[i] = (char)('a' + i % 26);
	h = Fcreate("C:\\FDB.TXT", 0);
	ckx("B Fcreate C:\\FDB.TXT >= 6", h >= 6, 1);
	ckx("B Fwrite 5000", Fwrite((short)h, sizeof(big), big), 5000);
	ckx("B Fdelete own open file", Fdelete("C:\\FDB.TXT"), 0);
	ckx("B Fopen deleted name -> EFILNF", Fopen("C:\\FDB.TXT", 0), -33);
	ckx("B Fseek(0,h,1) = position 5000", Fseek(0, (short)h, 1), 5000);
	ckx("B Fseek(0,h,2) = length 5000", Fseek(0, (short)h, 2), 5000);
	ckx("B Fseek(5001,h,0) -> ERANGE", Fseek(5001, (short)h, 0), -64);
	ckx("B Fwrite after delete -> EACCDN", Fwrite((short)h, sizeof(big), big), -36);
	ckx("B Fseek(0,h,0)", Fseek(0, (short)h, 0), 0);
	ckx("B Fread after delete -> 0", Fread((short)h, 100, rd), 0);
	ckx("B Fdatime get", Fdatime((_DOSTIME *)dt, (short)h, 0), 0);
	ckx("B Fdatime set", Fdatime((_DOSTIME *)dt, (short)h, 1), 0);
	ckx("B Fclose -> EINTRN", Fclose((short)h), -65);
	ckx("B Fclose again -> EIHNDL", Fclose((short)h), -37);
	ckx("B Dfree free clusters back", free_clusters(), before);
	for (i = 0; i < (int)sizeof(big); i++) big[i] = 'C';
	h = Fcreate("C:\\FDC.TXT", 0);
	ckx("B Fcreate C:\\FDC.TXT >= 6", h >= 6, 1);
	ckx("B FDC Fwrite 5000", Fwrite((short)h, sizeof(big), big), 5000);
	ckx("B FDC Fclose", Fclose((short)h), 0);
	n = slurp("C:\\FDC.TXT");
	ck("B FDC.TXT reads back 5000 x 'C'", n == 5000 && memcmp(rd, big, 5000) == 0);

	/* C: Fcreate over the own open file */
	h = Fcreate("C:\\FDD.TXT", 0);
	ckx("C Fwrite first", Fwrite((short)h, 5, "first"), 5);
	h2 = Fcreate("C:\\FDD.TXT", 0);
	ckx("C Fcreate over own open file >= 6 and new", h2 >= 6 && h2 != h, 1);
	ckx("C old handle Fwrite -> EACCDN", Fwrite((short)h, 2, "zz"), -36);
	ckx("C new handle Fwrite", Fwrite((short)h2, 6, "second"), 6);
	ckx("C new handle Fclose", Fclose((short)h2), 0);
	ckx("C old handle Fclose -> EINTRN", Fclose((short)h), -65);
	n = slurp("C:\\FDD.TXT");
	ck("C content \"second\"", n == 6 && memcmp(rd, "second", 6) == 0);

	/* D: Ddelete of a folder in the slot of a deleted open file */
	ckx("D Dcreate C:\\FDE", Dcreate("C:\\FDE"), 0);
	h = Fcreate("C:\\FDE\\F.TXT", 0);
	ckx("D Fwrite", Fwrite((short)h, 3, "abc"), 3);
	ckx("D Fdelete own open file", Fdelete("C:\\FDE\\F.TXT"), 0);
	ckx("D Dcreate C:\\FDE\\G", Dcreate("C:\\FDE\\G"), 0);
	ckx("D stale Fwrite -> EACCDN", Fwrite((short)h, 3, "def"), -36);
	ckx("D stale Fclose -> EINTRN", Fclose((short)h), -65);
	ckx("D Ddelete C:\\FDE\\G", Ddelete("C:\\FDE\\G"), 0);
	ckx("D Ddelete C:\\FDE", Ddelete("C:\\FDE"), 0);

	/* E: mintlib tmpfile() in a child */
	before = free_clusters();
	n = root_count();
	rc = Pexec(0, "C:\\FKID.NCP", "\001T", 0L);
	ckx("E kid tmpfile() paths T1 ok, T2 remove 0 + fflush EOF (exit 0x50)", rc, 0x50);
	ckx("E C:\\TMP entries unchanged after the kid", root_count(), n);
	ckx("E Dfree unchanged after the kid", free_clusters(), before);

	Cconws("FD RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
	return 0;
}

#endif
