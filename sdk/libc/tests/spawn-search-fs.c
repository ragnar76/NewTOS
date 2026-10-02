/*
 * spawn-search-fs.c -- the GEMDOS side of spawn-search-host (TASK/PORT10,
 * user decision 2026-10-02: native executable extensions).
 *
 * Compiled by spawn-host.sh with the mintlib include path and DEFS of
 * sdk/libc/build-libc.sh, so `struct stat', S_IFREG and the errno values are
 * mintlib's. It stands in for what the real search reaches on the target:
 *
 *   __quickstat  -- what mintlib/buffindf.c's EXISTS() calls; here a lookup in
 *                   a fixed table of FAT names, matched the way GEMDOS matches
 *                   them on FAT: case-insensitive (tos-api/src/bdos/fs/dir.rs
 *                   fcb_name upper-cases both the 8 and the 3 part), with '/'
 *                   and '\' as the same separator.
 *   __execve     -- what posix/execvp.c tries name by name (renamed in with
 *                   -D__execve=spawn_test_execve). A name in the table is
 *                   "found": recorded, and failed with E2BIG so execvp stops
 *                   and returns (a real execve would not return). Anything
 *                   else fails with ENOENT, and execvp tries the next one.
 *   __mempcpy, errno, __mint (0, as on NewTOS -- execvp then walks its
 *                   extension list).
 *
 * The directory entries are upper case, as the FAT short names on the card
 * are (C:\P10\P10KID.NCP).
 */
#include <errno.h>
#include <stddef.h>
#include <sys/stat.h>

int errno;
int __mint = 0;

static const char *const files[] = {
  "C:\\P10\\P10KID.NCP",		/* the measured case */
  "C:\\BIN\\TOOL.NTP",
  "C:\\BIN\\GEMAPP.NAP",
  "C:\\BIN\\TRIO.NCP", "C:\\BIN\\TRIO.NAP", "C:\\BIN\\TRIO.NTP",
  "C:\\BIN\\DUO.NCP", "C:\\BIN\\DUO.NAP",
  "C:\\BIN\\ACC.NAC",			/* accessory: not a program */
  "C:\\OLD\\LEGACY.PRG", "C:\\OLD\\LEGACY.TOS", "C:\\OLD\\LEGACY.TTP",
  "C:\\OLD\\LEGACY.APP", "C:\\OLD\\LEGACY.GTP",
  "C:\\OLD\\MIXED.PRG", "C:\\OLD\\MIXED.NCP",
  /* for execvp, whose PATH walk joins with '/' and whose "" element means
     the current directory (a name without directory) */
  "\\XBIN\\XTOOL.NAP", "\\XBIN\\XOLD.PRG",
  "HERE.NCP",
  NULL
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

/* 1 if `name' is one of the table's files, compared as GEMDOS does on FAT. */
int
spawn_test_fs_has (const char *name)
{
  int i;

  for (i = 0; files[i]; i++)
    {
      const char *a = files[i], *b = name;

      while (*a && fold (*a) == fold (*b))
	a++, b++;
      if (*a == '\0' && *b == '\0')
	return 1;
    }
  return 0;
}

int
__quickstat (const char *path, struct stat *st, int lflag)
{
  char *p = (char *) st;
  size_t i;

  (void) lflag;
  for (i = 0; i < sizeof *st; i++)
    p[i] = 0;
  if (!spawn_test_fs_has (path))
    {
      __set_errno (ENOENT);
      return -1;
    }
  st->st_mode = S_IFREG | 0644;
  return 0;
}

/* the last name __execve was handed that exists ("" = none) */
char spawn_test_exec_found[256];
int spawn_test_exec_tries;

int
spawn_test_execve (const char *path, char *const argv[], char *const envp[])
{
  size_t i;

  (void) argv;
  (void) envp;
  spawn_test_exec_tries++;
  if (spawn_test_fs_has (path))
    {
      for (i = 0; path[i] && i < sizeof spawn_test_exec_found - 1; i++)
	spawn_test_exec_found[i] = path[i];
      spawn_test_exec_found[i] = '\0';
      __set_errno (E2BIG);
      return -1;
    }
  __set_errno (ENOENT);
  return -1;
}

void *
spawn_test_mempcpy (void *dst, const void *src, size_t n)
{
  char *d = dst;
  const char *s = src;

  while (n--)
    *d++ = *s++;
  return d;
}

int spawn_test_errno (void) { return errno; }
void spawn_test_set_errno (int e) { errno = e; }
int spawn_test_enoent (void) { return ENOENT; }
int spawn_test_e2big (void) { return E2BIG; }
