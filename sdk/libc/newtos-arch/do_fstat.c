/*
 * do_fstat.c -- NewTOS lean __do_fstat, replacing mintlib/do_fstat.c.
 *
 * mintlib's __do_fstat fills a full Unix `struct stat`: it resolves the file
 * times (__unixtime -> localtime -> timezone/200Hz clock via get_sysvar), the
 * owner/group (getuid/getgid -> passwd/group DB), and the tty flags (isatty ->
 * ioctl). None of that is available (or meaningful) on NewTOS, and it is
 * exactly the tail that dragged the 6 unresolved symbols
 * (get_sysvar, getpwuid, getgrent, setgrent, endgrent, ioctl).
 *
 * stdio's stream initialiser (stdio/stdio_init.c __stdio_init_stream) only
 * reads st_mode (S_ISCHR test, then __isatty) and st_blksize (buffer sizing).
 * unix/fstat.c calls __do_fstat(fd, st, 1) for a real fstat(). So we fill only:
 *
 *   st_mode    S_IFCHR|0600 for the console fds 0,1,2; S_IFREG|0644 otherwise.
 *   st_size    for a regular file, the HONEST size via GEMDOS Fseek to SEEK_END
 *              with the original position restored by a second Fseek; 0 for the
 *              console.
 *   st_blksize BUFSIZ, so stdio chooses full buffering.
 *   st_blocks  derived from st_size (512-byte units).
 *   st_nlink   1.
 *
 * Times (st_atime/mtime/ctime) and owner (st_uid/st_gid) are left zeroed --
 * NOT resolved. Their resolution is the whole step-3 struct-stat tail; we do
 * the MINIMUM libtest/stdio need. Matches the mintlib `struct stat` layout
 * (include/sys/stat.h + include/bits/stat.h) via <sys/stat.h>, and the exact
 * signature stdio_init.c calls: int __do_fstat(int, struct stat *, int).
 */

#include <string.h>       /* memset */
#include <stdio.h>        /* BUFSIZ, SEEK_SET/CUR/END */
#include <sys/types.h>
#include <sys/stat.h>     /* struct stat, S_IFCHR, S_IFREG, S_BLKSIZE */
#include <mint/osbind.h>  /* Fseek (GEMDOS 0x42) */
#include "lib.h"

int
__do_fstat (int fd, struct stat *st, int exact)
{
	(void) exact;   /* NewTOS lean: no time/owner resolution (step-3 tail) */

	memset (st, 0, sizeof (*st));

	if (fd >= 0 && fd <= 2) {
		/* Console handles: character device, no size. */
		st->st_mode = S_IFCHR | 0600;
		st->st_size = 0;
	} else {
		/* Regular file: honest size via Fseek(end), restore position. */
		long cur, end;

		st->st_mode = S_IFREG | 0644;
		cur = Fseek (0L, fd, SEEK_CUR);     /* remember where we are */
		end = Fseek (0L, fd, SEEK_END);     /* size == end offset */
		st->st_size = (end >= 0) ? (off_t) end : 0;
		if (cur >= 0)
			(void) Fseek (cur, fd, SEEK_SET);   /* restore */
	}

	st->st_blksize = BUFSIZ;
	st->st_blocks  = (st->st_size + S_BLKSIZE - 1) / S_BLKSIZE;
	st->st_nlink   = 1;

	return 0;
}
