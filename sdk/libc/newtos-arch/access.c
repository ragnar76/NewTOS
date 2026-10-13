/*
 * access.c -- NewTOS lean access(), replacing mintlib/unix/access.c.
 *
 * mintlib's access() does a full Unix permission check: after __quickstat it
 * compares the requested mode against owner / group / supplementary-group
 * bits, and the group branch calls getgroups(), which on a fall-through pulls
 * the Unix passwd/group database (getpwuid + setgrent/getgrent/endgrent). None
 * of that has a TOS analog -- there is no /etc/passwd or /etc/group -- and it
 * dragged in the last unresolved symbols when QED's timezone code (tz/
 * localtime.c) called access().
 *
 * On single-user TOS the current user owns every file: __quickstat sets
 * st_uid = geteuid() for every entry, and derives the owner rwx bits honestly
 * from the DOS attributes (FA_RDONLY -> clears the write bits, FA_HIDDEN ->
 * clears the read bits; executable extensions set the exec bits). So the owner
 * permission bits are the whole story, and we drop the group machinery. This
 * is the same lean-shim pattern as newtos-arch/{do_stat,ioctl}.c: a real
 * check over the real stat, with the Unix-only tail removed. No stub.
 */

#include <errno.h>
#include <fcntl.h>		/* F_OK, R_OK, W_OK, X_OK */
#include <unistd.h>
#include <sys/stat.h>
#include "lib.h"		/* __quickstat, __set_errno */
#include "libc-symbols.h"	/* weak_alias */

int
__access(const char *path, int imode)
{
	unsigned int mode = (unsigned int) imode;
	struct stat sb;

	if (__quickstat(path, &sb, 0) < 0)
		return -1;		/* errno set by __quickstat */

	if (mode == F_OK)
		return 0;		/* existence test succeeded */

	/* R_OK/W_OK/X_OK match the low 3 mode bits; check the owner triplet. */
	if (((sb.st_mode >> 6) & mode) == mode)
		return 0;

	__set_errno(EACCES);
	return -1;
}

weak_alias(__access, access)
