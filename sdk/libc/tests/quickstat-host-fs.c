/*
 * quickstat-host-fs.c -- the GEMDOS side of quickstat-host (user decision
 * 2026-10-02, TASK/PORT10: the executable bit follows NewTOS's native
 * extensions .NTP/.NAP/.NCP, case-insensitively).
 *
 * Compiled by quickstat-host.sh with the mintlib include path and DEFS of
 * sdk/libc/build-libc.sh, so `struct stat', _DTA, FA_* and the errno values
 * are mintlib's. It stands in for what mintlib's __quickstat (the overlay
 * sdk/libc/overlay/mintlib/quickstat.c, or with QUICKSTAT_SRC the pristine
 * one) reaches on the target:
 *
 *   __sys_stat   -- the lean sdk/libc/newtos-arch/do_stat.c issues the
 *                   Fstat64 trap (GEMDOS 0x14b); NewTOS's GEMDOS answers an
 *                   unimplemented function with EINVFN (-32), which is
 *                   mintlib's -ENOSYS, so __quickstat takes its own Fsfirst
 *                   path. Here: -ENOSYS, always.
 *   Fsfirst      -- a lookup in a fixed table of FAT entries, matched the way
 *                   GEMDOS matches on FAT: case-insensitive (the kernel
 *                   upper-cases name and extension, tos-api/src/bdos/fs/dir.rs
 *                   fcb_name), '/' and '\' the same. A hit fills the caller's
 *                   DTA (set with Fsetdta) with the entry's attribute, size
 *                   and upper-case name; a miss is -ENOENT (GEMDOS EFILNF).
 *   Fgetdta/Fsetdta/Dgetdrv, _unx2dos ('/' -> '\', never a device),
 *   _enoent, __inode, __libc_unix_names (0, mintlib/globals.c's value),
 *   errno, geteuid/getegid (renamed in, 0).
 *
 * The entries are upper case, as the FAT short names on the card are.
 * qs_test_mode() and qs_test_access() hand the harness plain ints, so the
 * harness itself can use the host's headers.
 */
#include <errno.h>
#include <stddef.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include "lib.h"

int __access (const char *, int);	/* newtos-arch/access.c */

int errno;
ino_t __inode = 32;
int __libc_unix_names = 0;

struct entry { const char *name; char attr; long size; };

static const struct entry files[] = {
  { "C:\\T\\TOOL.NTP",   0x20, 100 },
  { "C:\\T\\GEMAPP.NAP", 0x20, 200 },
  { "C:\\T\\CONS.NCP",   0x20, 300 },
  { "C:\\T\\RO.NCP",     0x21, 300 },	/* FA_RDONLY|FA_ARCHIVE */
  { "C:\\T\\ACC.NAC",    0x20, 400 },	/* accessory: not a program */
  { "C:\\T\\OLD.PRG",    0x20, 500 },
  { "C:\\T\\OLD.TOS",    0x20, 500 },
  { "C:\\T\\OLD.TTP",    0x20, 500 },
  { "C:\\T\\OLD.APP",    0x20, 500 },
  { "C:\\T\\OLD.GTP",    0x20, 500 },
  { "C:\\T\\README.TXT", 0x20, 600 },
  { "C:\\T\\NOEXT",      0x20, 700 },
  { "C:\\T\\SUB.NCP",    0x10, 0 },	/* a directory named like a program */
  { NULL, 0, 0 }
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
lookup (const char *name)
{
  int i;

  for (i = 0; files[i].name; i++)
    {
      const char *a = files[i].name, *b = name;

      while (*a && fold (*a) == fold (*b))
	a++, b++;
      if (*a == '\0' && *b == '\0')
	return &files[i];
    }
  return NULL;
}

static _DTA *cur_dta;

_DTA *qs_test_Fgetdta (void) { return cur_dta; }
void qs_test_Fsetdta (_DTA *d) { cur_dta = d; }
short qs_test_Dgetdrv (void) { return 2; }
uid_t qs_test_geteuid (void) { return 0; }
gid_t qs_test_getegid (void) { return 0; }

long
qs_test_Fsfirst (const char *spec, int attr)
{
  const struct entry *e = lookup (spec);
  const char *base, *p;
  int i;

  (void) attr;
  if (!e)
    return -ENOENT;
  for (base = p = e->name; *p; p++)
    if (*p == '\\')
      base = p + 1;
  cur_dta->dta_attribute = e->attr;
  cur_dta->dta_size = (int) e->size;
  for (i = 0; base[i] && i < 13; i++)
    cur_dta->dta_name[i] = base[i];
  cur_dta->dta_name[i] = '\0';
  return 0;
}

long
__sys_stat (const char *path, struct stat *st, int lflag, int exact)
{
  (void) path; (void) st; (void) lflag; (void) exact;
  return -ENOSYS;
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
  return 0;
}

/* __quickstat's return value; *mode = st_mode on success. */
int
qs_test_mode (const char *path, unsigned *mode)
{
  struct stat st;
  int r = __quickstat (path, &st, 0);

  *mode = r == 0 ? (unsigned) st.st_mode : 0;
  return r;
}

/* the lean newtos-arch/access.c over that __quickstat */
int
qs_test_access (const char *path, int amode)
{
  return __access (path, amode);
}

int qs_test_errno (void) { return errno; }
void qs_test_set_errno (int e) { errno = e; }
int qs_test_eacces (void) { return EACCES; }
int qs_test_enoent (void) { return ENOENT; }
unsigned qs_test_s_ifmt (void) { return S_IFMT; }
unsigned qs_test_s_ifreg (void) { return S_IFREG; }
unsigned qs_test_s_ifdir (void) { return S_IFDIR; }
int qs_test_x_ok (void) { return X_OK; }
int qs_test_w_ok (void) { return W_OK; }
int qs_test_f_ok (void) { return F_OK; }
