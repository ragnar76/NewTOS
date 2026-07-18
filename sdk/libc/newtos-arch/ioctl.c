/*
 * ioctl.c -- NewTOS thin ioctl, replacing mintlib/unix/ioctl.c.
 *
 * mintlib/unix/ioctl.c is m68k Super()-mode assembly and does not assemble for
 * AArch64. On MiNT/TOS ioctl and Fcntl share the request space -- ioctl(fd,
 * request, arg) is Fcntl(fd, arg, request) -- so we forward to the GEMDOS
 * Fcntl trap (include/mint/mintbind.h) via the retargeted trap layer. This is
 * a real trap call, not a stub: the kernel returns the actual result.
 *
 * The only reachable reference from the libtest link is unix/open.c's
 * controlling-tty setup (ioctl(fd, TIOCSCTTY, 0)), which is guarded by
 * isatty(fd) and therefore dead for a regular file; the forward keeps it
 * honest for the general case. Prototype matches include/sys/ioctl.h:
 *   extern int ioctl (int, int, void *).
 */

#include <sys/ioctl.h>
#include <mint/mintbind.h>   /* Fcntl */
#include "lib.h"

int
ioctl (int fd, int request, void *arg)
{
	return (int) Fcntl (fd, arg, request);
}
