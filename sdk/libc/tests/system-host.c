/*
 * system-host.c -- host harness for the system() overlay (TASK/PORT10):
 * sdk/libc/overlay/posix/system.c, mintlib's posix/system.c without its
 * POSIX branch (vfork/execve/waitpid), so system() is always mintlib's TOS
 * branch: split the line into words, redirect handle 0/1 for < > >>, then
 * spawnvp(P_WAIT, argv[0], argv).
 *
 * system-host.sh compiles the overlay with the archive's include path, but
 * with <mint/osbind.h> held back (its trap macros are `svc #1') and
 * system-host-gemdos.h forced in instead, which routes Fopen/Fcreate/Fdup/
 * Fforce/Fclose/Fseek to this file's recorders. spawnvp and _unx2dos are
 * renamed into recorders as well (-Dspawnvp=..., -D_unx2dos=...). The real
 * Pexec path runs on the target (sdk/examples/port10.c).
 *
 * The script additionally checks the overlay object's undefined symbols: no
 * vfork/execve/waitpid/sigaction/sigprocmask/access/getenv may be left.
 *
 * Not checked here, on purpose: mintlib's system_hack allocates argv with
 * argc slots and writes argv[i] = NULL at index argc when the line has no
 * redirection (posix/system.c, the malloc and the `argv[i] = (char *)0'
 * line) -- an upstream overrun this overlay keeps verbatim. The host's
 * malloc rounds that slot into its slack; no sanitizer is used for that
 * reason (see TASK/PORT10-STATUS-*.MD).
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define P_WAIT 0

extern int system (const char *);

/* mintlib's errno is a plain `extern int' (include/errno.h). */
int errno;

/* -D_ctype=system_test_ctype: mintlib's table comes from its own
 * mintlib/ctype.c, compiled by system-host.sh with the same rename. */

/* --- recorders ------------------------------------------------------------ */
static char trace[1024];            /* "call;call;..." in call order */
static void
log_call (const char *fmt, long a, long b, const char *s)
{
  char one[160];
  if (s)
    snprintf (one, sizeof one, fmt, s, a, b);
  else
    snprintf (one, sizeof one, fmt, a, b);
  strncat (trace, one, sizeof trace - strlen (trace) - 1);
  strncat (trace, ";", sizeof trace - strlen (trace) - 1);
}

/* the standard handles as Fforce leaves them; -1 = not redirected */
static long std_map[2];

/* knobs for the next system() call */
static long next_fopen, next_fcreate, next_spawn;
static long next_dup = 20;

long
system_test_Fopen (const char *path, int mode)
{
  log_call ("Fopen(%s,%ld)", mode, 0, path);
  return next_fopen;
}

long
system_test_Fcreate (const char *path, int attr)
{
  log_call ("Fcreate(%s,%ld)", attr, 0, path);
  return next_fcreate;
}

long
system_test_Fdup (int h)
{
  log_call ("Fdup(%ld)=%ld", h, next_dup, NULL);
  return next_dup++;
}

long
system_test_Fforce (int std, int h)
{
  log_call ("Fforce(%ld,%ld)", std, h, NULL);
  if (std == 0 || std == 1)
    std_map[std] = h;
  return 0;
}

long
system_test_Fclose (int h)
{
  log_call ("Fclose(%ld)", h, 0, NULL);
  return 0;
}

long
system_test_Fseek (long off, int h, int whence)
{
  char one[64];
  snprintf (one, sizeof one, "Fseek(%ld,%d,%d);", off, h, whence);
  strncat (trace, one, sizeof trace - strlen (trace) - 1);
  return 0;
}

/* -D_unx2dos=...: mark the conversion so the checks see it was applied */
int
system_test_unx2dos (const char *unx, char *dos, size_t len)
{
  snprintf (dos, len, "DOS<%s>", unx);
  return 0;
}

/* -Dspawnvp=...: what the TOS branch hands the spawn layer */
static int spawn_calls;
static int rec_mode;
static char rec_name[128];
static char rec_argv[16][64];
static int rec_argc;
static long rec_std0, rec_std1;     /* std_map[] at the time of the call */

int
system_test_spawnvp (int mode, const char *name, char *const *argv)
{
  spawn_calls++;
  rec_mode = mode;
  snprintf (rec_name, sizeof rec_name, "%s", name);
  for (rec_argc = 0; argv[rec_argc] && rec_argc < 16; rec_argc++)
    snprintf (rec_argv[rec_argc], sizeof rec_argv[0], "%s", argv[rec_argc]);
  rec_std0 = std_map[0];
  rec_std1 = std_map[1];
  log_call ("spawnvp(%s)", 0, 0, name);
  return (int) next_spawn;
}

/* --- checks ---------------------------------------------------------------- */
static int failures, checks;

static void
reset (long fopen_rc, long fcreate_rc, long spawn_rc)
{
  trace[0] = 0;
  std_map[0] = std_map[1] = -1;
  next_fopen = fopen_rc;
  next_fcreate = fcreate_rc;
  next_spawn = spawn_rc;
  next_dup = 20;
  spawn_calls = 0;
  rec_argc = -1;
  rec_name[0] = 0;
  rec_std0 = rec_std1 = -1;
}

/* argv: NULL-terminated expected words; NULL = spawnvp must not be called */
static void
expect (const char *what, int got, int want, const char *const *argv,
	long std0, long std1, const char *want_trace)
{
  int ok = got == want && strcmp (trace, want_trace) == 0;
  int i;

  checks++;
  if (argv)
    {
      ok = ok && spawn_calls == 1 && rec_mode == P_WAIT
	&& strcmp (rec_name, argv[0]) == 0 && rec_std0 == std0
	&& rec_std1 == std1;
      for (i = 0; ok && argv[i]; i++)
	ok = i < rec_argc && strcmp (rec_argv[i], argv[i]) == 0;
      ok = ok && i == rec_argc;
    }
  else
    ok = ok && spawn_calls == 0;
  if (!ok)
    {
      failures++;
      printf ("FAIL  %s\n      rc %d (want %d), spawnvp calls %d, argc %d, "
	      "std0/1 at spawn %ld/%ld\n      trace: %s\n      want:  %s\n",
	      what, got, want, spawn_calls, rec_argc, rec_std0, rec_std1,
	      trace, want_trace);
      for (i = 0; i < rec_argc; i++)
	printf ("      argv[%d] = [%s]\n", i, rec_argv[i]);
    }
  else
    printf ("PASS  %s\n", what);
}

int
main (void)
{
  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("system-host: skipped -- host is not LP64\n");
      return 77;
    }

  /* mintlib: system(NULL) reports "a command processor is available". */
  reset (0, 0, 0);
  expect ("system(NULL) == 1, nothing run", system (NULL), 1, NULL, -1, -1, "");

  {
    static const char *const w[] = { "P10KID.NCP", "a", "b", NULL };
    reset (0, 0, 42);
    expect ("plain words -> spawnvp(P_WAIT), child's code returned",
	    system ("P10KID.NCP a b"), 42, w, -1, -1, "spawnvp(P10KID.NCP);");
  }
  {
    static const char *const w[] = { "KID", "a b", "c", "xy z", NULL };
    reset (0, 0, 0);
    /* mintlib's _parseargs: a quoted word groups up to the same quote and
     * loses the quotes; a quote inside a word continues that word. */
    expect ("quotes group words and are stripped",
	    system ("KID 'a b' \"c\" x'y z'"), 0, w, -1, -1, "spawnvp(KID);");
  }
  {
    static const char *const w[] = { "KID", "x", NULL };
    reset (0, 6, 3);
    expect ("> file: Fcreate + Fdup(1) + Fforce(1) around the spawn",
	    system ("KID x > out.txt"), 3, w, -1, 6,
	    "Fcreate(DOS<out.txt>,0);Fdup(1)=20;Fforce(1,6);spawnvp(KID);"
	    "Fforce(1,20);Fclose(20);Fclose(6);");
  }
  {
    static const char *const w[] = { "KID", NULL };
    reset (7, 99, 0);
    expect (">> file that exists: Fopen(2) + Fseek to the end",
	    system ("KID >>log.txt"), 0, w, -1, 7,
	    "Fopen(DOS<log.txt>,2);Fseek(0,7,2);Fdup(1)=20;Fforce(1,7);"
	    "spawnvp(KID);Fforce(1,20);Fclose(20);Fclose(7);");
  }
  {
    static const char *const w[] = { "KID", NULL };
    reset (-33, 8, 0);
    expect (">> file that does not exist: Fopen fails -> Fcreate",
	    system ("KID >>log.txt"), 0, w, -1, 8,
	    "Fopen(DOS<log.txt>,2);Fcreate(DOS<log.txt>,0);Fdup(1)=20;"
	    "Fforce(1,8);spawnvp(KID);Fforce(1,20);Fclose(20);Fclose(8);");
  }
  {
    static const char *const w[] = { "KID", NULL };
    reset (9, 0, 0);
    expect ("< file: Fopen(0) + Fdup(0) + Fforce(0)",
	    system ("KID < in.txt"), 0, w, 9, -1,
	    "Fopen(DOS<in.txt>,0);Fdup(0)=20;Fforce(0,9);spawnvp(KID);"
	    "Fforce(0,20);Fclose(20);Fclose(9);");
  }
  {
    /* doomgeneric i_system.c ZenityAvailable(): the program is not found,
     * spawnvp says -1, system() passes it on -> "zenity not available". */
    static const char *const w[] = { "/usr/bin/zenity", "--help", "2>&1", NULL };
    reset (0, 6, -1);
    expect ("DOOM's zenity probe: not found -> -1, handle 1 restored",
	    system ("/usr/bin/zenity --help >/dev/null 2>&1"), -1, w, -1, 6,
	    "Fcreate(DOS</dev/null>,0);Fdup(1)=20;Fforce(1,6);"
	    "spawnvp(/usr/bin/zenity);Fforce(1,20);Fclose(20);Fclose(6);");
  }
  {
    reset (0, -36, 0);
    printf ("      (the next check makes mintlib's perror print one line)\n");
    fflush (stdout);
    expect ("> file that cannot be created: 2, nothing run",
	    system ("KID >C:\\NOPE\\X.TXT"), 2, NULL, -1, -1,
	    "Fcreate(DOS<C:\\NOPE\\X.TXT>,0);");
  }

  printf ("system-host: %d of %d checks failed\n", failures, checks);
  return failures;
}
