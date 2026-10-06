/*
 * cmdargs.c -- NewTOS/AArch64 environment + command tail -> environ and
 * argc/argv for the C runtime: mintlib's crtinit.c `parseargs()`.
 *
 * mintlib builds `environ`, argc/argv and `program_invocation_name` in
 * mintlib/crtinit.c `parseargs()`, called from `_crtinit()`. That file does
 * not compile for AArch64 (m68k inline asm for the "Mshrink onto a new
 * stack" idiom, plus the LP64 basepage casts -- sdk/libc/expected-failures.txt,
 * docs/sdk/07-mintlib-limits.md Sec.1+5), and its `parseargs()` is `static`
 * there and sizes its arrays in 4-byte m68k pointers (`count += 4`). So
 * sdk/libc/crt0.S calls newtos-arch/heap_init.c `__libc_crtinit` instead, and
 * that calls this file. The algorithm below is `parseargs()`'s, step for step,
 * with AArch64 pointer widths; the OBSERVABLE contract is mintlib's, because
 * ported applications are written against it (TASK/PORT01, TASK/PORT09).
 *
 * THE ENVIRONMENT (crtinit.c:290-353). `p_env` points at the block the
 * launcher handed down -- since TASK/PPARENT every launch path has one, as
 * on EmuTOS (an AUTO program `{0, 0}`, the desktop's, the `#Z` program's and
 * the accessories' a copy of the AES's `"PATH=\0X:\\\0"`, a `Pexec` child
 * the caller's or the given block: docs/api/gemdos/process/_parent-and-
 * environment.md), at an address every process can read (ARCH03 stage 1,
 * VA = PA). Each NUL-terminated string up to the empty one becomes one
 * `environ` entry, pointing INTO the block (no copy). Two in-place rewrites,
 * both mintlib's:
 *
 *   - the desktop's "PATH=\0C:\\\0" form: a string without '=' that follows
 *     one ending in '=' is appended to it ("PATH=C:\\"), and the entry is
 *     dropped ("the desktop (and some shells) use the environment in the
 *     wrong way", crtinit.c:332-345);
 *   - the ARGV extension (references/freemint/doc/programmer/atari/ARGV.txt
 *     :95-142): a string beginning "ARGV=" ends the environment -- its 'A'
 *     is overwritten by NUL, so the block itself ends there for everybody
 *     who reads it later, e.g. a `Pexec(.., env = NULL)` child -- and the
 *     strings after it are the arguments, argv[0] first (the Pexec path
 *     name). As in mintlib, the command tail's length byte 127 is NOT
 *     required (STRICTLY_COMPATIBLE_WITH_STANDARD is off, crtinit.c:61-70:
 *     "some applications (gulam is a notable example) ... don't set the
 *     command byte to 127").
 *   - `ARGV=NULL:i,j,...` (ARGV-blank.txt:12-34): the listed argv entries
 *     are empty arguments (transferred as " "); each is cut to "" after
 *     argc is known. An index that is not a decimal number or not below
 *     argc ends the list (crtinit.c `bail_out`).
 *
 * THE COMMAND TAIL (crtinit.c:355-449), used when there is no ARGV=:
 *
 *   - argv[0] is "" -- the TOS "program name not available" convention;
 *     argc counts it, so argc >= 1 always.
 *   - leading blanks/tabs of the tail are skipped;
 *   - if the first remaining character is a single quote, the MagX quoting
 *     rules apply (quoted runs keep their blanks, '' is a literal quote);
 *     otherwise the tail is split on runs of blanks/tabs.
 *
 * `program_invocation_name` / `_short_name` (crtinit.c:451-462) become
 * argv[0] and its last path component when argv[0] is not empty, i.e. under
 * ARGV=. argv and environ are NULL-terminated; `__libc_argc` /
 * `__libc_argv` / `environ` carry the result for the mintlib members that
 * read them (posix/wordexp.c, mintlib/spawn.c, stdlib/getenv.c, ...).
 *
 * WHERE THE RESULT LIVES -- mintlib's place. parseargs() lays `environ[]`,
 * the split command line and `argv[]` out in the TPA just above the BSS and
 * returns the byte count; `_crtinit` adds it to the size it keeps
 * (crtinit.c:167-172, `m = parseargs(bp); m += tlen + dlen + blen +
 * sizeof(BASEPAGE)`), so the heap and the stack start above it. Here
 * heap_init.c hands in `area` (the 16-byte aligned `_end`) and `limit`
 * (`p_hitpa`) and adds the return value to its `m` the same way. Unlike
 * mintlib, the worst case is checked against `limit` before anything is
 * written, and -1 is returned when it does not fit (heap_init.c then stops
 * with mintlib's "insufficient memory"): the number of strings in the block
 * bounds both pointer arrays, and the tail needs at most PDCLSIZE + 2 chars.
 */

#include <stddef.h>
#include <string.h>
#include <mint/basepage.h>

/* sizeof(BASEPAGE.p_cmdlin) -- PDCLSIZE in tos-types/src/basepage.rs. */
#define PDCLSIZE 128

/* argv[0] ("") + at most PDCLSIZE/2 tail arguments + the NULL terminator: n
   one-character arguments separated by single blanks need 2n-1 <= PDCLSIZE-1
   characters, so n <= PDCLSIZE/2 = 64. */
#define ARGV_MAX (1 + (PDCLSIZE / 2) + 1)

/* The split command line in the area: at most PDCLSIZE-1 characters plus
   "\0\0", rounded up to whole pointers. */
#define TAIL_BYTES \
  ((PDCLSIZE + 2 + sizeof (char *) - 1) & ~(sizeof (char *) - 1))

/* mintlib's own globals: `environ` (mintlib/globals.c),
   `program_invocation_name` / `_short_name` (mintlib/program-name.c). */
extern char **environ;
extern char *program_invocation_name;
extern char *program_invocation_short_name;

/* The wire layout this file reads: p_env @ 0x2c and p_cmdlin, the last 128
   bytes of the 256-byte BasePage (tos-types/src/basepage.rs; mint/basepage.h
   pins the rest of the offsets on both ABIs). sdk/libc/tests/cmdargs-host.c
   builds its test BasePage from these numbers. */
_Static_assert (sizeof (((BASEPAGE *) 0)->p_cmdlin) == PDCLSIZE,
		"p_cmdlin is 128 bytes");
_Static_assert (__builtin_offsetof (BASEPAGE, p_cmdlin) == 0x80,
		"p_cmdlin @ 0x80");
_Static_assert (__builtin_offsetof (BASEPAGE, p_env) == 0x2c,
		"p_env @ 0x2c");

/*
 * The pair mintlib's own members expect (crtinit.c defines them upstream;
 * that file is not built here, so there is no duplicate definition).
 */
long __libc_argc = 1;
static char *__libc_argv_default[] = { "", NULL };
char **__libc_argv = __libc_argv_default;

/* crtinit.c's own local definitions -- not the full <ctype.h>. */
#define IS_SPACE(c) ((c) == ' ' || (c) == '\t')
#define IS_DIGIT(c) ((c) >= '0' && (c) <= '9')

/* Number of NUL-terminated strings in `env` before the empty one -- the
   environment, ARGV= and its arguments together; 0 for no block. */
static unsigned long
env_strings (const char *env)
{
  unsigned long n = 0;

  if (env == NULL)
    return 0;
  while (*env)
    {
      n++;
      while (*env++)
	;
    }
  return n;
}

/*
 * parseargs() proper, over an environment block `env` (NULL = none) and the
 * 128-byte command tail image `cmdlin` (length byte first). Lays environ[],
 * the split tail and argv[] out from `area` (pointer-aligned), publishes
 * environ / __libc_argc / __libc_argv / program_invocation_name, and returns
 * the number of bytes of `area` used, or -1 (nothing written, nothing
 * published) when the worst case would pass `limit`.
 */
long
__libc_parse_env_args (char *env, const char *cmdlin, char *area,
		       const char *limit)
{
  unsigned long nstr = env_strings (env);
  char line[PDCLSIZE + 2];
  char *from, *to, *cmdln;
  char **envp, **arg;
  char *null_list = NULL;
  long i;
  unsigned n;
  long argc;

  /* environ: at most nstr + 1, argv under ARGV=: at most nstr + 1 more,
     without ARGV=: the tail buffer and ARGV_MAX pointers. */
  if (area > limit
      || (unsigned long) (limit - area)
	 < (nstr + 2) * sizeof (char *) + TAIL_BYTES
	   + ARGV_MAX * sizeof (char *))
    return -1;

  /* handle the environment first */
  environ = envp = (char **) area;
  from = env;
  if (from)
    {
      while (*from)
	{
	  /* if we find MWC arguments, tie off environment here */
	  if (from[0] == 'A' && from[1] == 'R' && from[2] == 'G'
	      && from[3] == 'V' && from[4] == '=')
	    {
	      *envp++ = NULL;
	      *from = '\0';
	      from += 5;		/* skip ARGV= string */

	      /* find list of empty params */
	      if (from[0] == 'N' && from[1] == 'U' && from[2] == 'L'
		  && from[3] == 'L' && from[4] == ':')
		null_list = from + 5;

	      while (*from++)
		;			/* skip ARGV= value */
	      __libc_argv = arg = envp;
	      *arg++ = from;
	      while (*from++)
		;			/* skip argv[0] */
	      goto do_argc;
	    }

	  *envp++ = from;
	  {
	    /* flag to indicate desktop-style arg. passing */
	    int desktoparg = 1;

	    while (*from)
	      {
		if (*from == '=')
		  desktoparg = 0;
		from++;
	      }
	    from++;			/* skip 0 */

	    /* "PATH=\0C:\0" instead of "PATH=C:": a string without '='
	       after one ending in '=' is appended to that one. */
	    if (desktoparg && envp > &environ[1])
	      {
		char *p, *q;

		q = envp[-2];		/* current one is envp[-1] */
		while (*q)
		  q++;
		if (q[-1] == '=')
		  {
		    p = *--envp;
		    while (*p)
		      *q++ = *p++;
		    *q = '\0';
		  }
	      }
	  }
	}
    }
  *envp++ = NULL;

  /* The command tail. A private copy, so every pointer walk below stays
     inside our own storage even where the original reads one character
     past `i`. */
  for (n = 0; n < PDCLSIZE; n++)
    line[n] = cmdlin[n];
  line[PDCLSIZE] = '\0';
  line[PDCLSIZE + 1] = '\0';

  /* `i = *cmdln++` -- the length byte, unsigned, capped at what the buffer
     can actually hold. */
  i = (long) (unsigned char) line[0];
  if (i > PDCLSIZE - 1)
    i = PDCLSIZE - 1;
  cmdln = line + 1;

  /* room for the parsed command line, then argv after it */
  from = to = (char *) envp;
  envp = (char **) ((char *) envp + TAIL_BYTES);
  __libc_argv = arg = envp;
  *arg++ = "";			/* argv[0] not available */

  while (i > 0 && IS_SPACE (*cmdln))
    {
      cmdln++;
      --i;
    }

  /* MagXDesk only uses ARGV if the arg is longer than the 126 characters
     of p_cmdlin; a short argument containing blanks comes quoted. */
  if (*cmdln != '\'')
    {
      /* Plain split: a run of blanks/tabs ends the current argument. */
      while (i > 0)
	{
	  if (IS_SPACE (*cmdln))
	    {
	      --i;
	      cmdln++;
	      while (i > 0 && IS_SPACE (*cmdln))
		{
		  --i;
		  cmdln++;
		}
	      *to++ = '\0';
	    }
	  else
	    {
	      if ((*to++ = *cmdln++) == '\0')
		break;
	      --i;
	    }
	}
    }
  else
    {
      /* MagX quoting: 'a b' is one argument, '' inside a quoted run is a
         literal quote character. */
      int in_quote = 0;

      while (i > 0)
	{
	  if (*cmdln == '\'')
	    {
	      --i;
	      cmdln++;
	      if (!in_quote)
		in_quote = 1;
	      else if (*cmdln == '\'')
		{
		  *to++ = *cmdln++;
		  --i;
		}
	      else
		{
		  in_quote = 0;
		  *to++ = '\0';
		  --i;
		  cmdln++;
		}
	    }
	  else if (*cmdln == ' ' && !in_quote)
	    {
	      --i;
	      cmdln++;
	      *to++ = '\0';
	    }
	  else
	    {
	      *to++ = *cmdln++;
	      --i;
	    }
	}
    }

  /* Both loops write at most one byte per consumed character, and `i` is at
     most PDCLSIZE-1, so the two terminators fit TAIL_BYTES. The second NUL
     is what stops the collect loop when the tail ended without a
     separator. */
  *to++ = '\0';
  *to = '\0';

do_argc:
  /* Find out the name we have been invoked with. */
  if (__libc_argv[0] != NULL && __libc_argv[0][0] != '\0')
    {
      char *p;

      program_invocation_name = __libc_argv[0];
      p = strrchr (program_invocation_name, '\\');
      if (p == NULL)
	p = strrchr (program_invocation_name, '/');
      if (p != NULL)
	p++;
      program_invocation_short_name = p == NULL ? program_invocation_name : p;
    }

  argc = 1;			/* at this point argv[0] is done */
  while (*from)
    {
      *arg++ = from;
      argc++;
      while (*from++)
	;
    }
  *arg++ = NULL;
  __libc_argc = argc;

  /* zero empty params */
  if (null_list)
    {
      while (*null_list)
	{
	  char *s = null_list;
	  long idx = 0;

	  while (*++null_list)	/* find ',' or '\0' */
	    if (*null_list == ',')
	      {
		*null_list++ = '\0';
		break;
	      }

	  for (;;)
	    {
	      if (!IS_DIGIT (*s))
		goto bail_out;
	      idx += *s++ - '0';
	      if (*s)
		idx = (idx << 3) + (idx << 1);
	      else
		break;
	    }

	  if (idx < argc)
	    *(__libc_argv[idx]) = '\0';
	  else
	    goto bail_out;
	}
    }

bail_out:
  return (long) ((char *) arg - area);
}

/*
 * `parseargs(bp)` for the start-up code: the environment at `p_env` (a
 * 32-bit field in the wire BasePage, mint/basepage.h; 0 = none) and the
 * command tail `p_cmdlin`. Called once from newtos-arch/heap_init.c
 * `__libc_crtinit`, before the TPA is shrunk; see there for `area` and
 * `limit`.
 *
 * `basepage` is passed as an integer because on NewTOS a BasePage address is
 * an ordinary pool address that must not be mistaken for a pointer of
 * another kind.
 */
long
__libc_parseargs (unsigned long basepage, char *area, const char *limit)
{
  BASEPAGE *bp = (BASEPAGE *) basepage;

  return __libc_parse_env_args ((char *) (unsigned long) bp->p_env,
				bp->p_cmdlin, area, limit);
}
