/*
 * stat-host-fs.c -- the GEMDOS side of stat-host (TASK/SDK01 F: stat() and
 * lstat() fill st_mtime/st_atime/st_ctime from the DTA, as mintlib's
 * __do_stat does on TOS, mintlib/do_stat.c:281-282).
 *
 * Compiled by stat-host.sh with the mintlib include path and DEFS of
 * sdk/libc/build-libc.sh, so `struct stat', _DTA, FA_*, time_t and errno
 * are mintlib's. It stands in for what newtos-arch/do_stat.c reaches on the
 * target:
 *
 *   Fstat64   -- EINVFN (-32) = mintlib's -ENOSYS, always: NewTOS has no
 *                GEMDOS 0x14b (tos-api/src/gemdos/mod.rs, id > MAX_FN), so
 *                __do_stat takes its Fsfirst path. Counted.
 *   Fsfirst   -- a lookup of the search spec in a fixed table, matched
 *                case-insensitively with '/' = '\' (the kernel upper-cases
 *                FAT names, tos-api/src/bdos/fs/dir.rs fcb_name). The table
 *                holds the exact specs __do_stat's "."/".."/"dir\" kludge
 *                produces ("C:\T\SUB\*.*" -> the "." entry, "C:\*.*" -> the
 *                first root entry). A hit fills the DTA set with Fsetdta:
 *                attribute, DOS time, DOS date, size, name (the kernel's
 *                makbuf, tos-api/src/bdos/fs/find.rs); a miss is -ENOENT.
 *                Records the last spec.
 *   Fgetdta/Fsetdta, Dgetdrv (2 = C:), _unx2dos ('/' -> '\'; "CON:" is a
 *                device name -> 1, as mintlib's unx2dos.c returns for
 *                device names), _enoent, __inode, __libc_unix_names (0,
 *                mintlib/globals.c), errno, geteuid/getegid (renamed in, 0).
 *   time      -- ST_NOW, counted (on the target: Tgettimeofday/Tgettime/
 *                Tgetdate, unix/gettimeofday.c).
 *   mktime    -- what mintlib/unixtime.c (linked unchanged) calls: mintlib's
 *                mktime with TZ unset and no /etc/localtime falls back to
 *                UTC (tz/localtime.c zoneinit("")), so it is modelled by the
 *                host's timegm() (stat-host.c st_host_timegm), which also
 *                normalises tm_mon -1 / tm_mday 0 the same way.
 */
#include <errno.h>
#include <stddef.h>
#include <string.h>
#include <time.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include "lib.h"

int errno;
ino_t __inode = 32;
int __libc_unix_names = 0;

struct entry {
  const char *spec;		/* what Fsfirst is asked */
  const char *name;		/* the DTA name it answers */
  char attr;
  long size;
  unsigned short time, date;
};

static const struct entry files[] = {
  /* 2026-10-21 14:37:58 */
  { "C:\\T\\A.TXT",       "A.TXT",   0x20, 1234,  0x74bd, 0x5d55 },
  /* read-only, 2024-02-29 12:00:02 */
  { "C:\\T\\RO.NCP",      "RO.NCP",  0x21, 300,   0x6001, 0x585d },
  /* a folder, 2025-03-01 00:00:00 */
  { "C:\\T\\SUB",         "SUB",     0x10, 0,     0x0000, 0x5a61 },
  /* "C:\T\SUB\" and "C:\T\SUB\." search <dir>\*.*: the "." entry, which
     FAT stamps with the folder's creation time, here 1980-01-01 00:00:00 */
  { "C:\\T\\SUB\\*.*",    ".",       0x10, 0,     0x0000, 0x0021 },
  { "C:\\T\\SUB\\.\\*.*", ".",       0x10, 0,     0x0000, 0x0021 },
  /* "C:\.." searches the root, whose first entry is not "." */
  { "C:\\*.*",            "AUTO",    0x10, 0,     0x74bd, 0x5d55 },
  /* 2037-12-31 23:59:58: the last stamp mintlib's 32-bit time_t holds */
  { "C:\\T\\LATE.DAT",    "LATE.DAT",0x20, 9,     0xbf7d, 0x739f },
  { NULL, NULL, 0, 0, 0, 0 }
};

static int
fold (int c)
{
  if (c >= 'a' && c <= 'z')
    return c - 'a' + 'A';
  if (c == '/')
    return '\\';
  return c;
}

static const struct entry *
lookup (const char *spec)
{
  int i;

  for (i = 0; files[i].spec; i++)
    {
      const char *a = files[i].spec, *b = spec;

      while (*a && fold (*a) == fold (*b))
	a++, b++;
      if (*a == '\0' && *b == '\0')
	return &files[i];
    }
  return NULL;
}

static _DTA *cur_dta;
static char last_spec[256];
static long fstat64_calls, time_calls;

#define ST_NOW 1792600000L

_DTA *st_test_Fgetdta (void) { return cur_dta; }
void st_test_Fsetdta (_DTA *d) { cur_dta = d; }
short st_test_Dgetdrv (void) { return 2; }
uid_t st_test_geteuid (void) { return 0; }
gid_t st_test_getegid (void) { return 0; }

long
st_test_Fstat64 (int flag, const char *name, void *st)
{
  (void) flag; (void) name; (void) st;
  fstat64_calls++;
  return -32;				/* EINVFN: no GEMDOS 0x14b */
}

long
st_test_Fsfirst (const char *spec, int attr)
{
  const struct entry *e = lookup (spec);
  int i;

  (void) attr;
  strncpy (last_spec, spec, sizeof last_spec - 1);
  if (!e)
    return -ENOENT;
  memset (cur_dta, 0, sizeof *cur_dta);
  cur_dta->dta_attribute = e->attr;
  cur_dta->dta_time = e->time;
  cur_dta->dta_date = e->date;
  cur_dta->dta_size = (int) e->size;
  for (i = 0; e->name[i] && i < 13; i++)
    cur_dta->dta_name[i] = e->name[i];
  cur_dta->dta_name[i] = '\0';
  return 0;
}

time_t
time (time_t *tp)
{
  time_calls++;
  if (tp)
    *tp = ST_NOW;
  return ST_NOW;
}

long st_host_timegm (int, int, int, int, int, int);	/* stat-host.c */

time_t
mktime (struct tm *tm)
{
  return (time_t) st_host_timegm (tm->tm_year, tm->tm_mon, tm->tm_mday,
				  tm->tm_hour, tm->tm_min, tm->tm_sec);
}

int
_enoent (const char *path)
{
  (void) path;
  return 1;
}

int
_unx2dos (const char *unx, char *dos, size_t len)
{
  size_t i;

  for (i = 0; unx[i] && i + 1 < len; i++)
    dos[i] = unx[i] == '/' ? '\\' : unx[i];
  dos[i] = '\0';
  return strcmp (unx, "CON:") == 0 ? 1 : 0;
}

/* stat(path) / lstat(path) through __do_stat (unix/stat.c, unix/lstat.c):
 * its result; t[0..2] = st_mtime, st_atime, st_ctime; *mode, *size; *err =
 * errno on failure; *nf = Fstat64 calls, *nt = time() calls; spec = the
 * last Fsfirst spec ("" = none). */
int
st_probe (const char *path, int lflag, long t[3], unsigned *mode, long *size,
	  int *err, long *nf, long *nt, char *spec, size_t speclen)
{
  struct stat st;
  long f0 = fstat64_calls, c0 = time_calls;
  int r;

  last_spec[0] = '\0';
  errno = 0;
  memset (&st, 0x5a, sizeof st);
  r = __do_stat (path, &st, lflag);
  t[0] = (long) st.st_mtime;
  t[1] = (long) st.st_atime;
  t[2] = (long) st.st_ctime;
  *mode = (unsigned) st.st_mode;
  *size = (long) st.st_size;
  *err = errno;
  *nf = fstat64_calls - f0;
  *nt = time_calls - c0;
  strncpy (spec, last_spec, speclen - 1);
  spec[speclen - 1] = '\0';
  return r;
}

long st_now (void) { return ST_NOW; }
int st_enoent (void) { return ENOENT; }
unsigned st_s_ifmt (void) { return S_IFMT; }
unsigned st_s_ifreg (void) { return S_IFREG; }
unsigned st_s_ifdir (void) { return S_IFDIR; }
unsigned st_s_ifchr (void) { return S_IFCHR; }
