/*
 * argvtest.c -- print the argc/argv the C runtime built for this program.
 *
 * The GEM (`.NAP`) counterpart of examples/cmdline.c, and the difference
 * between the two is the whole point (TASK/PORT01):
 *
 *   cmdline.c  reads the BasePage command tail `p_cmdlin` ITSELF, links no
 *              libc, and therefore proved only that the KERNEL delivers the
 *              argument (DESK19, measurement-2026-09-23-desk19-desk20);
 *   argvtest.c goes through the SDK startup -- sdk/libc/crt0.S ->
 *              __libc_parseargs (sdk/libc/newtos-arch/cmdargs.c) -> main --
 *              exactly like every ported application (QED, TeraDesk), and so
 *              shows what the APPLICATION receives.
 *
 * Both halves are printed in one run, so a single serial log separates the two
 * possible faults: the raw tail out of the BasePage, then argc/argv.
 *
 * It is a `.NAP` on purpose: `*.NAP` is a GEM-application ANODE in the
 * desktop's built-in list (kernel/src/desk/inf.rs:678), so dropping a file on
 * its icon -- or on a desktop shortcut for it -- is a launch with that file as
 * the command tail, the same gesture that starts QED with a document.
 * It uses no AES call, so nothing but the startup stands between the launch
 * and the output.
 *
 * Output (GEMDOS Cconws, docs/api/gemdos/console/09-cconws.md -> serial AND
 * the framebuffer console):
 *
 *     ARGVTEST: cmdlin len 13 raw "C:\README.TXT"
 *     ARGVTEST: argc 2
 *     ARGVTEST: argv[0] ""
 *     ARGVTEST: argv[1] "C:\README.TXT"
 *
 * and then holds the screen until a key arrives (TASK/T13) -- it is reached
 * only by a double-click or a drop on its icon.
 *
 * Build (links libc.a, so it uses sdk/libc/crt0.S):
 *     sdk/libc/build-libc.sh sdk/examples/argvtest.c sdk/libc/build/argvtest.NAP
 * Installed as C:\ARGVTEST.NAP by tools/make-sd-image.sh.
 */
#include <mint/basepage.h>
#include <mint/osbind.h>

/* `_base` is the BasePage pointer sdk/libc/newtos-arch/heap_init.c stores at
   startup -- a real variable, not the literal address 0, so the read below is
   an honest pointer dereference. BASEPAGE itself is the LP64-corrected
   overlay struct (fixed 32-bit address fields, offsets pinned by
   _Static_assert in mint/basepage.h), i.e. the wire layout the kernel writes. */

static void
put_dec (unsigned long n)
{
  char buf[24];
  int i = sizeof buf - 1;

  buf[i] = '\0';
  do
    {
      buf[--i] = (char) ('0' + n % 10);
      n /= 10;
    }
  while (n);
  Cconws (&buf[i]);
}

/* Print `n` bytes verbatim; Cconws needs a NUL-terminated string, so this
   chunks through a small buffer rather than trusting the source. */
static void
put_bytes (const char *p, unsigned n)
{
  char buf[33];
  unsigned i = 0;

  while (i < n)
    {
      unsigned k = 0;

      while (k < sizeof buf - 1 && i < n)
	buf[k++] = p[i++];
      buf[k] = '\0';
      Cconws (buf);
    }
}

/* The TASK/T13 pause, hand-written rather than taken from the SDK header:
   this program is linked against mintlib, whose <mint/osbind.h> defines
   Cconws and Cconin as function-like MACROS, so <newtos.h> -- which declares
   both as static inline functions -- cannot be included beside it. Same two
   calls, same contract as newtos.h's newtos_pause(); keep the wording in step
   with it. ARGVTEST.NAP sits in the root of C: and is reached only by a
   double-click or a drop on its icon, never from \AUTO\. */
static void
pause_for_key (void)
{
  Cconws ("\r\n-- press a key --\r\n");
  Cconin ();
}

int
main (int argc, char *argv[])
{
  const BASEPAGE *bp = _base;
  unsigned len;
  int i;

  /* 1. What the KERNEL delivered: the raw command tail, length byte first. */
  Cconws ("ARGVTEST: cmdlin len ");
  len = (unsigned char) bp->p_cmdlin[0];
  put_dec (len);
  Cconws (" raw \"");
  put_bytes (&bp->p_cmdlin[1], len);
  Cconws ("\"\r\n");

  /* 2. What the STARTUP made of it. */
  Cconws ("ARGVTEST: argc ");
  put_dec ((unsigned long) argc);
  Cconws ("\r\n");

  for (i = 0; i < argc; i++)
    {
      Cconws ("ARGVTEST: argv[");
      put_dec ((unsigned long) i);
      Cconws ("] \"");
      Cconws (argv[i] ? argv[i] : "(null)");
      Cconws ("\"\r\n");
    }

  /* The terminator must be there, or argv is not a usable C vector. */
  Cconws (argv[argc] == 0 ? "ARGVTEST: argv terminated\r\n"
			  : "ARGVTEST: argv NOT terminated\r\n");

  pause_for_key ();
  return 0;
}
