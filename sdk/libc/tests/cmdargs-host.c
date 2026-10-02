/*
 * cmdargs-host.c -- host harness for sdk/libc/newtos-arch/cmdargs.c.
 *
 * `__libc_parseargs()` is plain C over a 128-byte BasePage command tail with
 * no OS dependency, so a host can run the same cases a NewTOS program would
 * (the pattern setjmp-host.c already uses). The cases are the contract
 * mintlib's crtinit.c `parseargs()` defines and ported applications are
 * written against -- see cmdargs.c's header (TASK/PORT01).
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

extern long __libc_parseargs (unsigned long basepage);

/* mintlib's `environ` as cmdargs.c sees it (cmdargs-host.sh compiles it with
   -Denviron=cmdargs_test_environ). Poisoned before every check that needs
   to see it published. */
char **cmdargs_test_environ = (char **) 1;
extern long __libc_argc;
extern char **__libc_argv;

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
  argc = __libc_parseargs ((unsigned long) (size_t) basepage);

  if (argc != n_want || __libc_argc != argc)
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

    argc = __libc_parseargs ((unsigned long) (size_t) basepage);
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
  __libc_parseargs ((unsigned long) (size_t) basepage);
  if (cmdargs_test_environ == (char **) 1 || cmdargs_test_environ == NULL
      || cmdargs_test_environ[0] != NULL)
    {
      failures++;
      printf ("  FAIL environ: not published as the empty environment\n");
    }
  else
    printf ("  ok   environ is the empty environment\n");

  /* The tail mintlib's _spawnve (spawn.c) hands Pexec: length byte 0x7f --
     the ARGV= signal -- then the blank-separated arguments, NUL-terminated;
     and a non-zero p_env, because _spawnve always builds an environment.
     That p_env is not mapped in a NewTOS child today (TASK/MEM03), so
     parseargs() must not touch it: the field is set to an address that
     faults on the host if it were read. */
  {
    static const char *const want[] = { "alpha", "beta", "gamma", NULL };
    unsigned char *tail = &basepage[P_CMDLIN];
    const char *s = "alpha beta gamma";
    unsigned int poison = 0x00000010u;	/* page 0: unmapped on the host */
    long argc;
    int k, bad = 0;

    memset (basepage, 0, sizeof basepage);
    tail[0] = 0x7f;
    memcpy (&tail[1], s, strlen (s) + 1);
    memcpy (&basepage[0x2c], &poison, sizeof poison);	/* p_env */
    cmdargs_test_environ = (char **) 1;
    argc = __libc_parseargs ((unsigned long) (size_t) basepage);
    if (argc != 4)
      bad = 1;
    for (k = 0; !bad && want[k]; k++)
      if (strcmp (__libc_argv[k + 1], want[k]) != 0)
	bad = 1;
    if (!bad && __libc_argv[4] != NULL)
      bad = 1;
    if (cmdargs_test_environ == (char **) 1 || cmdargs_test_environ == NULL
	|| cmdargs_test_environ[0] != NULL)
      bad = 1;
    if (bad)
      {
	failures++;
	printf ("  FAIL spawn tail (0x7f, p_env set): argc %ld\n", argc);
      }
    else
      printf ("  ok   spawn tail (0x7f, p_env set, not read)\n");
  }

  printf ("cmdargs-host: %s\n", failures ? "FAILED" : "all cases pass");
  return failures;
}
