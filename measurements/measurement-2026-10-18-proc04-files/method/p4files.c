/*
 * p4files.c -- TASK/PROC04 point 4 probe WITH expectations (EmuTOS): the
 * open-file table has OPNFILES = 75 entries, handles 6..80, system-wide;
 * a full table answers ENHNDL (-35) from opnfil (Fopen/Fcreate,
 * bdos/fsopnclo.c:341-346) and xdup (Fdup, bdos/fshand.c:100-105).
 * Spec: docs/api/gemdos/_state.md section 2, file/3e-fclose.md.
 *
 * Card: C:\AUTO\P4F.NCP (EL0 console program, runs before the AES).
 *
 *   1. Fcreate C:\PF00.TXT .. C:\PF69.TXT, all kept open (70 handles), each
 *      handle in 6..80 and distinct; write "PFnn" into each.
 *   2. Fopen C:\PF00.TXT until ENHNDL: 70 + extra must be 75 (nothing else is
 *      open at AUTO time), the last handle is 80; Fopen, Fcreate and Fdup(1)
 *      on the full table answer ENHNDL.
 *   3. Fclose every handle (E_OK); Fclose(81) is EIHNDL (>= NUMHANDLES).
 *   4. Reopen each PFnn.TXT, read back "PFnn", close, Fdelete (also the
 *      PFXX.TXT the failed Fcreate left behind: ixcreat wrote its entry
 *      before opnfil, bdos/fsopnclo.c:186-201).
 *
 * Every check prints "[PF] ok|FAIL ..."; ends with "PF RESULT fails=<n>".
 */
#include "common.h"

#define NFILES  70
#define OPNFILES 75
#define NUMSTD  6
#define ENHNDL  (-35L)
#define EIHNDL  (-37L)

static void say(const char *s) { Cconws("[PF] "); Cconws(s); }

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

static void name(char *b, int i)
{
	/* C:\PFnn.TXT */
	const char *t = "C:\\PF00.TXT";
	int k;
	for (k = 0; t[k]; k++) b[k] = t[k];
	b[k] = 0;
	b[5] = '0' + i / 10;
	b[6] = '0' + i % 10;
}

int main(void)
{
	static long h[OPNFILES + 1];
	char nm[16], buf[8];
	int i, n, extra, bad;
	long r, last;

	Cconws("[PF] start: open-file table probe (OPNFILES 75)\r\n");

	/* 1. 70 files created and kept open */
	bad = 0;
	for (i = 0; i < NFILES; i++) {
		name(nm, i);
		h[i] = Fcreate(nm, 0);
		if (h[i] < NUMSTD || h[i] >= NUMSTD + OPNFILES) { bad++; continue; }
		buf[0] = 'P'; buf[1] = 'F'; buf[2] = nm[5]; buf[3] = nm[6];
		if (Fwrite((short)h[i], 4, buf) != 4) bad++;
	}
	ckx("70 x Fcreate+Fwrite, bad", bad, 0);
	bad = 0;
	for (i = 0; i < NFILES; i++)
		for (n = 0; n < i; n++)
			if (h[i] == h[n]) bad++;
	ckx("70 handles distinct, dups", bad, 0);
	ckx("handle #70", h[NFILES - 1], NUMSTD + NFILES - 1);

	/* 2. fill the rest of the table */
	extra = 0; last = h[NFILES - 1];
	for (;;) {
		r = Fopen("C:\\PF00.TXT", 0);
		if (r < 0) break;
		h[NFILES + extra] = r; last = r; extra++;
		if (NFILES + extra > OPNFILES) break; /* table larger than 75 */
	}
	ckx("Fopen until full: result", r, ENHNDL);
	ckx("open handles when full", NFILES + extra, OPNFILES);
	ckx("last handle", last, NUMSTD + OPNFILES - 1);
	ckx("Fopen on full table", Fopen("C:\\PF00.TXT", 0), ENHNDL);
	ckx("Fcreate on full table", Fcreate("C:\\PFXX.TXT", 0), ENHNDL);
	ckx("Fdup(1) on full table", Fdup(1), ENHNDL);

	/* 3. close all */
	bad = 0;
	for (i = 0; i < NFILES + extra; i++)
		if (Fclose((short)h[i]) != 0) bad++;
	ckx("Fclose all, bad", bad, 0);
	ckx("Fclose(81) (>= NUMHANDLES)", Fclose(NUMSTD + OPNFILES), EIHNDL);

	/* 4. read back + delete */
	bad = 0;
	for (i = 0; i < NFILES; i++) {
		long fh;
		name(nm, i);
		fh = Fopen(nm, 0);
		if (fh < NUMSTD) { bad++; continue; }
		buf[0] = buf[1] = buf[2] = buf[3] = 0;
		if (Fread((short)fh, 4, buf) != 4 || buf[0] != 'P' || buf[1] != 'F' ||
		    buf[2] != nm[5] || buf[3] != nm[6])
			bad++;
		if (Fclose((short)fh) != 0) bad++;
		if (Fdelete(nm) != 0) bad++;
	}
	ckx("70 x reopen+verify+delete, bad", bad, 0);
	ckx("Fdelete PFXX.TXT (created before ENHNDL)", Fdelete("C:\\PFXX.TXT"), 0);
	ck("done", 1);

	Cconws("PF RESULT fails=");
	puthex((unsigned long)fails);
	Cconws("\r\n");
	return 0;
}
