/*
 * isatty.c -- NewTOS lean isatty, replacing mintlib/isatty.c.
 *
 * mintlib's __isatty probes the handle with Fcntl(TIOCGPGRP) and a seek trick,
 * pulling the ioctl/tty machinery. On NewTOS the console is fds 0,1,2 and
 * everything else is a file, so isatty is a fixed fd-range test.
 *
 * mintlib/isatty.c also *defined* the file-handle status table __open_stat[],
 * which is referenced by open/close/dup/dup2/pipe/read/write/main; since we
 * exclude that file, we provide the storage here (zero-initialised, exactly as
 * mintlib left it before any handle is classified).
 *
 * Public symbols match mintlib/isatty.c: __isatty and its weak alias isatty.
 */

#include <unistd.h>
#include <mint/mintbind.h>
#include "lib.h"

/* Handle status table; indexed by __OPEN_INDEX(fd) (see mintlib/lib.h). */
struct __open_file __open_stat[__NHANDLES];

int
__isatty (int fd)
{
	return (fd >= 0 && fd <= 2) ? 1 : 0;
}
weak_alias (__isatty, isatty)
