/*
 * cmdargs-host.c -- host harness for sdk/libc/newtos-arch/cmdargs.c.
 *
 * `__libc_parseargs()` / `__libc_parse_env_args()` are plain C over an
 * environment block and a 128-byte BasePage command tail with no OS
 * dependency, so a host can run the same cases a NewTOS program would (the
 * pattern setjmp-host.c already uses). The cases are the contract mintlib's
 * crtinit.c `parseargs()` defines and ported applications are written
 * against -- see cmdargs.c's header (TASK/PORT01: the tail; TASK/PORT09:
 * environ and the ARGV= extension).
 *
 * The environment cases call `__libc_parse_env_args()` with a host pointer:
 * the BasePage's `p_env` is a 32-bit field, and a host's data lives above
 * 4 GiB (macOS/arm64), so only `p_env == 0` goes through the BasePage.
 *
 * This file uses the HOST's <stdio.h>/<string.h>, so it is compiled as its own
 * translation unit without mintlib's include path; cmdargs.c is compiled with
 * it. The two numbers that connect them -- the BasePage is 256 bytes and
 * `p_cmdlin` is its last 128 -- are pinned by _Static_asserts inside cmdargs.c
 * itself (and mint/basepage.h), so this harness can build the test BasePage
 * from raw bytes, which is also the wire form the kernel writes
 * (tos-api/src/bdos/process/loader.rs `populate_basepage`).
 *
 * Build + run: sdk/libc/tests/cmdargs-host.sh (which sdk/run-host-tests.sh
 * calls). Exit status: number of failed cases, 0 = all pass, 77 = skipped
 * (host not LP64 little-endian).
 */
#include <stdio.h>
#include <string.h>

extern long __libc_parseargs (unsigned long basepage, char *area,
			      const char *limit);
extern long __libc_parse_env_args (char *env, const char *cmdlin, char *area,
				   const char *limit);

/* mintlib's `environ` as cmdargs.c sees it (cmdargs-host.sh compiles it with
   -Denviron=cmdargs_test_environ). Poisoned before every check that needs
   to see it published. */
char **cmdargs_test_environ = (char **) 1;
extern long __libc_argc;
extern char **__libc_argv;

/* mintlib's program_invocation_name / _short_name (mintlib/program-name.c),
   renamed by cmdargs-host.sh like `environ`. */
char *cmdargs_test_pin = "unknown application";
char *cmdargs_test_pisn = "unknown application";

/* The TPA bytes above the BSS that parseargs lays its arrays out in
   (heap_init.c hands in the 16-byte aligned `_end`). */
static char area[16384] __attribute__ ((aligned (16)));
#define AREA_LIMIT (area + sizeof area)

/* BasePage wire layout (tos-types/src/basepage.rs). */
#define BASEPAGE_SIZE 256
#define P_CMDLIN      0x80
#define PDCLSIZE      128

static int failures;
static unsigned char basepage[BASEPAGE_SIZE];

/*
 * Lay the classic Atari command-tail image into the test BasePage, exactly as
 * the NewTOS desktop builds it (kernel/src/desk/main.rs `build_cmd_tail`) and
 * the loader copies it: length byte, the characters, a NUL, a trailing CR.
 */
static void
set_tail (const char *s)
{
  size_t n = strlen (s);
  unsigned char *tail = &basepage[P_CMDLIN];

  memset (basepage, 0, sizeof basepage);
  if (n > PDCLSIZE - 3)
    {
      printf ("  FAIL set_tail: tail of %zu bytes does not fit\n", n);
      failures++;
      return;
    }
  tail[0] = (unsigned char) n;
  memcpy (&tail[1], s, n);
  tail[1 + n] = '\0';
  tail[2 + n] = '\r';
}

/*
 * Run one case: `tail` is the command tail (without its length byte), `want`
 * the expected argv entries AFTER argv[0], NULL-terminated.
 */
static void
one (const char *name, const char *tail, const char *const *want)
{
  long argc;
  long n_want = 1;			/* argv[0] */
  long i;
  int ok = 1;

  while (want[n_want - 1] != NULL)
    n_want++;

  set_tail (tail);
  /* parseargs returns the bytes it used, argc is in __libc_argc */
  argc = __libc_parseargs ((unsigned long) (size_t) basepage, area, AREA_LIMIT) < 0
	 ? -1 : __libc_argc;

  if (argc != n_want)
    ok = 0;
  else if (__libc_argv[0] == NULL || __libc_argv[0][0] != '\0')
    ok = 0;				/* argv[0] must be "" */
  else if (__libc_argv[argc] != NULL)
    ok = 0;				/* must be NULL-terminated */
  else
    for (i = 1; i < argc; i++)
      if (__libc_argv[i] == NULL || strcmp (__libc_argv[i], want[i - 1]) != 0)
	{
	  ok = 0;
	  break;
	}

  if (ok)
    printf ("  ok   %s\n", name);
  else
    {
      failures++;
      printf ("  FAIL %s: tail \"%s\" -> argc %ld:", name, tail, argc);
      for (i = 0; i < argc && i < 80; i++)
	printf (" [%ld]=\"%s\"", i,
		__libc_argv[i] ? __libc_argv[i] : "(null)");
      printf ("\n       expected argc %ld:", n_want);
      for (i = 0; i + 1 < n_want; i++)
	printf (" [%ld]=\"%s\"", i + 1, want[i]);
      printf ("\n");
    }
}

/* ---- TASK/PORT09: environ and the ARGV= extension ---------------------- */

/* A writable copy of an environment block (parseargs rewrites it in place,
   as mintlib's does): `len` bytes including the terminating double NUL. */
static char envbuf[1024];

static char *
set_env (const char *img, size_t len)
{
  memset (envbuf, 0x55, sizeof envbuf);
  memcpy (envbuf, img, len);
  return envbuf;
}

/* A raw 128-byte tail image: length byte `len`, then `s` (NUL-terminated). */
static char tailbuf[PDCLSIZE];

static const char *
raw_tail (unsigned char len, const char *s)
{
  memset (tailbuf, 0, sizeof tailbuf);
  tailbuf[0] = (char) len;
  strncpy (&tailbuf[1], s, PDCLSIZE - 2);
  return tailbuf;
}

static int
vec_eq (char **got, const char *const *want)
{
  long k;

  if (got == NULL)
    return 0;
  for (k = 0; want[k]; k++)
    if (got[k] == NULL || strcmp (got[k], want[k]) != 0)
      return 0;
  return got[k] == NULL;
}

static void
show_vec (const char *what, char **v)
{
  long k;

  printf ("       %s:", what);
  for (k = 0; v && v[k] && k < 40; k++)
    printf (" [%ld]=\"%s\"", k, v[k]);
  printf ("\n");
}

/* Every array parseargs published must lie inside [area, area + used). */
static int
inside (char **v, long used)
{
  long k;

  for (k = 0;; k++)
    {
      if ((char *) &v[k] < area || (char *) &v[k + 1] > area + used)
	return 0;
      if (v[k] == NULL)
	return 1;
    }
}

/* One environment case: `want_env` / `want_argv` NULL-terminated (argv
   from argv[0] on); `want_pin` the expected program_invocation_name (NULL =
   unchanged from the poison). */
static void
env_one (const char *name, const char *img, size_t len, const char *cmdlin,
	 const char *const *want_env, const char *const *want_argv,
	 const char *want_pin, const char *want_pisn)
{
  char *env = set_env (img, len);
  long used;
  long n_argv = 0;
  int ok = 1;

  while (want_argv[n_argv])
    n_argv++;
  cmdargs_test_environ = (char **) 1;
  cmdargs_test_pin = cmdargs_test_pisn = "poison";
  __libc_argc = -7;
  used = __libc_parse_env_args (env, cmdlin, area, AREA_LIMIT);

  if (used <= 0)
    ok = 0;
  else if (!vec_eq (cmdargs_test_environ, want_env)
	   || !inside (cmdargs_test_environ, used))
    ok = 0;
  else if (__libc_argc != n_argv || !vec_eq (__libc_argv, want_argv))
    ok = 0;
  else if (!inside (__libc_argv, used))
    ok = 0;
  else if (strcmp (cmdargs_test_pin, want_pin ? want_pin : "poison") != 0
	   || strcmp (cmdargs_test_pisn, want_pisn ? want_pisn : "poison") != 0)
    ok = 0;

  if (ok)
    printf ("  ok   %s\n", name);
  else
    {
      failures++;
      printf ("  FAIL %s: used %ld, argc %ld, pin \"%s\" / \"%s\"\n", name,
	      used, __libc_argc, cmdargs_test_pin, cmdargs_test_pisn);
      if (used > 0)
	{
	  show_vec ("environ", cmdargs_test_environ);
	  show_vec ("argv", __libc_argv);
	}
    }
}

static void
env_cases (void)
{
  /* The PORT09 probe's block (measurement-2026-10-02-port09/method/envpar.c
     = TeraDesk's single-task launch, startprg.c:543-567/650-655): 14
     arguments, 196 characters joined, one containing blanks; the tail is
     127 + the first 125 characters. */
  static const char argv14[] =
    "PATH=C:\\\0"
    "ARGV=\0"
    "C:\\ARGVTEST.NAP\0"
    "alpha-one\0bravo-two\0charlie-three\0delta-four\0echo-five\0"
    "foxtrot-six\0golf-seven\0hotel-eight\0india-nine\0juliet-ten\0"
    "kilo-eleven\0lima-twelve\0mike thirteen has blanks\0"
    "november-fourteen-is-the-last-argument\0"
    "\0";
  static const char *const env_path[] = { "PATH=C:\\", NULL };
  static const char *const want14[] = {
    "C:\\ARGVTEST.NAP", "alpha-one", "bravo-two", "charlie-three",
    "delta-four", "echo-five", "foxtrot-six", "golf-seven", "hotel-eight",
    "india-nine", "juliet-ten", "kilo-eleven", "lima-twelve",
    "mike thirteen has blanks", "november-fourteen-is-the-last-argument",
    NULL };
  const char *t127 = raw_tail (127, "alpha-one bravo-two charlie-three "
			       "delta-four echo-five foxtrot-six golf-seven "
			       "hotel-eight india-nine juliet-ten kilo-el");

  env_one ("ARGV: 14 arguments (196 chars), tail 127", argv14, sizeof argv14,
	   t127, env_path, want14, "C:\\ARGVTEST.NAP", "ARGVTEST.NAP");

  /* The block is cut at ARGV= in place ('A' -> NUL), so whoever reads it
     later (a Pexec(.., env = NULL) child: EmuTOS alloc_env copies up to the
     double NUL) sees the environment without the arguments. */
  if (envbuf[9] != '\0' || envbuf[8] != '\0' || memcmp (envbuf, "PATH=C:\\", 8))
    {
      failures++;
      printf ("  FAIL ARGV: block not cut at \"ARGV=\"\n");
    }
  else
    printf ("  ok   ARGV: block cut at \"ARGV=\" (double NUL after PATH)\n");

  /* mintlib does not require the 127 length byte (STRICTLY_COMPATIBLE_WITH_
     STANDARD off, crtinit.c:61-70), and ARGV= may carry a value. */
  {
    static const char img[] =
      "HOME=C:\\\0ARGV=whatever\0D:\\SH.NTP\0-c\0echo hi\0\0";
    static const char *const env[] = { "HOME=C:\\", NULL };
    static const char *const want[] = { "D:\\SH.NTP", "-c", "echo hi", NULL };
    env_one ("ARGV without length byte 127 (gulam), with a value", img,
	     sizeof img, raw_tail (5, "-c ec"), env, want, "D:\\SH.NTP",
	     "SH.NTP");
  }

  /* ARGV-blank.txt:28-33, verbatim: "foo bar '' '' blah '' frob". */
  {
    static const char img[] =
      "ARGV=NULL:2,3,5\0foo\0bar\0 \0 \0blah\0 \0frob\0\0";
    static const char *const env[] = { NULL };
    static const char *const want[] =
      { "foo", "bar", "", "", "blah", "", "frob", NULL };
    env_one ("ARGV=NULL:2,3,5 (ARGV-blank.txt example)", img, sizeof img,
	     raw_tail (127, "bar '' '' blah '' frob"), env, want, "foo", "foo");
  }

  /* An index not below argc ends the list (crtinit.c `bail_out`); the ones
     before it are applied. A '/' path gives the short name too. */
  {
    static const char img[] = "X=1\0ARGV=NULL:1,9,2\0/bin/p\0 \0 \0\0";
    static const char *const env[] = { "X=1", NULL };
    static const char *const want[] = { "/bin/p", "", " ", NULL };
    env_one ("ARGV=NULL: index out of range stops the list", img, sizeof img,
	     raw_tail (127, ""), env, want, "/bin/p", "p");
  }

  /* mintlib's own _spawnve (mintlib/spawn.c:353-420, used by system() and
     spawnvp): ARGV= + argv[0] + the arguments, tail 0x7f + the blank-joined
     arguments. */
  {
    static const char img[] =
      "PATH=C:\\BIN\0ARGV=\0C:\\P10\\P10KID.NCP\0alpha\0beta\0gamma\0\0";
    static const char *const env[] = { "PATH=C:\\BIN", NULL };
    static const char *const want[] =
      { "C:\\P10\\P10KID.NCP", "alpha", "beta", "gamma", NULL };
    env_one ("mintlib spawn block (ARGV=, tail 0x7f)", img, sizeof img,
	     raw_tail (0x7f, "alpha beta gamma"), env, want,
	     "C:\\P10\\P10KID.NCP", "P10KID.NCP");
  }

  /* No ARGV=: every string is a variable, argv comes from the tail and
     argv[0] stays "" (program_invocation_name untouched). */
  {
    static const char img[] = "FOO=bar\0BAZ=1\0EMPTY=\0\0";
    static const char *const env[] = { "FOO=bar", "BAZ=1", "EMPTY=", NULL };
    static const char *const want[] = { "", "a", "b c", NULL };
    env_one ("no ARGV: environ + tail argv", img, sizeof img,
	     raw_tail (9, "'a' 'b c'"), env, want, NULL, NULL);
  }
  {
    static const char img[] = "FOO=bar\0\0";
    static const char *const env[] = { "FOO=bar", NULL };
    static const char *const want[] = { "", "x", NULL };
    env_one ("no ARGV, tail length 127 (not validated without ARGV=)", img,
	     sizeof img, raw_tail (127, "x"), env, want, NULL, NULL);
  }

  /* The desktop / AES form "PATH=\0C:\\\0" (EmuTOS default_env; every #Z,
     desktop and accessory start gets a copy) joins to "PATH=C:\\"
     (crtinit.c:332-345). */
  {
    static const char img[] = "PATH=\0C:\\\0\0";
    static const char *const env[] = { "PATH=C:\\", NULL };
    static const char *const want[] = { "", "C:\\README.TXT", NULL };
    env_one ("desktop PATH=\\0C:\\\\\\0 joined", img, sizeof img,
	     raw_tail (13, "C:\\README.TXT"), env, want, NULL, NULL);
  }

  /* An empty environment block (the AUTO folder's {0, 0}). */
  {
    static const char img[] = "\0\0";
    static const char *const env[] = { NULL };
    static const char *const want[] = { "", NULL };
    env_one ("empty environment block", img, sizeof img, raw_tail (0, ""),
	     env, want, NULL, NULL);
  }

  /* Not enough room below the limit: -1, nothing published. */
  {
    long used;

    set_env (argv14, sizeof argv14);
    cmdargs_test_environ = (char **) 1;
    used = __libc_parse_env_args (envbuf, t127, area, area + 64);
    if (used != -1 || cmdargs_test_environ != (char **) 1
	|| envbuf[9] != 'A')
      {
	failures++;
	printf ("  FAIL limit: used %ld\n", used);
      }
    else
      printf ("  ok   limit too small -> -1, nothing written\n");
  }
}

int
main (void)
{
  static const char *const none[] = { NULL };
  static const char *const full[] = { "C:\\README.TXT", NULL };
  static const char *const bare[] = { "README.TXT", NULL };
  static const char *const three[] = { "a", "bb", "ccc", NULL };
  static const char *const tabs[] = { "one", "two", NULL };
  static const char *const quoted[] = { "a b", "c", NULL };
  static const char *const dquote[] = { "it's", "x", NULL };
  static const char *const two_num[] = { "C:\\F.TXT", "42", NULL };
  unsigned long probe = 1;

  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("cmdargs-host: skipped -- host is not LP64\n");
      return 77;
    }
  if (*(const unsigned char *) &probe != 1)
    {
      printf ("cmdargs-host: skipped -- host is not little-endian\n");
      return 77;
    }

  printf ("cmdargs-host: __libc_parseargs over the BasePage command tail\n");

  /* No argument: what a plain double-click hands a program. */
  one ("empty tail", "", none);

  /* The two DESK19 measurements, the tails the kernel is known to deliver
     (measurements/measurement-2026-09-23-desk19-desk20): Parameter = Full path, and
     Parameter = File name. */
  one ("full path", "C:\\README.TXT", full);
  one ("file name", "README.TXT", bare);

  /* Separators: leading, trailing and repeated blanks collapse. */
  one ("blank runs", "  a  bb   ccc  ", three);
  one ("tabs", "\tone\t\ttwo", tabs);

  /* QED's second and third arguments are line/column numbers (src/main.c:176). */
  one ("name + line", "C:\\F.TXT 42", two_num);

  /* MagX quoting, the branch crtinit.c takes when the tail starts with '. */
  one ("quoted blank", "'a b' c", quoted);
  one ("doubled quote", "'it''s' x", dquote);

  /* The densest tail the 128-byte buffer allows: 64 one-character arguments
     in 127 characters -- what the argv array is sized for. Built here rather
     than through set_tail(), which keeps room for the NUL and the CR the
     desktop appends; a 127-character tail leaves room for the NUL only. */
  {
    static char words[64][2];
    const char *want[65];
    unsigned char *tail = &basepage[P_CMDLIN];
    int k, j = 1;
    long argc;

    memset (basepage, 0, sizeof basepage);
    for (k = 0; k < 64; k++)
      {
	if (k)
	  tail[j++] = ' ';
	tail[j++] = (unsigned char) ('a' + (k % 26));
	words[k][0] = (char) ('a' + (k % 26));
	words[k][1] = '\0';
	want[k] = words[k];
      }
    want[64] = NULL;
    tail[0] = (unsigned char) (j - 1);	/* 127 */
    tail[j] = '\0';

    (void) __libc_parseargs ((unsigned long) (size_t) basepage, area, AREA_LIMIT);
    argc = __libc_argc;
    if (argc != 65 || __libc_argv[65] != NULL)
      {
	failures++;
	printf ("  FAIL 64 arguments (127 chars): argc %ld (want 65), "
		"terminated %d\n", argc, __libc_argv[65] == NULL);
      }
    else
      {
	int bad = 0;
	for (k = 0; k < 64; k++)
	  if (strcmp (__libc_argv[k + 1], want[k]) != 0)
	    bad = 1;
	if (bad)
	  {
	    failures++;
	    printf ("  FAIL 64 arguments (127 chars): wrong strings\n");
	  }
	else
	  printf ("  ok   64 arguments (127 chars)\n");
      }
  }

  /* environ (TASK/PORT10): parseargs() always publishes it, and with no
     environment to walk it is the empty environment -- one NULL, never a
     NULL pointer (mintlib/spawn.c `_spawnve` indexes it unconditionally). */
  cmdargs_test_environ = (char **) 1;
  set_tail ("");
  __libc_parseargs ((unsigned long) (size_t) basepage, area, AREA_LIMIT);
  if (cmdargs_test_environ == (char **) 1 || cmdargs_test_environ == NULL
      || cmdargs_test_environ[0] != NULL)
    {
      failures++;
      printf ("  FAIL environ: not published as the empty environment\n");
    }
  else
    printf ("  ok   environ is the empty environment (p_env 0)\n");

  env_cases ();

  printf ("cmdargs-host: %s\n", failures ? "FAILED" : "all cases pass");
  return failures;
}
