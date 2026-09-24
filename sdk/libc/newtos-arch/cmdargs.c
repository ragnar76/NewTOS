/*
 * cmdargs.c -- NewTOS/AArch64 command tail -> argc/argv for the C runtime.
 *
 * A program started from the desktop with an argument -- a file dragged onto
 * its icon, or onto a desktop shortcut for it -- receives that argument in its
 * BasePage's command tail `p_cmdlin` (offset 0x80): a length byte followed by
 * that many characters (tos-types/src/basepage.rs,
 * tos-api/src/bdos/process/loader.rs `populate_basepage`; the desktop builds
 * the image in kernel/src/desk/main.rs `build_cmd_tail`).
 *
 * mintlib turns that tail into argc/argv in mintlib/crtinit.c `parseargs()`,
 * called from `_crtinit()`. Neither runs here: crtinit.c does not compile for
 * AArch64 (m68k inline asm for the "Mshrink onto a new stack" idiom, plus two
 * LP64 basepage casts -- sdk/libc/expected-failures.txt,
 * docs/sdk/07-mintlib-limits.md Sec.1+5), so sdk/libc/crt0.S took its place
 * and, until this file existed, called main() with a fixed
 * argc == 1 / argv == { "", NULL }. Every libc-linked program therefore saw
 * no arguments at all, whatever the launcher handed it -- which is why a file
 * dropped on QED started the editor empty (TASK/PORT01).
 *
 * This is the argument half of `parseargs()`, reimplemented for AArch64. The
 * OBSERVABLE contract is mintlib's, because ported applications are written
 * against it -- QED's src/main.c:147-190, for one, re-assembles a file name
 * containing blanks from several argv entries because that is what this libc
 * hands it ("MagXDesk liefert Namen mit Blanks nicht ueber ARGV sondern die
 * Kommandozeile"):
 *
 *   - argv[0] is "" -- the TOS "program name not available" convention
 *     (crtinit.c `*arg++ = ""`); argc counts it, so argc >= 1 always.
 *   - leading blanks/tabs of the tail are skipped;
 *   - if the first remaining character is a single quote, the MagX quoting
 *     rules apply (quoted runs keep their blanks, '' is a literal quote);
 *     otherwise the tail is split on runs of blanks/tabs;
 *   - argv is NULL-terminated, and `__libc_argc` / `__libc_argv` carry the
 *     same pair for the mintlib members that read them (posix/wordexp.c).
 *
 * What is NOT here, and why: the environment half. `parseargs()` also walks
 * `p_env`, publishes `environ`, and honours the MWC/Atari `ARGV=` extension,
 * under which a launcher too long for the 126-byte tail passes the arguments
 * in the environment instead. On NewTOS `p_env` is 0 on every launch path that
 * exists today -- the desktop, the AUTO folder and the boot launcher all pass
 * `EnvSpec::None` (kernel/src/launch.rs, tos-api/src/bdos/process/loader.rs),
 * so `environ` stays the zero global it is documented to be
 * (docs/sdk/07-mintlib-limits.md Sec.5) and no `ARGV=` block can reach a
 * program. When an environment is wired, this file is where the ARGV=
 * extension belongs.
 *
 * WHERE THE RESULT LIVES. mintlib puts the argv array and the split command
 * line in the gap just above the BSS (`p_bbase + p_blen + 4`) and then starts
 * the malloc arena above it (crtinit.c: `_heapbase = (void *)((long)bp + m)`,
 * with `m` including parseargs' byte count). That placement cannot be copied:
 * NewTOS's `__libc_init_heap` (newtos-arch/heap_init.c) starts the arena at
 * the linker's `_end`, i.e. exactly there, so argv would sit under the heap.
 * The strings and the pointer array live in this file's BSS instead. The
 * sizes are bounded, not guessed: `p_cmdlin` is 128 bytes, so the length byte
 * is at most 127 and both split loops write at most one byte per consumed
 * character -- 127 bytes plus the two terminating NULs. The densest possible
 * argument list is one-character arguments separated by single blanks, i.e.
 * 2n-1 <= 127 characters for n arguments, so at most 64 of them -- plus
 * argv[0] and the NULL terminator, 66 slots.
 */

#include <stddef.h>
#include <mint/basepage.h>

/* sizeof(BASEPAGE.p_cmdlin) -- PDCLSIZE in tos-types/src/basepage.rs. */
#define PDCLSIZE 128

/* The split command line: at most PDCLSIZE-1 characters plus "\0\0". */
static char arg_chars[PDCLSIZE + 2];

/* argv[0] ("") + at most PDCLSIZE/2 arguments + the NULL terminator: n
   one-character arguments separated by single blanks need 2n-1 <= PDCLSIZE-1
   characters, so n <= PDCLSIZE/2 = 64. */
#define ARGV_MAX (1 + (PDCLSIZE / 2) + 1)
static char *arg_vec[ARGV_MAX];

/* The wire layout this file reads: p_cmdlin is the last 128 bytes of the
   256-byte BasePage (tos-types/src/basepage.rs; mint/basepage.h pins the rest
   of the offsets on both ABIs). sdk/libc/tests/cmdargs-host.c builds its test
   BasePage from these two numbers. */
_Static_assert (sizeof (((BASEPAGE *) 0)->p_cmdlin) == PDCLSIZE,
		"p_cmdlin is 128 bytes");
_Static_assert (__builtin_offsetof (BASEPAGE, p_cmdlin) == 0x80,
		"p_cmdlin @ 0x80");

/*
 * The pair mintlib's own members expect (crtinit.c defines them upstream;
 * that file is not built here, so there is no duplicate definition).
 * `posix/wordexp.c` is the archive member that reads them.
 */
long __libc_argc = 1;
static char *__libc_argv_default[] = { "", NULL };
char **__libc_argv = __libc_argv_default;

/* crtinit.c's own local definition -- blank or tab, not the full <ctype.h>. */
#define IS_SPACE(c) ((c) == ' ' || (c) == '\t')

/*
 * Build argc/argv from `basepage`'s command tail, publish them in
 * `__libc_argc` / `__libc_argv`, and return argc. Called once from
 * sdk/libc/crt0.S, after __libc_init_heap and before main().
 *
 * `basepage` is passed as an integer because on NewTOS the BasePage VA is 0
 * (USER_VA_BASE) -- a perfectly valid address here, mapped by the loader, and
 * one that must not be mistaken for a null pointer.
 */
long
__libc_parseargs (unsigned long basepage)
{
  const BASEPAGE *bp = (const BASEPAGE *) basepage;
  /* A private copy, so every pointer walk below stays inside our own
     storage even where the original reads one character past `i`. */
  char line[PDCLSIZE + 2];
  const char *cmdln;
  const char *from;
  char *to;
  long i;
  unsigned n;
  unsigned argc;

  for (n = 0; n < PDCLSIZE; n++)
    line[n] = bp->p_cmdlin[n];
  line[PDCLSIZE] = '\0';
  line[PDCLSIZE + 1] = '\0';

  /* `i = *cmdln++` -- the length byte, unsigned, capped at what the buffer
     can actually hold. */
  i = (long) (unsigned char) line[0];
  if (i > PDCLSIZE - 1)
    i = PDCLSIZE - 1;
  cmdln = line + 1;

  from = to = arg_chars;

  while (i > 0 && IS_SPACE (*cmdln))
    {
      cmdln++;
      --i;
    }

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
     most PDCLSIZE-1, so `to` is at most arg_chars + PDCLSIZE - 1 here and the
     two terminators below fit. The second NUL is what stops the collect loop
     when the tail ended without a separator. */
  *to++ = '\0';
  *to = '\0';

  argc = 0;
  arg_vec[argc++] = "";		/* argv[0] not available */
  while (*from != '\0' && argc + 1 < ARGV_MAX)
    {
      arg_vec[argc++] = (char *) from;
      while (*from++)
	;
    }
  arg_vec[argc] = NULL;

  __libc_argv = arg_vec;
  __libc_argc = (long) argc;
  return __libc_argc;
}
