/*
 * statt.c -- TASK/SDK01 F runtime probe: stat()/lstat()/fstat() timestamps
 * on NewTOS (QEMU raspi4b), C:\AUTO\STATT.NCP (console, before the AES).
 *
 * Every expected value is computed in this file from what GEMDOS itself
 * reports (Fsfirst DTA, Fdatime, Tgetdate/Tgettime) with an own
 * days-from-civil (no mintlib time code), or is a constant from Python
 * calendar.timegm; TZ is not set on the card, so mintlib's mktime is UTC.
 *
 *   D  C:\DATED.TXT (card file, FAT stamp 2026-10-17 12:00:00): stat and
 *      lstat mtime = atime = ctime = 1792238400 = epoch(DTA fields).
 *   N  C:\TMP\NEW.TXT created now (fopen/fputs/fclose): stat mtime =
 *      epoch(DTA) and |mtime - time()| <= 4; time() = epoch(Tgetdate/
 *      Tgettime) +- 2; fstat(fileno) of the reopened file = stat mtime and
 *      Fdatime(get) raw = DTA raw.
 *   S  Fdatime(set) 2001-02-03 04:05:06 on an Fopen handle: fstat(handle)
 *      = 981173106; after Fclose stat = 981173106.
 *   F  C:\SUB (folder): stat mtime = epoch(Fsfirst("C:\SUB", FA_DIR) DTA),
 *      S_ISDIR; C:\SUB\ = the "." entry's stamp (Fsfirst "C:\SUB\*.*", 0xff,
 *      the search __do_stat makes; its name and attribute byte are printed).
 *   R  C:\ (root): OLDDATE = __unixtime(0,0) = 1979-11-30 = 312768000.
 *   V  /dev/con (device name) and fstat(1) (console): time() - 2 (+-1), S_IFCHR.
 *   E  C:\NOPE.TXT: -1, ENOENT.
 * Every check prints "[ST] ok|FAIL ..."; ends with "ST RESULT fails=<n>".
 */
#include "common.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>
#include <unistd.h>
#include <sys/stat.h>

static void putdec(long v)
{
	char b[24];
	int i = 23;
	int neg = v < 0;
	unsigned long u = neg ? -(unsigned long)v : (unsigned long)v;
	b[i] = 0;
	do { b[--i] = '0' + (u % 10); u /= 10; } while (u);
	if (neg) b[--i] = '-';
	Cconws(b + i);
}

static void ck(const char *what, int ok, long got, long want)
{
	Cconws(ok ? "[ST] ok   " : "[ST] FAIL ");
	Cconws(what);
	Cconws(" got=");
	putdec(got);
	Cconws(" want=");
	putdec(want);
	Cconws("\r\n");
	if (!ok)
		fails++;
}

static void note(const char *a, const char *b)
{
	Cconws("[ST] "); Cconws(a); Cconws(b); Cconws("\r\n");
}

/* days since 1970-01-01 of a civil date (H. Hinnant's days_from_civil) */
static long days_from_civil(long y, long m, long d)
{
	long era, yoe, doy, doe;
	y -= m <= 2;
	era = (y >= 0 ? y : y - 399) / 400;
	yoe = y - era * 400;
	doy = (153 * (m + (m > 2 ? -3 : 9)) + 2) / 5 + d - 1;
	doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	return era * 146097 + doe - 719468;
}

/* GEMDOS packed time/date -> seconds since the epoch, read as UTC */
static long dos_epoch(unsigned tm, unsigned dt)
{
	long y = 1980 + ((dt >> 9) & 127), mo = (dt >> 5) & 15, d = dt & 31;
	long h = (tm >> 11) & 31, mi = (tm >> 5) & 63, s = (tm & 31) * 2;
	return days_from_civil(y, mo, d) * 86400L + h * 3600 + mi * 60 + s;
}

static void show_dos(const char *label, unsigned tm, unsigned dt)
{
	Cconws("[ST] "); Cconws(label); Cconws(" DOS ");
	putdec(1980 + ((dt >> 9) & 127)); Cconws("-"); putdec((dt >> 5) & 15); Cconws("-"); putdec(dt & 31);
	Cconws(" "); putdec((tm >> 11) & 31); Cconws(":"); putdec((tm >> 5) & 63); Cconws(":"); putdec((tm & 31) * 2);
	Cconws(" = "); putdec(dos_epoch(tm, dt)); Cconws("\r\n");
}

static int dta_of(const char *spec, int attr, _DTA *out)
{
	_DTA *old = Fgetdta();
	long r;
	Fsetdta(out);
	r = Fsfirst(spec, attr);
	Fsetdta(old);
	return r == 0;
}

static void three(const char *what, struct stat *st, long want)
{
	char w[96];
	strcpy(w, what); strcat(w, " st_mtime");
	ck(w, st->st_mtime == want, st->st_mtime, want);
	strcpy(w, what); strcat(w, " st_atime");
	ck(w, st->st_atime == want, st->st_atime, want);
	strcpy(w, what); strcat(w, " st_ctime");
	ck(w, st->st_ctime == want, st->st_ctime, want);
}

int main(void)
{
	struct stat st, st2;
	_DTA d;
	_DOSTIME dt;
	FILE *fp;
	long now, ge, h, r, sm;
	unsigned short tt, td;
	char *tz = getenv("TZ");

	note("STATT probe TASK/SDK01 F; TZ=", tz ? tz : "(unset)");
	tt = Tgettime(); td = Tgetdate();
	now = time(NULL);
	show_dos("clock Tgetdate/Tgettime", tt, td);
	ge = dos_epoch(tt, td);
	ck("V time() = epoch(Tgetdate/Tgettime) +-2", now >= ge - 2 && now <= ge + 2, now, ge);

	/* D: card file with a known stamp */
	if (dta_of("C:\\DATED.TXT", 0x07, &d)) {
		show_dos("C:\\DATED.TXT DTA", d.dta_time, d.dta_date);
		ck("D DTA epoch is 2026-10-17 12:00:00", dos_epoch(d.dta_time, d.dta_date) == 1792238400L,
		   dos_epoch(d.dta_time, d.dta_date), 1792238400L);
	} else
		ck("D Fsfirst C:\\DATED.TXT", 0, 0, 0);
	r = stat("C:\\DATED.TXT", &st);
	ck("D stat C:\\DATED.TXT rc", r == 0, r, 0);
	three("D stat C:\\DATED.TXT", &st, 1792238400L);
	note("D ctime(st_mtime) = ", ctime(&st.st_mtime));
	r = lstat("C:\\DATED.TXT", &st);
	three("D lstat C:\\DATED.TXT", &st, 1792238400L);

	/* N: a file created now */
	fp = fopen("C:\\TMP\\NEW.TXT", "w");
	ck("N fopen w C:\\TMP\\NEW.TXT", fp != NULL, fp != NULL, 1);
	if (fp) { fputs("SDK01 F\r\n", fp); fclose(fp); }
	now = time(NULL);
	r = stat("C:\\TMP\\NEW.TXT", &st);
	ck("N stat rc", r == 0, r, 0);
	if (dta_of("C:\\TMP\\NEW.TXT", 0x07, &d)) {
		show_dos("C:\\TMP\\NEW.TXT DTA", d.dta_time, d.dta_date);
		three("N stat C:\\TMP\\NEW.TXT = epoch(DTA)", &st, dos_epoch(d.dta_time, d.dta_date));
	} else
		ck("N Fsfirst NEW.TXT", 0, 0, 0);
	ck("N |st_mtime - time()| <= 4", st.st_mtime >= now - 4 && st.st_mtime <= now + 4, st.st_mtime, now);
	note("N ctime(st_mtime) = ", ctime(&st.st_mtime));
	sm = st.st_mtime;
	fp = fopen("C:\\TMP\\NEW.TXT", "r");
	if (fp) {
		r = fstat(fileno(fp), &st2);
		ck("N fstat(fileno) rc", r == 0, r, 0);
		three("N fstat(reopened) = stat", &st2, sm);
		ck("N fstat S_ISREG", S_ISREG(st2.st_mode), st2.st_mode, S_IFREG);
		r = Fdatime(&dt, fileno(fp), 0);
		ck("N Fdatime(get) rc", r == 0, r, 0);
		ck("N Fdatime time word = DTA", dt.time == d.dta_time, dt.time, d.dta_time);
		ck("N Fdatime date word = DTA", dt.date == d.dta_date, dt.date, d.dta_date);
		fclose(fp);
	} else
		ck("N fopen r", 0, 0, 1);

	/* S: a stamp set with Fdatime */
	h = Fopen("C:\\TMP\\NEW.TXT", 2);
	ck("S Fopen rw handle >= 6", h >= 6, h, 6);
	if (h >= 0) {
		dt.time = 0x20a3; dt.date = 0x2a43;	/* 2001-02-03 04:05:06 */
		r = Fdatime(&dt, (short) h, 1);
		ck("S Fdatime(set) rc", r == 0, r, 0);
		r = fstat((int) h, &st2);
		ck("S fstat(handle) rc", r == 0, r, 0);
		three("S fstat(handle) after Fdatime set", &st2, 981173106L);
		Fclose((short) h);
	}
	r = stat("C:\\TMP\\NEW.TXT", &st);
	three("S stat after Fclose", &st, 981173106L);

	/* F: folders */
	r = stat("C:\\SUB", &st);
	ck("F stat C:\\SUB rc", r == 0, r, 0);
	ck("F stat C:\\SUB S_ISDIR", S_ISDIR(st.st_mode), st.st_mode, S_IFDIR);
	if (dta_of("C:\\SUB", 0x10, &d)) {
		show_dos("C:\\SUB DTA", d.dta_time, d.dta_date);
		three("F stat C:\\SUB = epoch(DTA)", &st, dos_epoch(d.dta_time, d.dta_date));
	} else
		ck("F Fsfirst C:\\SUB", 0, 0, 0);
	r = stat("C:\\SUB\\", &st);
	ck("F stat C:\\SUB\\ rc", r == 0, r, 0);
	ck("F stat C:\\SUB\\ S_ISDIR", S_ISDIR(st.st_mode), st.st_mode, S_IFDIR);
	/* the search __do_stat makes (mask 0xff, mintlib/do_stat.c:259) */
	if (dta_of("C:\\SUB\\*.*", 0xff, &d)) {
		note("F Fsfirst C:\\SUB\\*.* first name = ", d.dta_name);
		ck("F   its attribute byte", 1, (unsigned char) d.dta_attribute, (unsigned char) d.dta_attribute);
	}
	if (dta_of("C:\\SUB\\*.*", 0xff, &d) && d.dta_name[0] == '.' && d.dta_name[1] == 0) {
		show_dos("C:\\SUB\\. DTA", d.dta_time, d.dta_date);
		three("F stat C:\\SUB\\ = epoch(\".\" DTA)", &st, dos_epoch(d.dta_time, d.dta_date));
	} else
		ck("F Fsfirst C:\\SUB\\*.* first entry is \".\"", 0, 0, 0);

	/* R: root */
	r = stat("C:\\", &st);
	ck("R stat C:\\ rc", r == 0, r, 0);
	ck("R stat C:\\ S_ISDIR", S_ISDIR(st.st_mode), st.st_mode, S_IFDIR);
	three("R stat C:\\ = OLDDATE 1979-11-30", &st, 312768000L);

	/* V: devices */
	r = stat("/dev/con", &st);	/* _unx2dos: device name -> 1 (unx2dos.c:30-62) */
	now = time(NULL);
	ck("V stat /dev/con rc", r == 0, r, 0);
	ck("V stat /dev/con S_ISCHR", S_ISCHR(st.st_mode), st.st_mode, S_IFCHR);
	ck("V stat /dev/con mtime = time()-2 +-1", st.st_mtime >= now - 3 && st.st_mtime <= now - 1, st.st_mtime, now - 2);
	r = fstat(1, &st);
	now = time(NULL);
	ck("V fstat(1) rc", r == 0, r, 0);
	ck("V fstat(1) S_ISCHR", S_ISCHR(st.st_mode), st.st_mode, S_IFCHR);
	ck("V fstat(1) mtime = time()-2 +-1", st.st_mtime >= now - 3 && st.st_mtime <= now - 1, st.st_mtime, now - 2);

	/* E: missing */
	errno = 0;
	r = stat("C:\\NOPE.TXT", &st);
	ck("E stat C:\\NOPE.TXT = -1", r == -1, r, -1);
	ck("E errno ENOENT", errno == ENOENT, errno, ENOENT);

	Cconws("ST RESULT fails=");
	putdec(fails);
	Cconws("\r\n");
	return 0;
}
