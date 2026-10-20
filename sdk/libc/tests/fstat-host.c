/*
 * fstat-host.c -- host harness for the stdio buffer size of a regular file
 * (TASK/FS08 part 4, user decision 2026-10-07: "a larger buffer is no
 * problem, we have more memory than a real Atari").
 *
 * mintlib's stdio takes a stream's buffer size from st_blksize
 * (stdio/stdio_init.c:72-77), which the lean sdk/libc/newtos-arch/do_fstat.c
 * fills. Before FS08 part 4 it was BUFSIZ (1024) for every handle; then a
 * GEMDOS file handle (>= 6) got NEWTOS_FILE_BLKSIZE; since TASK/LIBC-FSTAT
 * every handle >= 0 that is not a terminal and whose Fseek(END) succeeds gets
 * it, and a standard handle 0..5 is S_IFCHR exactly when __isatty says so.
 * fstat-host.sh links the shim, mintlib's own
 * mintlib/isatty.c (TASK/LIBC-TTY: the NewTOS isatty shim is gone) and
 * stdio/stdio_init.c, fstat-host-fs.c (Fseek/Fcntl model + probes) and this
 * file.
 *
 * Checked:
 *   regular file  handles 6, 7 (empty), 8: S_IFREG, honest st_size,
 *                 st_blksize 32768, the handle's position unchanged by the
 *                 fstat; __stdio_init_stream picks a 32768-byte buffer, not
 *                 line-buffered, seekable;
 *   console       handles 0, 1, 2: S_IFCHR, st_blksize BUFSIZ (1024);
 *                 the stream is line-buffered, BUFSIZ, its seek removed
 *                 (as before);
 *   devices 3-5   standard handles at their device (AUX/PRN): S_IFCHR,
 *                 BUFSIZ, line-buffered, no seek (TASK/LIBC-FSTAT; before:
 *                 S_IFREG, size 0);
 *   redirected    standard handles 0, 1, 2, 3 Fforce'd to a file (fh_redirect)
 *                 and the isatty cache cleared as at start-up (fh_reset_cache):
 *                 S_IFREG, honest size, 32768, position unchanged, 32768-byte
 *                 stdio buffer, not line-buffered, seekable (TASK/LIBC-FSTAT;
 *                 before: S_IFCHR, 1024 for 0-2);
 *   consistency   isatty(fd) == S_ISCHR(fstat(fd)) for 0..5 before and after
 *                 the redirection; the stdio/read()/dup2() stat of 0..5
 *                 (__do_fstat exact 0) after isatty makes no trap for a
 *                 terminal (the cached answer) -- fstat() itself (exact 1)
 *                 asks time() for a terminal's stamp since TASK/SDK01 F;
 *   stale cache   a handle whose isatty answer is cached as a terminal and
 *                 that is Fforce'd to a file afterwards in the same process
 *                 stays S_IFCHR until the cache is cleared -- the upstream
 *                 limit (mintlib/isatty.c:23-24), pinned as a finding;
 *   unchanged     handle -1 (a BIOS device that answers Fseek 0) and handle
 *                 9 (not open, Fseek EIHNDL): st_blksize BUFSIZ.
 *   isatty        (mintlib/isatty.c: Fcntl -> EINVFN, then the Fseek probe)
 *                 handles 0, 1, 2 (devices): 1, status FH_ISATTY, flags
 *                 CRMOD|ECHO -- the flags __read needs to take the Fread line
 *                 path instead of the RAW path; handles 3, 4, 5 (devices): 1;
 *                 handle 3 (Fforce'd to a file),
 *                 6 (file, mid-way), 7 (empty file), 9 (not open): 0, status
 *                 FH_ISAFILE, file position unchanged by the probe; handle -1
 *                 (BIOS device): 1 (upstream); a second query is answered from
 *                 __open_stat[] without a trap.
 *
 *   __sys_fstat   (TASK/SDK01 A: ttyname/ttyname_r/ctermid link against it)
 *                 handles 0, 6, -1, exact 0 and 1: one Ffstat64 trap with the
 *                 caller's handle and stat buffer, the kernel's EINVFN
 *                 returned unchanged (= mintlib's -ENOSYS, the "no native
 *                 stat" answer ttyname's MiNT branch expects).
 *
 *   timestamps    (TASK/SDK01 F, mintlib/do_fstat.c:102-119 TOS branch)
 *                 fstat() (exact 1) of a file handle 6, 7, 8: one Fdatime
 *                 GET (rwflag 0) on that handle, st_mtime = st_atime =
 *                 st_ctime = the DOS stamp as UTC seconds (expected values
 *                 from Python calendar.timegm, independent of the code under
 *                 test; 8 is 2037-12-31 23:59:58 = 2145916798, the last DOS
 *                 stamp mintlib's 32-bit time_t holds); a standard handle
 *                 Fforce'd to a file: its file's
 *                 stamp; a terminal 0: no Fdatime, time() - 2; a handle
 *                 Fdatime rejects (9 not open, -1 BIOS device): time() - 2;
 *                 the stdio/read()/dup2() stat (exact 0): no Fdatime, no
 *                 time(), times 0.
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdio.h>
#include <time.h>
#include <string.h>

/* mintlib's mktime with TZ unset = UTC (fstat-host-fs.c mktime). */
long
fh_host_timegm (int year, int mon, int mday, int hour, int min, int sec)
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

int fh_probe_stat (int, int *, int *, long *, long *);
int fh_probe_stat_ex (int, int, int *, int *, long *, long *);
int fh_probe_times (int, int, long[3], long *, long *, int *, int *);
long fh_now (void);
long fh_probe_stream (int, int *, int *);
void fh_set_pos (short, long);
long fh_get_pos (short);
long fh_bufsiz (void);
int fh_probe_isatty (int, int *, int *);
long fh_trap_count (void);
void fh_redirect (short, long);
void fh_reset_cache (void);
int fh_fh_isatty (void);
int fh_fh_isafile (void);
int fh_crmod_echo (void);
long fh_probe_sys_fstat (int, int, int *, int *, int *, long *);

#define WANT_FILE_BLKSIZE 32768L	/* do_fstat.c NEWTOS_FILE_BLKSIZE */

static int failed, checks;

static void
check (int ok, const char *what)
{
  checks++;
  if (!ok)
    failed++;
  printf ("%s  %s\n", ok ? "PASS" : "FAIL", what);
}

static void
file_case (int fd, long size)
{
  int reg, chr, lb, sk;
  long sz, bs, before = size / 3, b;
  char w[160];

  fh_set_pos ((short) fd, before);
  fh_probe_stat (fd, &reg, &chr, &sz, &bs);
  snprintf (w, sizeof w, "fd %d: S_IFREG, st_size %ld (got %ld)", fd, size, sz);
  check (reg && !chr && sz == size, w);
  snprintf (w, sizeof w, "fd %d: st_blksize %ld (got %ld)", fd, WANT_FILE_BLKSIZE, bs);
  check (bs == WANT_FILE_BLKSIZE, w);
  snprintf (w, sizeof w, "fd %d: position %ld restored after fstat (got %ld)", fd, before, fh_get_pos ((short) fd));
  check (fh_get_pos ((short) fd) == before, w);
  b = fh_probe_stream (fd, &lb, &sk);
  snprintf (w, sizeof w, "fd %d: stdio buffer %ld, full buffering, seekable (got %ld, linebuf %d, seek %d)",
	    fd, WANT_FILE_BLKSIZE, b, lb, sk);
  check (b == WANT_FILE_BLKSIZE && !lb && sk, w);
}

static void
small_case (int fd, int want_chr, const char *what)
{
  int reg, chr, lb, sk;
  long sz, bs, b;
  char w[200];

  fh_probe_stat (fd, &reg, &chr, &sz, &bs);
  snprintf (w, sizeof w, "fd %d (%s): st_blksize BUFSIZ %ld (got %ld)%s", fd, what, fh_bufsiz (), bs,
	    want_chr ? ", S_IFCHR" : "");
  check (bs == fh_bufsiz () && (!want_chr || (chr && !reg)), w);
  b = fh_probe_stream (fd, &lb, &sk);
  if (want_chr)
    {
      snprintf (w, sizeof w, "fd %d (%s): stdio buffer %ld, line-buffered, no seek (got %ld, linebuf %d, seek %d)",
		fd, what, fh_bufsiz (), b, lb, sk);
      check (b == fh_bufsiz () && lb && !sk, w);
    }
  else
    {
      snprintf (w, sizeof w, "fd %d (%s): stdio buffer %ld (got %ld)", fd, what, fh_bufsiz (), b);
      check (b == fh_bufsiz (), w);
    }
}

static void
isatty_case (int fd, int want_tty, long pos, const char *what)
{
  int st, fl, r;
  long t, after;
  char w[200];

  if (pos >= 0)
    fh_set_pos ((short) fd, pos);
  r = fh_probe_isatty (fd, &st, &fl);
  if (want_tty)
    {
      snprintf (w, sizeof w, "isatty(%d) (%s) = 1, status FH_ISATTY, flags CRMOD|ECHO 0x%x (got %d, status %d, flags 0x%x)",
		fd, what, fh_crmod_echo (), r, st, fl);
      check (r == 1 && st == fh_fh_isatty () && fl == fh_crmod_echo (), w);
    }
  else
    {
      snprintf (w, sizeof w, "isatty(%d) (%s) = 0, status FH_ISAFILE (got %d, status %d)", fd, what, r, st);
      check (r == 0 && st == fh_fh_isafile (), w);
    }
  if (pos >= 0)
    {
      after = fh_get_pos ((short) fd);
      snprintf (w, sizeof w, "isatty(%d) (%s): position %ld unchanged (got %ld)", fd, what, pos, after);
      check (after == pos, w);
    }
  t = fh_trap_count ();
  r = fh_probe_isatty (fd, &st, &fl);
  snprintf (w, sizeof w, "isatty(%d) (%s): second query from the cache, same answer, no trap (got %d, %ld traps)",
	    fd, what, r, fh_trap_count () - t);
  check (r == want_tty && fh_trap_count () == t, w);
}

/* isatty(fd) == S_ISCHR(fstat(fd)); for a terminal the fstat (after the
 * isatty that filled the cache) makes no GEMDOS call. */
static void
consistency_case (int fd, const char *what)
{
  int st, fl, tty, reg, chr;
  long sz, bs, t, n;
  char w[200];

  tty = fh_probe_isatty (fd, &st, &fl);
  fh_probe_stat (fd, &reg, &chr, &sz, &bs);
  snprintf (w, sizeof w, "fd %d (%s): isatty %d == S_ISCHR(fstat) %d", fd, what, tty, chr);
  check (tty == chr && reg == !chr, w);
  t = fh_trap_count ();
  fh_probe_stat_ex (fd, 0, &reg, &chr, &sz, &bs);
  n = fh_trap_count () - t;
  snprintf (w, sizeof w, "fd %d (%s): stdio stat (exact 0): isatty %d == S_ISCHR %d", fd, what, tty, chr);
  check (tty == chr && reg == !chr, w);
  if (tty)
    {
      snprintf (w, sizeof w, "fd %d (%s): stdio stat (exact 0) after isatty makes no trap (got %ld)", fd, what, n);
      check (n == 0, w);
    }
}

/* fd Fforce'd to a file AFTER its isatty answer was cached as a terminal, the
 * cache not cleared: still S_IFCHR, BUFSIZ (upstream limit, documented). */
static void
stale_case (int fd, long size)
{
  int st, fl, reg, chr;
  long sz, bs;
  char w[200];

  fh_probe_isatty (fd, &st, &fl);
  fh_redirect ((short) fd, size);
  fh_probe_stat (fd, &reg, &chr, &sz, &bs);
  snprintf (w, sizeof w, "fd %d: Fforce after a cached isatty, no cache reset: stays S_IFCHR, BUFSIZ (got chr %d, %ld)",
	    fd, chr, bs);
  check (st == fh_fh_isatty () && chr && !reg && bs == fh_bufsiz (), w);
  fh_reset_cache ();
  fh_probe_stat (fd, &reg, &chr, &sz, &bs);
  snprintf (w, sizeof w, "fd %d: after the cache reset: S_IFREG, size %ld, st_blksize %ld (got reg %d, %ld, %ld)",
	    fd, size, WANT_FILE_BLKSIZE, reg, sz, bs);
  check (reg && !chr && sz == size && bs == WANT_FILE_BLKSIZE, w);
}

static void
sys_fstat_case (int fd, int exact)
{
  int en, hok, bok;
  long n, r;
  char w[200];

  r = fh_probe_sys_fstat (fd, exact, &en, &hok, &bok, &n);
  snprintf (w, sizeof w, "__sys_fstat(%d, exact %d): one Ffstat64 trap with fd and buffer, EINVFN = -ENOSYS returned (got %ld, enosys %d, fd %d, buf %d, traps %ld)",
	    fd, exact, r, en, hok, bok, n);
  check (r == -32 && en && hok && bok && n == 1, w);
}

/* fstat() (exact 1) of a file handle: one Fdatime GET on fd, all three
 * times = want; the exact-0 stat: no Fdatime, no time(), times 0. */
static void
stamp_case (int fd, long want, const char *what)
{
  long t[3], nt, nc;
  int dh, drw;
  char w[240];

  fh_probe_times (fd, 1, t, &nt, &nc, &dh, &drw);
  snprintf (w, sizeof w, "fd %d (%s): fstat mtime=atime=ctime=%ld (got %ld %ld %ld), Fdatime get on fd (got fd %d rw %d), no time() (got %ld)",
	    fd, what, want, t[0], t[1], t[2], dh, drw, nc);
  check (t[0] == want && t[1] == want && t[2] == want && dh == fd && drw == 0 && nc == 0, w);
  fh_probe_times (fd, 0, t, &nt, &nc, &dh, &drw);
  snprintf (w, sizeof w, "fd %d (%s): exact 0: times 0, no Fdatime, no time() (got %ld %ld %ld, fd %d, %ld)",
	    fd, what, t[0], t[1], t[2], dh, nc);
  check (t[0] == 0 && t[1] == 0 && t[2] == 0 && dh == -999 && nc == 0, w);
}

/* fstat() (exact 1) of a terminal / a handle Fdatime rejects: time() - 2;
 * fdatime_tried says whether an Fdatime is expected first. */
static void
now_case (int fd, int fdatime_tried, const char *what)
{
  long t[3], nt, nc, want = fh_now () - 2;
  int dh, drw;
  char w[240];

  fh_probe_times (fd, 1, t, &nt, &nc, &dh, &drw);
  snprintf (w, sizeof w, "fd %d (%s): fstat mtime=atime=ctime=time()-2=%ld (got %ld %ld %ld, time() calls %ld, Fdatime fd %d)",
	    fd, what, want, t[0], t[1], t[2], nc, dh);
  check (t[0] == want && t[1] == want && t[2] == want && nc == 1
	 && (fdatime_tried ? dh == fd : dh == -999), w);
}

int
main (void)
{
  int fd;

  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("fstat-host: skipped -- host is not LP64\n");
      return 77;
    }
  check (fh_bufsiz () == 1024, "mintlib BUFSIZ is 1024 (include/stdio.h:243)");
  file_case (6, 10485760L);
  file_case (7, 0L);
  file_case (8, 300003L);
  /* start-up: every standard handle at its device */
  fh_reset_cache ();
  small_case (0, 1, "console");
  small_case (1, 1, "console");
  small_case (2, 1, "console");
  small_case (3, 1, "standard device AUX");
  small_case (4, 1, "standard device PRN");
  small_case (5, 1, "standard device");
  small_case (-1, 0, "BIOS device");
  small_case (9, 0, "not open");
  for (fd = 0; fd <= 5; fd++)
    consistency_case (fd, "device");
  now_case (0, 0, "console");	/* TASK/SDK01 F, before the redirection */
  isatty_case (0, 1, -1, "console");
  isatty_case (1, 1, -1, "console");
  isatty_case (2, 1, -1, "console");
  isatty_case (3, 1, -1, "standard device AUX");
  isatty_case (4, 1, -1, "standard device PRN");
  isatty_case (5, 1, -1, "standard device");
  /* a child started with 0, 1, 2, 3 Fforce'd to files: fresh cache */
  fh_redirect (0, 5000L);
  fh_redirect (1, 0L);
  fh_redirect (2, 12345L);
  fh_redirect (3, 4096L);
  fh_reset_cache ();
  file_case (0, 5000L);
  file_case (1, 0L);
  file_case (2, 12345L);
  file_case (3, 4096L);
  fh_reset_cache ();
  for (fd = 0; fd <= 5; fd++)
    consistency_case (fd, fd <= 3 ? "redirected" : "device");
  stale_case (4, 777L);
  fh_reset_cache ();
  isatty_case (3, 0, 0, "standard handle Fforce'd to a file, at 0");
  isatty_case (6, 0, 4242, "regular file, mid-way");
  isatty_case (7, 0, 0, "empty regular file");
  isatty_case (9, 0, -1, "not open");
  isatty_case (-1, 1, -1, "BIOS device");
  sys_fstat_case (0, 0);
  sys_fstat_case (6, 1);
  sys_fstat_case (-1, 0);
  /* TASK/SDK01 F: timestamps (expected: Python calendar.timegm) */
  fh_reset_cache ();
  stamp_case (6, 1792593478L, "file, 2026-10-21 14:37:58");
  stamp_case (7, 1740787200L, "empty file, 2025-03-01 00:00:00");
  stamp_case (8, 2145916798L, "file, 2037-12-31 23:59:58");
  stamp_case (2, 1709208002L, "standard handle Fforce'd to a file, 2024-02-29 12:00:02");
  now_case (5, 0, "standard device");
  now_case (9, 1, "not open");
  now_case (-1, 1, "BIOS device");
  printf ("fstat-host: %d checks, %d failed\n", checks, failed);
  return failed;
}
