/*
 * spawn-host.c -- host harness for the LP64 spawn* overlays (TASK/PORT10):
 * sdk/libc/overlay/mintlib/spawnve.c (spawnl, spawnle, spawnv, spawnve) and
 * spawnvp.c (spawnlp, spawnvp).
 *
 * The m68k originals cast the va_list to `char **' and handed the caller's
 * stacked argument list straight to _spawnve; the overlays collect the same
 * arguments with va_arg. What reaches _spawnve must be exactly what the m68k
 * cast produced: argv = the variadic arguments up to and including the NULL,
 * envp = `environ' for the l/v forms, and for spawnle the pointer that follows
 * the NULL. spawn-host.sh compiles the two overlay files with the archive's
 * include path and renames the four outside symbols they call into this
 * file's recorders (-D_spawnve=..., -D_buffindfile=..., -Dgetenv=...,
 * -Denviron=...), so nothing here touches mintlib's real Pexec path -- the
 * target run (sdk/examples/port10.c) does that.
 *
 * Limit: on an Apple arm64 host every variadic argument is passed on the
 * stack (Darwin's deviation from AAPCS64), so this harness proves the
 * collection logic -- order, the NULL, the pointer after it, counts beyond
 * eight -- but not the AAPCS64 register-save-area walk. That half is the
 * target's: sdk/examples/port10.c runs spawnl/spawnle/spawnlp, including one
 * call with more arguments than argument registers, on NewTOS.
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdio.h>
#include <string.h>

#define P_WAIT 0

extern int spawnl (int, const char *, ...);
extern int spawnle (int, const char *, ...);
extern int spawnlp (int, const char *, ...);
extern int spawnv (int, const char *, char *const *);
extern int spawnvp (int, const char *, char *const *);

/* -Denviron=spawn_test_environ */
static char *test_env_vec[] = { "PATH=C:\\", NULL };
char **spawn_test_environ = test_env_vec;

/* What the last _spawnve call received. */
static int rec_mode;
static const char *rec_path;
static char *rec_argv[64];
static int rec_argc;
static char *const *rec_envp;

int
spawn_test__spawnve (int mode, const char *path, char *const *argv,
		     char *const *envp)
{
  rec_mode = mode;
  rec_path = path;
  for (rec_argc = 0; argv[rec_argc] && rec_argc < 63; rec_argc++)
    rec_argv[rec_argc] = argv[rec_argc];
  rec_argv[rec_argc] = argv[rec_argc];
  rec_envp = envp;
  return 40 + rec_argc;
}

/* spawnvp's search: hand the name back unchanged (found as given). */
char *
spawn_test__buffindfile (const char *fname, const char *fpath,
			 const char *const *fext, char *try)
{
  (void) fpath;
  (void) fext;
  strcpy (try, fname);
  return try;
}

char *
spawn_test_getenv (const char *tag)
{
  (void) tag;
  return NULL;
}

static int failures, checks;

static void
expect (const char *what, int r, int mode, const char *path,
	const char *const *argv, char *const *envp)
{
  int ok = 1, i;

  checks++;
  if (r != 40 + rec_argc || rec_mode != mode || strcmp (rec_path, path) != 0
      || rec_envp != envp)
    ok = 0;
  for (i = 0; ok && argv[i]; i++)
    if (i >= rec_argc || strcmp (rec_argv[i], argv[i]) != 0)
      ok = 0;
  if (ok && (i != rec_argc || rec_argv[i] != NULL))
    ok = 0;
  if (!ok)
    failures++;
  printf ("%s  %s (argc %d, envp %s)\n", ok ? "PASS" : "FAIL", what, rec_argc,
	  rec_envp == spawn_test_environ ? "environ"
	  : rec_envp == NULL ? "NULL" : "explicit");
}

int
main (void)
{
  static char *myenv[] = { "FOO=bar", NULL };
  static const char *const w_none[] = { "KID", NULL };
  static const char *const w_three[] = { "KID", "a", "b c", NULL };
  static const char *const w_many[] = { "0", "1", "2", "3", "4", "5", "6",
    "7", "8", "9", "10", "11", NULL };
  char *v_three[] = { "KID", "a", "b c", NULL };

  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("spawn-host: skipped -- host is not LP64\n");
      return 77;
    }

  expect ("spawnl, no arguments",
	  spawnl (P_WAIT, "C:\\KID.NCP", "KID", (char *) NULL),
	  P_WAIT, "C:\\KID.NCP", w_none, spawn_test_environ);
  expect ("spawnl, three arguments",
	  spawnl (P_WAIT, "C:\\KID.NCP", "KID", "a", "b c", (char *) NULL),
	  P_WAIT, "C:\\KID.NCP", w_three, spawn_test_environ);
  /* More than the eight argument registers: the tail comes off the stack. */
  expect ("spawnl, twelve arguments (registers + stack)",
	  spawnl (P_WAIT, "C:\\KID.NCP", "0", "1", "2", "3", "4", "5", "6",
		  "7", "8", "9", "10", "11", (char *) NULL),
	  P_WAIT, "C:\\KID.NCP", w_many, spawn_test_environ);
  expect ("spawnle, environment after the NULL",
	  spawnle (P_WAIT, "C:\\KID.NCP", "KID", "a", "b c", (char *) NULL, myenv),
	  P_WAIT, "C:\\KID.NCP", w_three, myenv);
  expect ("spawnle, twelve arguments then the environment",
	  spawnle (P_WAIT, "C:\\KID.NCP", "0", "1", "2", "3", "4", "5", "6",
		   "7", "8", "9", "10", "11", (char *) NULL, myenv),
	  P_WAIT, "C:\\KID.NCP", w_many, myenv);
  expect ("spawnlp -> spawnvp -> spawnve(NULL env)",
	  spawnlp (P_WAIT, "C:\\KID.NCP", "KID", "a", "b c", (char *) NULL),
	  P_WAIT, "C:\\KID.NCP", w_three, NULL);
  expect ("spawnv", spawnv (P_WAIT, "C:\\KID.NCP", v_three),
	  P_WAIT, "C:\\KID.NCP", w_three, spawn_test_environ);
  expect ("spawnvp", spawnvp (P_WAIT, "C:\\KID.NCP", v_three),
	  P_WAIT, "C:\\KID.NCP", w_three, NULL);

  printf ("spawn-host: %d of %d checks failed\n", failures, checks);
  return failures;
}
