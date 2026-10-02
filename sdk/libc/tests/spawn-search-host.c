/*
 * spawn-search-host.c -- host harness for the program-name search of the
 * libc overlays (TASK/PORT10, user decision 2026-10-02): mintlib/spawnvp.c's
 * extension list { ntp, nap, ncp } through mintlib's REAL _buffindfile
 * (mintlib/buffindf.c, unmodified), and posix/execvp.c's list
 * { "", .ntp, .nap, .ncp }.
 *
 * spawn-host.sh links: the spawnve.c + spawnvp.c overlays, mintlib's
 * buffindf.c, the posix/execvp.c overlay, spawn-search-fs.c (fake FAT
 * directory behind __quickstat/__execve, matched case-insensitively as
 * GEMDOS does) and this file. Only _spawnve, getenv and environ are renamed
 * into recorders here; the search itself is mintlib's code.
 *
 * Checked: a name without extension finds .NCP / .NTP / .NAP, in the order
 * ntp, nap, ncp; it does NOT find .PRG/.TOS/.TTP (nor .APP/.GTP, nor the
 * accessory .NAC); an explicit extension is used as given (also an m68k
 * one -- the search list does not gate what the caller names); the PATH walk
 * reaches the native names too. And the measured case of
 * measurements/measurement-2026-10-02-port10b: system("C:\P10\P10KID s3")
 * -> spawnvp("C:\P10\P10KID") now resolves to C:\P10\P10KID.ncp, which the
 * FAT entry P10KID.NCP matches.
 *
 * mintlib/spawn.c's copy of the list (the #! interpreter lookup) is not
 * driven here -- it is reached only after Pexec returns ENOEXEC for a
 * script; spawn-host.sh checks that the compiled overlay object carries the
 * native strings and none of the m68k ones.
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdio.h>
#include <string.h>

#define P_WAIT 0

extern int spawnvp (int, const char *, char *const *);
extern int execvp (const char *, char *const *);

/* spawn-search-fs.c */
extern char spawn_test_exec_found[];
extern int spawn_test_exec_tries;
int spawn_test_errno (void);
void spawn_test_set_errno (int);
int spawn_test_enoent (void);
int spawn_test_e2big (void);

/* -Denviron=spawn_test_environ */
static char *test_env_vec[] = { NULL };
char **spawn_test_environ = test_env_vec;

/* -Dgetenv=spawn_test_getenv: only PATH is asked for */
static const char *test_path;
char *
spawn_test_getenv (const char *tag)
{
  if (strcmp (tag, "PATH") == 0)
    return (char *) test_path;
  return NULL;
}

/* -D_spawnve=spawn_test__spawnve: what spawnvp resolved the name to */
static char rec_path[256];
static int rec_calls;
int
spawn_test__spawnve (int mode, const char *path, char *const *argv,
		     char *const *envp)
{
  int n = 0;

  (void) mode;
  (void) envp;
  rec_calls++;
  snprintf (rec_path, sizeof rec_path, "%s", path);
  while (argv[n])
    n++;
  return 40 + n;
}

static int failures, checks;

static void
verdict (int ok, const char *what, const char *got)
{
  checks++;
  if (!ok)
    failures++;
  printf ("%s  %s  [%s]\n", ok ? "PASS" : "FAIL", what, got);
}

/* spawnvp(P_WAIT, name) with PATH = path: expect `want' (NULL = ENOENT). */
static void
vp (const char *what, const char *path, const char *name, const char *want)
{
  char *av[] = { (char *) name, "s3", NULL };
  char got[300];
  int r;

  test_path = path;
  rec_calls = 0;
  rec_path[0] = '\0';
  spawn_test_set_errno (0);
  r = spawnvp (P_WAIT, name, av);
  if (want)
    {
      snprintf (got, sizeof got, "r=%d -> %s", r, rec_calls ? rec_path : "(no spawn)");
      verdict (r == 42 && rec_calls == 1 && strcmp (rec_path, want) == 0, what, got);
    }
  else
    {
      snprintf (got, sizeof got, "r=%d errno=%d%s%s", r, spawn_test_errno (),
		rec_calls ? " spawned " : "", rec_calls ? rec_path : "");
      verdict (r == -1 && rec_calls == 0 && spawn_test_errno () == spawn_test_enoent (),
	       what, got);
    }
}

/* execvp(name) with PATH = path: expect the name it found (NULL = ENOENT). */
static void
ep (const char *what, const char *path, const char *name, const char *want)
{
  char *av[] = { (char *) name, NULL };
  char got[300];
  int r;

  test_path = path;
  spawn_test_exec_found[0] = '\0';
  spawn_test_exec_tries = 0;
  spawn_test_set_errno (0);
  r = execvp (name, av);
  if (want)
    {
      snprintf (got, sizeof got, "found %s after %d tries",
		spawn_test_exec_found[0] ? spawn_test_exec_found : "nothing",
		spawn_test_exec_tries);
      verdict (r == -1 && spawn_test_errno () == spawn_test_e2big ()
	       && strcmp (spawn_test_exec_found, want) == 0, what, got);
    }
  else
    {
      snprintf (got, sizeof got, "r=%d errno=%d, %d tries%s%s", r,
		spawn_test_errno (), spawn_test_exec_tries,
		spawn_test_exec_found[0] ? ", found " : "", spawn_test_exec_found);
      verdict (r == -1 && spawn_test_errno () == spawn_test_enoent ()
	       && spawn_test_exec_found[0] == '\0', what, got);
    }
}

int
main (void)
{
  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("spawn-search-host: skipped -- host is not LP64\n");
      return 77;
    }

  /* spawnvp -> mintlib _buffindfile with { ntp, nap, ncp } */
  vp ("spawnvp C:\\P10\\P10KID finds P10KID.NCP (the port10b ENOENT case)",
      NULL, "C:\\P10\\P10KID", "C:\\P10\\P10KID.ncp");
  vp ("spawnvp C:\\BIN\\TOOL finds TOOL.NTP", NULL, "C:\\BIN\\TOOL",
      "C:\\BIN\\TOOL.ntp");
  vp ("spawnvp C:\\BIN\\GEMAPP finds GEMAPP.NAP", NULL, "C:\\BIN\\GEMAPP",
      "C:\\BIN\\GEMAPP.nap");
  vp ("order: TRIO.{NCP,NAP,NTP} -> .ntp first", NULL, "C:\\BIN\\TRIO",
      "C:\\BIN\\TRIO.ntp");
  vp ("order: DUO.{NCP,NAP} -> .nap before .ncp", NULL, "C:\\BIN\\DUO",
      "C:\\BIN\\DUO.nap");
  vp ("lower-case name finds the upper-case FAT entry", NULL, "c:\\p10\\p10kid",
      "c:\\p10\\p10kid.ncp");
  vp ("LEGACY.{PRG,TOS,TTP,APP,GTP} only -> not found (ENOENT)", NULL,
      "C:\\OLD\\LEGACY", NULL);
  vp ("ACC.NAC (accessory) -> not found (ENOENT)", NULL, "C:\\BIN\\ACC", NULL);
  vp ("MIXED.{PRG,NCP} -> the native .ncp", NULL, "C:\\OLD\\MIXED",
      "C:\\OLD\\MIXED.ncp");
  vp ("explicit extension .NCP is used as given", NULL, "C:\\P10\\P10KID.NCP",
      "C:\\P10\\P10KID.NCP");
  vp ("explicit extension .NTP is used as given", NULL, "C:\\BIN\\TOOL.NTP",
      "C:\\BIN\\TOOL.NTP");
  vp ("explicit m68k extension .PRG is still found as given", NULL,
      "C:\\OLD\\LEGACY.PRG", "C:\\OLD\\LEGACY.PRG");
  vp ("explicit extension that does not exist -> ENOENT, no list appended",
      NULL, "C:\\P10\\P10KID.NAP", NULL);
  vp ("PATH walk: TOOL via PATH=C:\\OLD,C:\\BIN finds C:\\BIN\\TOOL.ntp",
      "C:\\OLD,C:\\BIN", "TOOL", "C:\\BIN\\TOOL.ntp");
  vp ("PATH walk: LEGACY via PATH=C:\\OLD -> not found", "C:\\OLD", "LEGACY", NULL);

  /* execvp's own list { "", .ntp, .nap, .ncp } (__mint == 0) */
  ep ("execvp HERE (current directory) finds HERE.NCP", "", "HERE", "HERE.ncp");
  ep ("execvp XTOOL via PATH=\\XBIN finds XTOOL.NAP", "\\XBIN", "XTOOL",
      "\\XBIN/XTOOL.nap");
  ep ("execvp XOLD (only XOLD.PRG) -> ENOENT", "\\XBIN", "XOLD", NULL);
  ep ("execvp explicit XOLD.PRG via PATH is tried as given", "\\XBIN",
      "XOLD.PRG", "\\XBIN/XOLD.PRG");

  printf ("spawn-search-host: %d of %d checks failed\n", failures, checks);
  return failures;
}
