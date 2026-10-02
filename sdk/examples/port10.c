/*
 * port10.c -- acceptance program for TASK/PORT10, as a NewTOS console
 * program linked against the ported mintlib.
 *
 *   1. printf("%f") & friends: __printf_fp printed wrong digits and could
 *      fault, because mintlib's gmp-mparam.h declared 32-bit GMP limbs while
 *      mp_limb_t is 64 bits on LP64 (fixed in sdk/libc/overlay/stdlib/
 *      gmp-mparam.h). Checked by exact string comparison through snprintf,
 *      and printed once through printf for the eye.
 *   2. intptr_t/uintptr_t (and their limits and PRI*PTR) are 64 bits on LP64
 *      (sdk/libc/overlay/include/{stdint,inttypes}.h).
 *   3. mintlib's spawn* family -- spawnv, spawnvp (what system()'s TOS
 *      emulation calls), spawnl, spawnlp, spawnle -- running a child through
 *      a synchronous Pexec(0), the way mintlib does on plain TOS without MiNT
 *      (sdk/libc/overlay/mintlib/spawn{ve,vp}.c compile on AArch64 now; the
 *      empty `environ` from sdk/libc/newtos-arch/cmdargs.c keeps _spawnve off
 *      address 0). The child is sdk/examples/port10kid.c; its exit code is
 *      40 + its argc, so each check proves both the arguments and that the
 *      spawn returns the child's Pterm code.
 *
 *   4. system() (since the sdk/libc/overlay/posix/system.c overlay: mintlib's
 *      TOS branch only, no vfork/execve/waitpid -- TASK/PORT10, removed again
 *      by TASK/PROC01): system(NULL) == 1; a .NCP run with arguments returns
 *      the child's Pterm code; "> file" redirects the child's handle 1 into
 *      a file the parent reads back; a name WITHOUT extension is looked up
 *      with spawnvp's NewTOS list (ntp, nap, ncp -- user decision
 *      2026-10-02, replacing mintlib's ttp, prg, tos), so C:\P10\P10KID
 *      finds P10KID.NTP/.NCP, C:\P10\ONLYNCP finds ONLYNCP.NCP, and
 *      C:\P10\LEGACY (only LEGACY.PRG on the card) is -1/ENOENT; and DOOM's
 *      I_Error probe "/usr/bin/zenity --help >/dev/null 2>&1" comes back
 *      non-zero. Two checks here measure KERNEL behaviour
 *      that system()'s redirection relies on, against EmuTOS: the child
 *      inherits the parent's forced standard handles (bdos/proc.c
 *      init_pd_files -> ixforce), and Fforce holds a use count so the parent's
 *      handle 1 survives Fclose of its Fdup'd copy (bdos/fshand.c ixforce).
 *
 * Build (links libc.a):
 *     sdk/libc/build-libc.sh sdk/examples/port10kid.c sdk/libc/build/P10KID.NCP
 *     sdk/libc/build-libc.sh sdk/examples/port10.c    sdk/libc/build/P10TEST.NCP
 * Card: C:\AUTO\P10TEST.NCP, plus the child as C:\P10\P10KID.NCP and the same
 * file again as C:\P10\P10KID.NTP, C:\P10\ONLYNCP.NCP and C:\P10\LEGACY.PRG
 * (measurements/measurement-2026-10-02-port10c/method/run-port10.sh).
 * Expect on the serial line: "port10: PASS ..." per check, then
 *     port10: ALL PASS (N checks)
 * No key pause: \AUTO\ is its route, and a key wait there stops the boot.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <inttypes.h>
#include <limits.h>
#include <errno.h>
#include <process.h>
#include <mint/osbind.h>

#define KID_NCP "C:\\P10\\P10KID.NCP"
#define KID_NTP "C:\\P10\\P10KID.NTP"

_Static_assert (sizeof (intptr_t) == sizeof (void *), "intptr_t is pointer-wide");
_Static_assert (sizeof (uintptr_t) == sizeof (void *), "uintptr_t is pointer-wide");

static unsigned checks, failed;

static void
verdict (int ok, const char *what, const char *detail)
{
  checks++;
  if (!ok)
    failed++;
  Cconws (ok ? "port10: PASS " : "port10: FAIL ");
  Cconws (what);
  if (detail)
    {
      Cconws (" -- ");
      Cconws (detail);
    }
  Cconws ("\r\n");
}

static void
fmt (const char *f, double v, const char *want)
{
  char got[160], what[200];

  snprintf (got, sizeof got, f, v);
  snprintf (what, sizeof what, "printf \"%s\" -> \"%s\"", f, got);
  verdict (strcmp (got, want) == 0, what, strcmp (got, want) ? want : NULL);
}

static void
spawned (const char *what, int r, int want)
{
  char d[80];

  snprintf (d, sizeof d, "returned %d, want %d (errno %d)", r, want,
	    r < 0 ? errno : 0);
  verdict (r == want, what, d);
}

int
main (void)
{
  Cconws ("port10: start\r\n");

  /* 1. float formatting (__printf_fp) and the integer/strtod paths that
     share the GMP limb width. */
  fmt ("%f", 1.5, "1.500000");
  fmt ("%f", 0.1, "0.100000");
  fmt ("%f", 3.14159, "3.141590");
  fmt ("%.3f", 79.123, "79.123");
  fmt ("%.2f", -2.5, "-2.50");
  fmt ("%e", 1e-5, "1.000000e-05");
  fmt ("%g", 123456789.0, "1.23457e+08");
  fmt ("%f", 1e100,
       "10000000000000000159028911097599180468360808563945281389781327557747838772170381060813469985856815104.000000");
  /* DOOM's timedemo line: "timed %i gametics in %i realtics (%f fps)". */
  {
    char got[120];
    const char *want = "timed 2134 gametics in 1000 realtics (74.690000 fps)";
    int gametics = 2134, realtics = 1000;
    snprintf (got, sizeof got, "timed %i gametics in %i realtics (%f fps)",
	      gametics, realtics, (double) (35 * gametics) / realtics);
    verdict (strcmp (got, want) == 0, got, strcmp (got, want) ? want : NULL);
  }
  {
    char got[60];
    snprintf (got, sizeof got, "%lld %llu %lx", -9876543210987LL,
	      18446744073709551615ULL, 0x123456789abcdefUL);
    verdict (strcmp (got, "-9876543210987 18446744073709551615 123456789abcdef") == 0,
	     "integers via _itoa", got);
  }
  {
    char *end;
    double d = strtod ("3.14159", &end);
    verdict (d == 3.14159 && *end == '\0', "strtod(\"3.14159\")", NULL);
  }
  printf ("port10: printf says %f and %g\r\n", 79.123, 1e-5);
  fflush (stdout);

  /* 2. pointer-wide integers. */
  {
    char got[40];
    uintptr_t u = (uintptr_t) (void *) 0x123456789000UL;
    intptr_t s = (intptr_t) (void *) -16L;
    verdict (u == 0x123456789000UL && s == -16L, "uintptr_t/intptr_t round-trip a 64-bit pointer", NULL);
    verdict (INTPTR_MAX == LONG_MAX && INTPTR_MIN == LONG_MIN
	     && UINTPTR_MAX == ULONG_MAX, "INTPTR_MIN/MAX, UINTPTR_MAX", NULL);
    snprintf (got, sizeof got, "%" PRIxPTR " %" PRIdPTR, (uintptr_t) 0x123456789abcUL,
	      (intptr_t) -5000000000L);
    verdict (strcmp (got, "123456789abc -5000000000") == 0, "PRIxPTR/PRIdPTR", got);
  }

  /* 3. spawn* -> synchronous Pexec(0); child exit code = 40 + its argc,
     argc counting argv[0] (always "" -- the tail carries no program name). */
  {
    char *av1[] = { "P10KID", "one", "two", NULL };
    spawned ("spawnv " KID_NCP " one two", spawnv (P_WAIT, KID_NCP, av1), 43);
  }
  {
    /* system("C:\P10\P10KID.NTP alpha")'s TOS branch, word for word:
       mintlib/system.c system_hack -> spawnvp(P_WAIT, argv[0], argv). */
    char *av2[] = { KID_NTP, "alpha", NULL };
    spawned ("spawnvp " KID_NTP " alpha", spawnvp (P_WAIT, av2[0], av2), 42);
  }
  spawned ("spawnl " KID_NCP " a b c",
	   spawnl (P_WAIT, KID_NCP, "P10KID", "a", "b", "c", (char *) NULL), 44);
  /* More arguments than AAPCS64 argument registers: va_arg walks from the
     general-register save area onto the caller's stack. */
  spawned ("spawnl " KID_NCP " 1..11 (registers + stack)",
	   spawnl (P_WAIT, KID_NCP, "P10KID", "1", "2", "3", "4", "5", "6",
		   "7", "8", "9", "10", "11", (char *) NULL), 52);
  spawned ("spawnlp " KID_NCP " (no arguments)",
	   spawnlp (P_WAIT, KID_NCP, "P10KID", (char *) NULL), 41);
  {
    char *env[] = { "FOO=bar", NULL };
    spawned ("spawnle " KID_NCP " e1 (explicit environment)",
	     spawnle (P_WAIT, KID_NCP, "P10KID", "e1", (char *) NULL, env), 42);
  }
  {
    char *av3[] = { "C:\\P10\\NOPE.NCP", NULL };
    int r;
    errno = 0;
    r = spawnvp (P_WAIT, av3[0], av3);
    verdict (r == -1 && errno == ENOENT, "spawnvp of a missing file -> -1/ENOENT", NULL);
  }

  /* 4. system(): mintlib's TOS branch (overlay posix/system.c) -> word split,
     redirection, spawnvp(P_WAIT) -> synchronous Pexec(0). */
  spawned ("system(NULL) (a command processor is available)", system (NULL), 1);
  spawned ("system(\"" KID_NCP " s1 s2\")",
	   system (KID_NCP " s1 s2"), 43);
  spawned ("system(\"" KID_NTP " t1\") (explicit extension)",
	   system (KID_NTP " t1"), 42);
  {
    /* The child writes a marker line through Fwrite(1) when its first
       argument is "fwrite"; system_hack has Fforce'd handle 1 to the file. */
    char line[80] = "";
    FILE *f;
    int r;
    Fdelete ("C:\\P10\\SYSOUT.TXT");
    r = system (KID_NCP " fwrite >C:\\P10\\SYSOUT.TXT");
    spawned ("system(\"" KID_NCP " fwrite >C:\\P10\\SYSOUT.TXT\")", r, 42);
    f = fopen ("C:\\P10\\SYSOUT.TXT", "r");
    if (f)
      {
	if (!fgets (line, sizeof line, f))
	  line[0] = 0;
	fclose (f);
      }
    verdict (f != NULL && strncmp (line, "p10kid: marker via handle 1", 27) == 0,
	     "> file: the child's handle 1 landed in C:\\P10\\SYSOUT.TXT",
	     f ? line : "file not there");
  }
  {
    /* system_hack restores handle 1 with Fforce(1, Fdup'd copy) and then
       Fcloses that copy; on EmuTOS the Fforce holds a use count
       (bdos/fshand.c ixforce), so handle 1 must still write afterwards. */
    static const char probe[] = "port10: handle 1 after the redirect\r\n";
    char d[60];
    long r = Fwrite (1, sizeof probe - 1, probe);
    snprintf (d, sizeof d, "Fwrite(1) returned %ld, want %ld", r,
	      (long) (sizeof probe - 1));
    verdict (r == (long) (sizeof probe - 1),
	     "handle 1 still writes after system(\"... >file\")", d);
  }
  {
    int r;
    errno = 0;
    r = system ("C:\\P10\\P10KID s3");
    spawned ("system(\"C:\\P10\\P10KID s3\") (no extension: spawnvp's "
	     "ntp/nap/ncp list)", r, 42);
  }
  spawned ("system(\"C:\\P10\\ONLYNCP s3 s4\") (no extension, only .NCP "
	   "on the card)", system ("C:\\P10\\ONLYNCP s3 s4"), 43);
  {
    int r;
    errno = 0;
    r = system ("C:\\P10\\LEGACY s5");
    verdict (r == -1 && errno == ENOENT,
	     "system(\"C:\\P10\\LEGACY s5\") -> -1/ENOENT: only LEGACY.PRG "
	     "exists, .prg is not in the native list", NULL);
  }
  {
    /* doomgeneric i_system.c ZenityAvailable(), on every I_Error. */
    char d[80];
    int r;
    errno = 0;
    r = system ("/usr/bin/zenity --help >/dev/null 2>&1");
    snprintf (d, sizeof d, "returned %d (errno %d)", r, errno);
    verdict (r != 0, "DOOM's zenity probe -> non-zero (zenity unavailable)", d);
  }

  if (failed == 0)
    {
      char line[60];
      snprintf (line, sizeof line, "port10: ALL PASS (%u checks)\r\n", checks);
      Cconws (line);
    }
  else
    {
      char line[60];
      snprintf (line, sizeof line, "port10: %u of %u checks FAILED\r\n", failed, checks);
      Cconws (line);
    }
  return failed ? 1 : 0;
}
