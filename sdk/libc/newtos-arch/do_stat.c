/*
 * do_stat.c -- NewTOS lean __sys_stat, replacing mintlib/do_stat.c.
 *
 * unix/open.c stats the path (via __quickstat -> __sys_stat) while opening a
 * file. mintlib's __sys_stat (mintlib/do_stat.c) tries Fstat64/Fxattr and then
 * converts the DOS timestamps with __unixtime -> localtime -> timezone/access
 * -> passwd/group DB, and pulls time() -> gettimeofday -> clock -> get_sysvar.
 * That is the same step-3 struct-stat tail that dragged the remaining
 * unresolved symbols (get_sysvar, getpwuid, getgrent, setgrent, endgrent).
 *
 * We keep the native fast path but drop the timestamp/owner conversion: make
 * the real GEMDOS Fstat64 trap (include/mint/mintbind.h) and return the
 * kernel's actual result -- no hardcoded value. If NewTOS does not implement
 * Fstat64 the trap returns -ENOSYS, and the callers (__quickstat in
 * mintlib/quickstat.c, _enoent) fall back to their own real GEMDOS Fsfirst
 * emulation, which fills st_mode/st_size honestly. So the actual path stat is
 * still performed by reused mintlib code; we only remove the time/owner tail.
 *
 * mintlib/do_stat.c also defined __do_stat (used only by unix/stat.c &
 * unix/lstat.c, neither of which the libtest link pulls), so we do not need to
 * provide it. Signature matches the __sys_stat that __quickstat/_enoent call:
 *   long __sys_stat(const char *, struct stat *, int lflag, int exact).
 */

#include <sys/types.h>
#include <sys/stat.h>
#include <mint/mintbind.h>   /* Fstat64 (native stat trap) */
#include "lib.h"

long
__sys_stat (const char *path, struct stat *st, int lflag, int exact)
{
	(void) exact;   /* NewTOS lean: no timestamp/owner resolution */
	return Fstat64 (lflag, path, st);
}
