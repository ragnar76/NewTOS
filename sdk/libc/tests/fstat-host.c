/*
 * fstat-host.c -- host harness for the stdio buffer size of a regular file
 * (TASK/FS08 part 4, user decision 2026-10-07: "a larger buffer is no
 * problem, we have more memory than a real Atari").
 *
 * mintlib's stdio takes a stream's buffer size from st_blksize
 * (stdio/stdio_init.c:72-77), which the lean sdk/libc/newtos-arch/do_fstat.c
 * fills. Before FS08 part 4 it was BUFSIZ (1024) for every handle; now a
 * GEMDOS file handle (>= 6) gets NEWTOS_FILE_BLKSIZE, everything else keeps
 * BUFSIZ. fstat-host.sh links the shim (unchanged), mintlib's own
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
 *   unchanged     handle 3 (a standard handle, here seekable as if Fforce'd
 *                 to a file), handle -1 (a BIOS device that answers Fseek
 *                 0) and handle 9 (not open, Fseek EIHNDL): st_blksize BUFSIZ.
 *   isatty        (mintlib/isatty.c: Fcntl -> EINVFN, then the Fseek probe)
 *                 handles 0, 1, 2 (devices): 1, status FH_ISATTY, flags
 *                 CRMOD|ECHO -- the flags __read needs to take the Fread line
 *                 path instead of the RAW path; handle 3 (Fforce'd to a file),
 *                 6 (file, mid-way), 7 (empty file), 9 (not open): 0, status
 *                 FH_ISAFILE, file position unchanged by the probe; handle -1
 *                 (BIOS device): 1 (upstream); a second query is answered from
 *                 __open_stat[] without a trap.
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdio.h>

int fh_probe_stat (int, int *, int *, long *, long *);
long fh_probe_stream (int, int *, int *);
void fh_set_pos (short, long);
long fh_get_pos (short);
long fh_bufsiz (void);
int fh_probe_isatty (int, int *, int *);
long fh_trap_count (void);
int fh_fh_isatty (void);
int fh_fh_isafile (void);
int fh_crmod_echo (void);

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

int
main (void)
{
  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("fstat-host: skipped -- host is not LP64\n");
      return 77;
    }
  check (fh_bufsiz () == 1024, "mintlib BUFSIZ is 1024 (include/stdio.h:243)");
  file_case (6, 10485760L);
  file_case (7, 0L);
  file_case (8, 300003L);
  small_case (0, 1, "console");
  small_case (1, 1, "console");
  small_case (2, 1, "console");
  small_case (3, 0, "standard handle, seekable");
  small_case (-1, 0, "BIOS device");
  small_case (9, 0, "not open");
  isatty_case (0, 1, -1, "console");
  isatty_case (1, 1, -1, "console");
  isatty_case (2, 1, -1, "console");
  isatty_case (3, 0, 0, "standard handle Fforce'd to a file, at 0");
  isatty_case (6, 0, 4242, "regular file, mid-way");
  isatty_case (7, 0, 0, "empty regular file");
  isatty_case (9, 0, -1, "not open");
  isatty_case (-1, 1, -1, "BIOS device");
  printf ("fstat-host: %d checks, %d failed\n", checks, failed);
  return failed;
}
