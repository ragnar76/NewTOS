/*
 * stat-host.c -- host harness for the timestamps of stat()/lstat()
 * (TASK/SDK01 F, docs/analysis/sdk-inventory-2026-10-20.md W4).
 *
 * newtos-arch/do_stat.c's __do_stat is the TOS branch of mintlib's own
 * __do_stat (mintlib/do_stat.c:108-339): after the Fstat64 probe answers
 * -ENOSYS it searches the name with Fsfirst and sets st_mtime = st_atime =
 * st_ctime = __unixtime(dta_time, dta_date); a drive root (or a "."/".."
 * search that lands in a root) gets OLDDATE = __unixtime(0, 0); a device
 * name gets time() - 2. Before SDK01 F __do_stat was __quickstat, which
 * leaves all three at 0 (counter-check: STAT_SRC=<HEAD do_stat.c>).
 *
 * stat-host.sh links do_stat.c (or STAT_SRC), mintlib's own unixtime.c and
 * ctype.c (minus weak_alias lines), stat-host-fs.c (Fsfirst table, Fstat64
 * = EINVFN, time(), mktime = UTC) and this file. The expected seconds are
 * Python calendar.timegm values, independent of the code under test.
 *
 * Checked:
 *   file         C:\T\A.TXT 2026-10-21 14:37:58 -> 1792593478, S_IFREG,
 *                size 1234, via one Fstat64 probe and Fsfirst of the name;
 *                the same through lstat and through "c:/t/a.txt";
 *   read-only    C:\T\RO.NCP 2024-02-29 12:00:02 -> 1709208002, mode 0555;
 *   2037         C:\T\LATE.DAT 2037-12-31 23:59:58 -> 2145916798 (the last
 *                stamp mintlib's 32-bit time_t holds);
 *   folder       C:\T\SUB 2025-03-01 -> 1740787200, S_IFDIR;
 *   dir\ and .   C:\T\SUB\ and C:\T\SUB\. -> Fsfirst "<dir>\*.*", the "."
 *                entry's stamp 1980-01-01 -> 315532800, S_IFDIR;
 *   root         C:\, C:, \ -> OLDDATE = __unixtime(0, 0) = 1979-11-30
 *                00:00:00 -> 312768000, S_IFDIR, no Fsfirst;
 *   ..           C:\.. -> Fsfirst "C:\*.*", first hit not "." -> root ->
 *                OLDDATE;
 *   device       CON: -> time() - 2, S_IFCHR, no Fsfirst;
 *   missing      C:\T\NONE.TXT -> -1, ENOENT.
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdio.h>
#include <string.h>
#include <time.h>

int st_probe (const char *, int, long[3], unsigned *, long *, int *, long *,
	      long *, char *, size_t);
long st_now (void);
int st_enoent (void);
unsigned st_s_ifmt (void);
unsigned st_s_ifreg (void);
unsigned st_s_ifdir (void);
unsigned st_s_ifchr (void);

/* mintlib's mktime with TZ unset = UTC (stat-host-fs.c mktime). */
long
st_host_timegm (int year, int mon, int mday, int hour, int min, int sec)
{
  struct tm tm;
  memset (&tm, 0, sizeof tm);
  tm.tm_year = year;
  tm.tm_mon = mon;
  tm.tm_mday = mday;
  tm.tm_hour = hour;
  tm.tm_min = min;
  tm.tm_sec = sec;
  return (long) timegm (&tm);
}

static int failed, checks;

static void
check (int ok, const char *what)
{
  checks++;
  if (!ok)
    failed++;
  printf ("%s  %s\n", ok ? "PASS" : "FAIL", what);
}

/* path must stat (lflag) with r == 0, all three times == want, file type
 * fmt, and Fsfirst asked `spec' ("" = not asked). */
static void
stat_case (const char *path, int lflag, long want, unsigned fmt,
	   const char *spec, long want_time_calls)
{
  long t[3], size, nf, nt;
  unsigned mode;
  int r, err;
  char got[256], w[512];

  r = st_probe (path, lflag, t, &mode, &size, &err, &nf, &nt, got, sizeof got);
  snprintf (w, sizeof w,
	    "%s %-14s mtime=atime=ctime=%ld type 0%o Fsfirst \"%s\" (got r %d, %ld %ld %ld, mode 0%o, Fsfirst \"%s\", Fstat64 %ld, time() %ld)",
	    lflag ? "lstat" : "stat ", path, want, fmt, spec, r, t[0], t[1], t[2],
	    mode, got, nf, nt);
  check (r == 0 && t[0] == want && t[1] == want && t[2] == want
	 && (mode & st_s_ifmt ()) == fmt && strcmp (got, spec) == 0
	 && nf == 1 && nt == want_time_calls, w);
}

int
main (void)
{
  long t[3], size, nf, nt;
  unsigned mode;
  int r, err;
  char got[256], w[512];

  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("stat-host: skipped -- host is not LP64\n");
      return 77;
    }

  stat_case ("C:\\T\\A.TXT", 0, 1792593478L, st_s_ifreg (), "C:\\T\\A.TXT", 0);
  stat_case ("C:\\T\\A.TXT", 1, 1792593478L, st_s_ifreg (), "C:\\T\\A.TXT", 0);
  stat_case ("c:/t/a.txt", 0, 1792593478L, st_s_ifreg (), "c:\\t\\a.txt", 0);
  r = st_probe ("C:\\T\\A.TXT", 0, t, &mode, &size, &err, &nf, &nt, got, sizeof got);
  snprintf (w, sizeof w, "stat  C:\\T\\A.TXT size 1234, mode 0644 (got %ld, 0%o)", size, mode & 0777);
  check (r == 0 && size == 1234 && (mode & 0777) == 0644, w);

  stat_case ("C:\\T\\RO.NCP", 0, 1709208002L, st_s_ifreg (), "C:\\T\\RO.NCP", 0);
  r = st_probe ("C:\\T\\RO.NCP", 0, t, &mode, &size, &err, &nf, &nt, got, sizeof got);
  snprintf (w, sizeof w, "stat  C:\\T\\RO.NCP read-only executable 0555 (got 0%o)", mode & 0777);
  check (r == 0 && (mode & 0777) == 0555, w);

  stat_case ("C:\\T\\LATE.DAT", 0, 2145916798L, st_s_ifreg (), "C:\\T\\LATE.DAT", 0);

  stat_case ("C:\\T\\SUB", 0, 1740787200L, st_s_ifdir (), "C:\\T\\SUB", 0);
  stat_case ("C:\\T\\SUB\\", 0, 315532800L, st_s_ifdir (), "C:\\T\\SUB\\*.*", 0);
  stat_case ("C:\\T\\SUB\\.", 0, 315532800L, st_s_ifdir (), "C:\\T\\SUB\\.\\*.*", 0);

  stat_case ("C:\\", 0, 312768000L, st_s_ifdir (), "", 0);
  stat_case ("C:", 0, 312768000L, st_s_ifdir (), "", 0);
  stat_case ("\\", 0, 312768000L, st_s_ifdir (), "", 0);
  stat_case ("C:\\..", 0, 312768000L, st_s_ifdir (), "C:\\*.*", 0);

  stat_case ("CON:", 0, st_now () - 2, st_s_ifchr (), "", 1);

  r = st_probe ("C:\\T\\NONE.TXT", 0, t, &mode, &size, &err, &nf, &nt, got, sizeof got);
  snprintf (w, sizeof w, "stat  C:\\T\\NONE.TXT missing -> -1, ENOENT (got %d, errno %d)", r, err);
  check (r == -1 && err == st_enoent (), w);

  printf ("stat-host: %d checks, %d failed\n", checks, failed);
  return failed;
}
