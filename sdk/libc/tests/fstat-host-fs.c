/*
 * fstat-host-fs.c -- the GEMDOS side and the mintlib-typed probes of
 * fstat-host (TASK/FS08 part 4: st_blksize, and through it the stdio buffer
 * size, for a regular file).
 *
 * Compiled by fstat-host.sh with the mintlib include path and DEFS of
 * sdk/libc/build-libc.sh, so `struct stat', FILE and BUFSIZ are mintlib's.
 * It stands in for the GEMDOS calls the lean __do_fstat and mintlib's own
 * __isatty (mintlib/isatty.c) make:
 *
 *   Fseek  -- a table of open handles, each with a size and a position;
 *             SEEK_SET/CUR/END move the position and return it, as GEMDOS
 *             Fseek does (docs/api/gemdos/file/42-fseek.md). A handle not in
 *             the table answers EIHNDL (-37). A character device -- a negative
 *             handle (a BIOS device, e.g. -1 = CON:) or a standard handle 0-5
 *             at its default device mapping -- answers 0 whatever the offset,
 *             as the spec's "Character-device and standard handles" case and
 *             the kernel (tos-api/src/bdos/fs/file.rs, Target::Device) do.
 *             For -1 that means it LOOKS seekable, so the check that devices
 *             keep BUFSIZ proves that __do_fstat does not size by Fseek
 *             success alone. fh_redirect(h, size) puts a standard handle 0-5
 *             into the table -- the handle Fforce'd to a file of that size
 *             (TASK/LIBC-FSTAT); from then on Fseek answers its position.
 *   Fcntl  -- answers EINVFN (-32) for every call: NewTOS, like EmuTOS, has no
 *             GEMDOS function 0x104 (tos-api/src/gemdos/mod.rs, id > MAX_FN),
 *             which is the -ENOSYS __isatty needs to take its Fseek probe.
 *
 *   Ffstat64 -- answers EINVFN (-32) for every call: NewTOS has no GEMDOS
 *             0x15d either (same dispatcher). It records the handle and the
 *             buffer it was given, so the harness sees that __sys_fstat
 *             (TASK/SDK01 A) makes the real trap with its own arguments and
 *             returns the kernel's answer unchanged.
 *   Fdatime -- (TASK/SDK01 F) get (rwflag 0): a handle in the table writes
 *             its DOS time WORD then date WORD (the FAT entry's 0x16..0x19,
 *             docs/api/gemdos/file/57-fdatime.md; tos-api/src/bdos/fs/
 *             file.rs fdatime); any other handle -- a device or one that
 *             is not open -- is EIHNDL (-37), as the kernel's ofd_of()
 *             answers a handle that is not a file. Records handle and
 *             rwflag of the last call.
 *   time    -- (TASK/SDK01 F) FH_NOW, counted separately (on the target it
 *             is Tgettimeofday/Tgettime/Tgetdate, unix/gettimeofday.c).
 *   mktime  -- (TASK/SDK01 F) what mintlib/unixtime.c (linked unchanged)
 *             calls: mintlib's mktime with TZ unset and no /etc/localtime
 *             falls back to UTC (tz/localtime.c zoneinit("")), so it is
 *             modelled by the host's timegm() (fstat-host.c fh_host_timegm),
 *             which also normalises tm_mon -1 / tm_mday 0 the same way.
 *
 * Every Fseek/Fcntl call is counted (fh_trap_count) so the harness can see
 * __isatty's cache answer without a trap. fh_reset_cache() clears
 * __open_stat[] as mintlib's start-up does (mintlib/main.c:161-162), i.e. what
 * a child started with an inherited redirection sees.
 *
 * fh_probe_stat(), fh_probe_stream() and fh_probe_isatty() hand the harness
 * plain integers, so the harness (fstat-host.c) can use the host's headers.
 */
#include <string.h>
#include <errno.h>
#include <stdio.h>
#include <sys/stat.h>
#include <time.h>		/* time, mktime, struct tm (mintlib) */
#include <unistd.h>		/* __isatty */
#include <sys/ioctl.h>		/* CRMOD, ECHO */
#include "lib.h"

struct fh { short h; long size; long pos; unsigned short time, date; };
static struct fh tab[] = {
	/* a regular file, 10 MiB, stamped 2026-10-21 14:37:58 */
	{ 6, 10485760L, 0, 0x74bd, 0x5d55 },
	/* an empty regular file, stamped 2025-03-01 00:00:00 */
	{ 7, 0L, 0, 0x0000, 0x5a61 },
	/* a regular file, odd size, stamped 2037-12-31 23:59:58 (the last
	 * DOS stamp mintlib's 32-bit time_t, include/bits/types.h:73, holds) */
	{ 8, 300003L, 0, 0xbf7d, 0x739f },
	{ -99, 0L, 0, 0, 0 }, { -99, 0L, 0, 0, 0 }, { -99, 0L, 0, 0, 0 },	/* free: fh_redirect */
	{ -99, 0L, 0, 0, 0 }, { -99, 0L, 0, 0, 0 }, { -99, 0L, 0, 0, 0 },
};
#define NTAB ((int)(sizeof tab / sizeof tab[0]))

static long traps;

long fh_trap_count (void) { return traps; }

long
fh_test_Fcntl (short h, void *arg, short cmd)
{
	(void) h; (void) arg; (void) cmd;
	traps++;
	return -32;				/* EINVFN: no GEMDOS 0x104 */
}

static short ffstat64_h = -999;
static void *ffstat64_buf;

long
fh_test_Ffstat64 (short h, void *st)
{
	traps++;
	ffstat64_h = h;
	ffstat64_buf = st;
	return -32;				/* EINVFN: no GEMDOS 0x15d */
}

static short fdatime_h = -999, fdatime_rw = -999;

long
fh_test_Fdatime (_DOSTIME *tp, short h, short rw)
{
	int i;
	traps++;
	fdatime_h = h;
	fdatime_rw = rw;
	for (i = 0; i < NTAB; i++)
		if (tab[i].h == h) {
			if (rw == 0) {
				tp->time = tab[i].time;
				tp->date = tab[i].date;
			}
			return 0;
		}
	return -37;				/* EIHNDL: not a file */
}

#define FH_NOW 1792600000L
static long time_calls;

time_t
time (time_t *tp)
{
	time_calls++;
	if (tp)
		*tp = FH_NOW;
	return FH_NOW;
}

long fh_now (void) { return FH_NOW; }

long fh_host_timegm (int, int, int, int, int, int);	/* fstat-host.c */

time_t
mktime (struct tm *tm)
{
	return (time_t) fh_host_timegm (tm->tm_year, tm->tm_mon, tm->tm_mday,
					tm->tm_hour, tm->tm_min, tm->tm_sec);
}

long
fh_test_Fseek (long off, short h, short mode)
{
	int i;
	traps++;
	for (i = 0; i < NTAB; i++) {
		long np;
		if (tab[i].h != h)
			continue;
		np = mode == SEEK_SET ? off : mode == SEEK_CUR ? tab[i].pos + off : tab[i].size + off;
		if (np < 0 || np > tab[i].size)
			return -64;		/* ERANGE, as GEMDOS */
		tab[i].pos = np;
		return np;
	}
	if (h <= 5)
		return 0;			/* character device */
	return -37;				/* EIHNDL */
}

/* Fforce(h, <file of `size' bytes>) for a standard handle 0..5: from now on
 * Fseek on h answers the file position (starting at 0). */
void
fh_redirect (short h, long size)
{
	int i;
	for (i = 0; i < NTAB; i++)
		if (tab[i].h == -99) {
			tab[i].h = h;
			tab[i].size = size;
			tab[i].pos = 0;
			tab[i].time = 0x6001;	/* 2024-02-29 12:00:02 */
			tab[i].date = 0x585d;
			return;
		}
}

/* mintlib/main.c:161-162: every __open_stat[] entry back to FH_UNKNOWN. */
void
fh_reset_cache (void)
{
	int i;
	for (i = 0; i < __NHANDLES; i++)
		__open_stat[i].status = FH_UNKNOWN;
}

void
fh_set_pos (short h, long pos)
{
	int i;
	for (i = 0; i < NTAB; i++)
		if (tab[i].h == h)
			tab[i].pos = pos;
}

long
fh_get_pos (short h)
{
	int i;
	for (i = 0; i < NTAB; i++)
		if (tab[i].h == h)
			return tab[i].pos;
	return -1;
}

long fh_bufsiz (void) { return BUFSIZ; }

/* __do_fstat(fd, exact): returns its result; *mode, *size, *blksize from
 * the stat. */
int
fh_probe_stat_ex (int fd, int exact, int *is_reg, int *is_chr, long *size, long *blksize)
{
	struct stat st;
	int r = __do_fstat (fd, &st, exact);
	*is_reg = S_ISREG (st.st_mode) ? 1 : 0;
	*is_chr = S_ISCHR (st.st_mode) ? 1 : 0;
	*size = (long) st.st_size;
	*blksize = (long) st.st_blksize;
	return r;
}

/* fstat()'s call: __do_fstat(fd, &st, 1) (unix/fstat.c) */
int
fh_probe_stat (int fd, int *is_reg, int *is_chr, long *size, long *blksize)
{
	return fh_probe_stat_ex (fd, 1, is_reg, is_chr, size, blksize);
}

/* __do_fstat(fd, &st, exact) (TASK/SDK01 F): its result; t[0..2] =
 * st_mtime, st_atime, st_ctime; *ntraps = GEMDOS calls (Fseek/Fcntl/
 * Fdatime); *ntime = time() calls; *dh, *drw = handle and rwflag of the
 * last Fdatime (-999 = none made). */
int
fh_probe_times (int fd, int exact, long t[3], long *ntraps, long *ntime, int *dh, int *drw)
{
	struct stat st;
	long t0 = traps, c0 = time_calls;
	int r;
	fdatime_h = fdatime_rw = -999;
	r = __do_fstat (fd, &st, exact);
	t[0] = (long) st.st_mtime;
	t[1] = (long) st.st_atime;
	t[2] = (long) st.st_ctime;
	*ntraps = traps - t0;
	*ntime = time_calls - c0;
	*dh = fdatime_h;
	*drw = fdatime_rw;
	return r;
}

static int
fh_dummy_seek (void *cookie, fpos_t *pos, int whence)
{
	(void) cookie; (void) pos; (void) whence;
	return 0;
}

/* mintlib's own __stdio_init_stream (stdio/stdio_init.c) on a fresh FILE
 * whose cookie is fd -- what fopen/fdopen run before the first transfer --
 * returns the __bufsize it chose; *linebuf and *seekable as it left them. */
long
fh_probe_stream (int fd, int *linebuf, int *seekable)
{
	FILE f;
	memset (&f, 0, sizeof f);
	f.__cookie = (void *) (long) fd;
	f.__io_funcs.__seek = fh_dummy_seek;	/* as __stdio_check_funcs sets it */
	__stdio_init_stream (&f);
	*linebuf = f.__linebuf;
	*seekable = f.__io_funcs.__seek != NULL;
	return (long) f.__bufsize;
}

/* mintlib's own __isatty(fd); *status and *flags as it left
 * __open_stat[__OPEN_INDEX(fd)]. */
int
fh_probe_isatty (int fd, int *status, int *flags)
{
	int r = __isatty (fd);
	*status = __open_stat[__OPEN_INDEX (fd)].status;
	*flags = __open_stat[__OPEN_INDEX (fd)].flags;
	return r;
}

int fh_fh_isatty (void) { return FH_ISATTY; }
int fh_fh_isafile (void) { return FH_ISAFILE; }
int fh_crmod_echo (void) { return CRMOD | ECHO; }

/* __sys_fstat(fd, &st, exact) (TASK/SDK01 A): its result; *is_enosys = the
 * result is mintlib's -ENOSYS; *handle_ok/*buf_ok = the Ffstat64 trap got fd
 * and &st; *ntraps = traps made by the call. */
long
fh_probe_sys_fstat (int fd, int exact, int *is_enosys, int *handle_ok, int *buf_ok, long *ntraps)
{
	struct stat st;
	long t0 = traps, r;
	ffstat64_h = -999;
	ffstat64_buf = NULL;
	r = __sys_fstat ((short) fd, &st, exact);
	*is_enosys = r == -ENOSYS;
	*handle_ok = ffstat64_h == (short) fd;
	*buf_ok = ffstat64_buf == (void *) &st;
	*ntraps = traps - t0;
	return r;
}
