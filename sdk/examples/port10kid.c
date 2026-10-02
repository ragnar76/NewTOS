/*
 * port10kid.c -- the child that sdk/examples/port10.c starts through mintlib's
 * spawn* family (TASK/PORT10).
 *
 * Prints, over GEMDOS Cconws (serial + console), what the C runtime handed it:
 * argc/argv as sdk/libc/newtos-arch/cmdargs.c built them from the command tail
 * mintlib's _spawnve wrote (length byte 0x7f, the ARGV= signal, then the
 * arguments), whether `environ` is the empty environment, and the raw p_env
 * value the kernel stamped. p_env is printed, NEVER dereferenced: a child
 * started by Pexec from EL0 with an environment gets a physical address that
 * is not mapped in its EL0 space (TASK/MEM03), and _spawnve always passes one.
 * Its exit code is 40 + argc, so the parent can check that Pexec's return
 * value is the child's Pterm code.
 *
 * Build (links libc.a):
 *     sdk/libc/build-libc.sh sdk/examples/port10kid.c sdk/libc/build/P10KID.NCP
 * Not a stand-alone test; port10.c says where it must lie on C:.
 */
#include <mint/basepage.h>
#include <mint/osbind.h>

extern char **environ;

static void
put_dec (unsigned long n)
{
  char buf[24];
  int i = sizeof buf - 1;

  buf[i] = '\0';
  do
    buf[--i] = (char) ('0' + n % 10);
  while ((n /= 10) != 0);
  Cconws (&buf[i]);
}

static void
put_hex (unsigned long n)
{
  char buf[20];
  int i = sizeof buf - 1;

  buf[i] = '\0';
  do
    buf[--i] = "0123456789abcdef"[n & 15];
  while ((n >>= 4) != 0);
  Cconws ("0x");
  Cconws (&buf[i]);
}

int
main (int argc, char **argv)
{
  int i;

  Cconws ("p10kid: argc ");
  put_dec ((unsigned long) argc);
  Cconws ("\r\n");
  for (i = 0; i < argc; i++)
    {
      Cconws ("p10kid: argv[");
      put_dec ((unsigned long) i);
      Cconws ("] \"");
      Cconws (argv[i]);
      Cconws ("\"\r\n");
    }
  Cconws ("p10kid: cmdlin[0] ");
  put_dec ((unsigned char) _base->p_cmdlin[0]);
  Cconws (", environ ");
  Cconws (environ == 0 ? "NULL" : environ[0] == 0 ? "empty" : "non-empty");
  Cconws (", p_env ");
  put_hex (_base->p_env);
  Cconws (" (not read, TASK/MEM03)\r\n");
  return 40 + argc;
}
