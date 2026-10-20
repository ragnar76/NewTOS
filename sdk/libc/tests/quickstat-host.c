/*
 * quickstat-host.c -- host harness for the executable bit of mintlib's
 * __quickstat (user decision 2026-10-02, TASK/PORT10).
 *
 * mintlib/quickstat.c sets st_mode |= 0111 for a file whose name ends in
 * ".app", ".gtp", ".ttp", ".prg" or ".tos", compared with strcmp -- lower
 * case only, against the name as the CALLER spelled it. access()
 * (newtos-arch/access.c), _buffindfile, open() and glob read their stat
 * through it; stat()/lstat() (newtos-arch/do_stat.c __do_stat, its own walk
 * since TASK/SDK01 F) apply the same rule. On NewTOS
 * the executable extensions are the native .ntp/.nap/.ncp, the same set the
 * program search uses (overlay mintlib/spawnvp.c), compared without regard to
 * case because FAT names are upper case (C:\P10\P10KID.NCP). The overlay
 * sdk/libc/overlay/mintlib/quickstat.c replaces the list.
 *
 * quickstat-host.sh links: the overlay quickstat.c (or the file named by
 * QUICKSTAT_SRC, for the before/after counter-check), the lean
 * newtos-arch/access.c, mintlib's ctype.c, quickstat-host-fs.c (fake FAT
 * directory behind Fsfirst, __sys_stat = -ENOSYS as on NewTOS) and this file.
 *
 * Checked, through access(path, X_OK) and st_mode & 0111 (all three bits):
 *   executable   .NTP .NAP .NCP, in upper, lower and mixed case;
 *   not          .PRG .TOS .TTP .APP .GTP (m68k, back with the emulator) and
 *                .NAC (accessory), in upper and lower case; a plain .TXT and
 *                a name without extension;
 *   unchanged    a directory is 0755 (S_IFDIR) whatever its name; a
 *                read-only .NCP is executable but not writable; a missing
 *                file is ENOENT for F_OK.
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdio.h>
#include <string.h>

/* quickstat-host-fs.c */
int qs_test_mode (const char *, unsigned *);
int qs_test_access (const char *, int);
int qs_test_errno (void);
void qs_test_set_errno (int);
int qs_test_eacces (void);
int qs_test_enoent (void);
unsigned qs_test_s_ifmt (void);
unsigned qs_test_s_ifreg (void);
unsigned qs_test_s_ifdir (void);
int qs_test_x_ok (void);
int qs_test_w_ok (void);
int qs_test_f_ok (void);

static int failed, checks;

static void
check (int ok, const char *what, const char *detail)
{
  checks++;
  if (!ok)
    failed++;
  printf ("%s  %s%s%s\n", ok ? "PASS" : "FAIL", what,
	  detail ? " -- " : "", detail ? detail : "");
}

/* path must stat as a regular file with exec bits all set (want=1) or all
   clear (want=0), and access(X_OK) must agree. */
static void
exec_case (const char *path, int want)
{
  char what[160], detail[160];
  unsigned mode = 0;
  int r, a, e;

  r = qs_test_mode (path, &mode);
  qs_test_set_errno (0);
  a = qs_test_access (path, qs_test_x_ok ());
  e = qs_test_errno ();
  snprintf (what, sizeof what, "%-20s %s", path,
	    want ? "executable" : "not executable");
  snprintf (detail, sizeof detail, "stat=%d st_mode=0%o access(X_OK)=%d errno=%d",
	    r, mode, a, a ? e : 0);
  check (r == 0
	 && (mode & qs_test_s_ifmt ()) == qs_test_s_ifreg ()
	 && (mode & 0111) == (want ? 0111u : 0u)
	 && (want ? a == 0 : (a == -1 && e == qs_test_eacces ())),
	 what, detail);
}

int
main (void)
{
  static const char *const yes[] = {
    "C:\\T\\TOOL.NTP", "C:\\T\\GEMAPP.NAP", "C:\\T\\CONS.NCP",
    "c:\\t\\tool.ntp", "c:\\t\\gemapp.nap", "c:\\t\\cons.ncp",
    "C:\\T\\Tool.Ntp", "C:\\T\\GemApp.nAp", "C:/T/CONS.nCP",
    NULL
  };
  static const char *const no[] = {
    "C:\\T\\OLD.PRG", "C:\\T\\OLD.TOS", "C:\\T\\OLD.TTP",
    "C:\\T\\OLD.APP", "C:\\T\\OLD.GTP", "C:\\T\\ACC.NAC",
    "c:\\t\\old.prg", "c:\\t\\old.tos", "c:\\t\\old.ttp",
    "c:\\t\\old.app", "c:\\t\\old.gtp", "c:\\t\\acc.nac",
    "C:\\T\\README.TXT", "c:\\t\\readme.txt", "C:\\T\\NOEXT",
    NULL
  };
  char detail[160];
  unsigned mode;
  int i, r, a, e;

  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("quickstat-host: skipped -- host is not LP64\n");
      return 77;
    }

  for (i = 0; yes[i]; i++)
    exec_case (yes[i], 1);
  for (i = 0; no[i]; i++)
    exec_case (no[i], 0);

  /* a directory keeps S_IFDIR | 0755 -- the exec bits of a directory are
     the search bits, not decided by the extension */
  r = qs_test_mode ("C:\\T\\SUB.NCP", &mode);
  snprintf (detail, sizeof detail, "stat=%d st_mode=0%o", r, mode);
  check (r == 0 && (mode & qs_test_s_ifmt ()) == qs_test_s_ifdir ()
	 && (mode & 0777) == 0755, "C:\\T\\SUB.NCP (directory) 0755", detail);

  /* read-only .NCP: FA_RDONLY clears the write bits, the extension still
     sets the exec bits */
  r = qs_test_mode ("C:\\T\\RO.NCP", &mode);
  a = qs_test_access ("C:\\T\\RO.NCP", qs_test_w_ok ());
  snprintf (detail, sizeof detail, "stat=%d st_mode=0%o access(W_OK)=%d", r, mode, a);
  check (r == 0 && (mode & 0777) == 0555 && a == -1,
	 "C:\\T\\RO.NCP read-only 0555", detail);

  /* a missing name is ENOENT, also with a native extension */
  qs_test_set_errno (0);
  a = qs_test_access ("C:\\T\\NONE.NCP", qs_test_f_ok ());
  e = qs_test_errno ();
  snprintf (detail, sizeof detail, "access(F_OK)=%d errno=%d", a, e);
  check (a == -1 && e == qs_test_enoent (), "C:\\T\\NONE.NCP missing -> ENOENT", detail);

  printf ("quickstat-host: %d of %d checks passed\n", checks - failed, checks);
  return failed;
}
