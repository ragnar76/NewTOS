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
 *   st_blksize NEWTOS_FILE_BLKSIZE (32 KiB) for a GEMDOS file handle
 *              (fd >= 6) whose size Fseek could read; BUFSIZ (1024) for
 *              everything else -- the console fds 0..2, the other standard
 *              handles 3..5 and negative (BIOS device) handles keep exactly
 *              the value they had before. See "STDIO BUFFER SIZE" below.
 *   st_blocks  derived from st_size (512-byte units).
 *   st_nlink   1.
 *
 * Times (st_atime/mtime/ctime) and owner (st_uid/st_gid) are left zeroed --
 * NOT resolved. Their resolution is the whole step-3 struct-stat tail; we do
 * the MINIMUM libtest/stdio need. Matches the mintlib `struct stat` layout
 * (include/sys/stat.h + include/bits/stat.h) via <sys/stat.h>, and the exact
 * signature stdio_init.c calls: int __do_fstat(int, struct stat *, int).
 *
 * STDIO BUFFER SIZE -- a named deviation from mintlib/TOS (TASK/FS08 part 4,
 * user decision 2026-10-07: "a larger buffer is no problem, we have more
 * memory than a real Atari"). mintlib's stdio sizes a stream's buffer from
 * st_blksize (stdio/stdio_init.c:72-77 -> internals.c init_stream malloc,
 * halving down to 128 bytes if that much is not free). mintlib's own TOS
 * branch reports 1024 (mintlib/do_fstat.c:150, quickstat.c:90/248), and so
 * did this shim (BUFSIZ). On the real Pi 4 that made every 1 KiB fread on a
 * file one Fread of 1 KiB, i.e. one SD command each: 1.35 MB/s against
 * 11.7 MB/s for one large Fread (measurements/measurement-2026-10-07-fs08-
 * pi-after/). A regular file now gets a 32 KiB buffer -- one Fread, one
 * multi-block command per 32 KiB. 32 KiB, not 64: measured in
 * measurements/measurement-2026-10-07-stdiobuf/ (16/32/64 KiB), the Pi model
 * gives ~84 % of whole-file throughput at 32 KiB (92 % at 64, 72 % at 16),
 * while every fseek costs a refill of up to one buffer -- DOOM, which reads
 * each lump with fseek + fread, read 9x / 18x the bytes at 32 / 64 KiB in the
 * same 90 s. Costs: 32 KiB of malloc per open FILE (freed at fclose; a
 * failing malloc halves it, internals.c:63-80), text-mode writes still go out
 * in <= 8 KiB chunks (sysd-stdio.c:205-214). Documented in
 * docs/sdk/07-mintlib-limits.md section 2. A program that wants another size
 * calls setvbuf(); console and device streams are unchanged.
 */

#include <string.h>       /* memset */
#include <stdio.h>        /* BUFSIZ, SEEK_SET/CUR/END */
#include <sys/types.h>
#include <sys/stat.h>     /* struct stat, S_IFCHR, S_IFREG, S_BLKSIZE */
#include <mint/osbind.h>  /* Fseek (GEMDOS 0x42) */
#include "lib.h"

/* First GEMDOS file handle: 0..5 are the standard handles (EmuTOS NUMSTD,
 * tos-api/src/bdos/fs/file.rs NUMSTD), negative handles are BIOS devices. */
#define NEWTOS_NUMSTD		6
/* stdio buffer size for a regular file (TASK/FS08 part 4, see above). */
#define NEWTOS_FILE_BLKSIZE	(32UL * 1024UL)

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
		if (fd >= NEWTOS_NUMSTD && end >= 0)
			st->st_blksize = NEWTOS_FILE_BLKSIZE;
	}

	if (st->st_blksize == 0)
		st->st_blksize = BUFSIZ;
	st->st_blocks  = (st->st_size + S_BLKSIZE - 1) / S_BLKSIZE;
	st->st_nlink   = 1;

	return 0;
}
