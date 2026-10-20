/*
 * do_stat.c -- NewTOS __sys_stat and __do_stat, replacing mintlib/do_stat.c.
 *
 * __sys_stat (the native stat probe, used by __quickstat, _enoent and
 * __do_stat): make the real GEMDOS Fstat64 trap (include/mint/mintbind.h)
 * and return the kernel's actual result -- no hardcoded value. NewTOS has no
 * Fstat64 (nor Fxattr): the GEMDOS dispatcher answers EINVFN (-32), which is
 * mintlib's -ENOSYS, and every caller then takes its own GEMDOS Fsfirst path,
 * as on a TOS without MiNT. mintlib's original additionally tried Fxattr,
 * which NewTOS answers with the same EINVFN; that second trap is dropped.
 *
 * __do_stat (stat() and lstat(), mintlib unix/stat.c, unix/lstat.c) is the
 * TOS branch of mintlib's own __do_stat (mintlib/do_stat.c:108-339),
 * including its timestamps (TASK/SDK01 F, docs/analysis/sdk-inventory-
 * 2026-10-20.md W4). Until SDK01 F it delegated to __quickstat, which takes
 * the identical Fsfirst path but "gets everything but the timestamps"
 * (mintlib/quickstat.c:26): st_atime/st_mtime/st_ctime were 0 for every
 * file. Now, as upstream on TOS:
 *
 *   a file or folder found by Fsfirst
 *              st_mtime = st_ctime = st_atime = __unixtime(dta_time,
 *              dta_date) -- the FAT entry's DOS time/date from the DTA
 *              (do_stat.c:281-282). __unixtime (mintlib/unixtime.c) reads
 *              the DOS stamp as LOCAL time and converts it with mktime, so
 *              the result is UTC seconds in the TZ the program sees -- the
 *              same mktime of Tgettime/Tgetdate that time() uses on a
 *              system without Tgettimeofday (unix/gettimeofday.c), so a
 *              fresh file's st_mtime and time() agree. With TZ unset and no
 *              /etc/localtime, mintlib's tz code falls back to UTC
 *              (tz/localtime.c zoneinit("")).
 *   the root of a drive ("C:", "C:\", "\"), and a "." / ".." / "dir\"
 *   name whose first Fsfirst hit is not the "." entry (a root)
 *              OLDDATE = __unixtime(0, 0) (do_stat.c:101, 187) -- the root
 *              has no directory entry and therefore no stamp.
 *   a device name ("CON:", "/dev/con"; _unx2dos returns 1)
 *              time(NULL) - 2 (do_stat.c:163).
 *
 * Everything else is __quickstat's walk unchanged (overlay
 * mintlib/quickstat.c, which is mintlib/quickstat.c = do_stat.c's TOS branch
 * minus the times), including NewTOS's executable extensions .ntp/.nap/.ncp
 * compared case-insensitively (user decision 2026-10-02, TASK/PORT10) in
 * place of mintlib's .app/.gtp/.ttp/.prg/.tos. Not carried over:
 * do_stat.c's `_x_Bit_set_in_stat' option (default 0; when set, it reads
 * the m68k magic 0x601A / "#!" from the file to set the exec bits) --
 * it was not in libc before SDK01 F either.
 *
 * Signatures as declared in mintlib/lib.h:154, 158.
 */

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <string.h>
#include <support.h>        /* __unixtime, _unx2dos */
#include <time.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <mint/mintbind.h>   /* Fstat64 (native stat trap), Fsfirst & co. */
#include "lib.h"

long
__sys_stat (const char *path, struct stat *st, int lflag, int exact)
{
	(void) exact;   /* selects only the Fxattr time conversion (dropped) */
	return Fstat64 (lflag, path, st);
}

/* date for files (like root directories) that don't have one
 * (mintlib/do_stat.c:101) */
#define OLDDATE __unixtime(0,0)

int
__do_stat (const char *_path, struct stat *st, int lflag)
{
	char pathbuf[PATH_MAX];
	char *path = (char *) _path;
	int nval;
	long r;

	if (!_path) {
		__set_errno (EFAULT);
		return -1;
	}

	if (*_path == '\0') {
		__set_errno (ENOENT);
		return -1;
	}

	if (__libc_unix_names) {
		nval = 0;
	} else {
		/* _unx2dos returns 1 for device names (like /dev/con) */
		path = pathbuf;
		nval = _unx2dos(_path, path, sizeof (pathbuf));
	}

	r = __sys_stat (path, st, lflag, 1);
	if (r != -ENOSYS) {
		if (r) {
			if ((r == -ENOTDIR) && _enoent(path)) {
				r = -ENOENT;
			}
			__set_errno (-r);
			return -1;
		}
		return 0;
	}

	{
	_DTA	*olddta;
	char	*ext, drv, *prevdir = NULL;
	_DTA	d;
	int	isdot = 0;
	int	isdir = 0;

	memset(st, 0, sizeof (*st));

	/* a name like CON: or AUX: */
	if (nval == 1) {
		st->st_mode = S_IFCHR | 0600;
		st->st_flags = 0;
		st->st_ino = __inode++;
		st->st_rdev = 0;
		st->st_mtime = st->st_ctime = st->st_atime = time((time_t *)0) - 2;
		st->st_dev = 0;
		st->st_nlink = 1;
		st->st_uid = geteuid();
		st->st_gid = getegid();
		st->st_size = st->st_blocks = 0;
		st->st_blksize = 1024;
		return 0;
	}

	/* A file name: check for root directory of a drive */
	if (path[0] == '\\' && path[1] == 0) {
		drv = Dgetdrv() + 'A';
		goto rootdir;
	}

	if (((drv = path[0]) != 0) && path[1] == ':' &&
	     (path[2] == 0 || (path[2] == '\\' && path[3] == 0)) ) {
rootdir:
		isdir = 1;
		st->st_mode = S_IFDIR | 0755;
		st->st_flags = 0;
		st->st_dev = isupper(drv) ? drv - 'A' : drv - 'a';
		st->st_ino = 2;
		st->st_mtime = st->st_ctime = st->st_atime = OLDDATE;
		goto fill_dir;
	}

	/* forbid wildcards in path names */
	if (strchr(path, '*') || strchr(path, '?')) {
		__set_errno (ENOENT);
		return -1;
	}

	/* Fsfirst(".",-1) and Fsfirst("..",-1) fail under TOS, and some TOS
	 * versions reject "RCS\\" too; mintlib searches "<dir>\*.*" instead,
	 * whose first hit is the "." entry (not in a root directory). Same
	 * kludge as mintlib/do_stat.c:197-256 and quickstat.c:118-174. */
	for (ext = path; ext[0] && ext[1]; ext++) {
		if (ext[1] && ext[1] != '.') {
			if (ext[0] == '\\') {
				prevdir = ext;
			}
		}
	}

	/* C:\XXXX\. */
	if (*ext == '.' && (ext == path || ext[-1] == '\\')) {
		isdot = 1;
		strcat (path, "\\*.*");
	}
	/* C:\XXXX\.. (one level only, as upstream) */
	else if (*ext == '.' && (ext == path || ext[-1] == '.')) {
		isdot = 1;
		if (prevdir) {
			/* C:\XXXX\YYYY\.. -> C:\XXXX\*.* */
			strcpy(prevdir, "\\*.*\0");
		} else {
			/* C:\.. -> C:\*.* */
			strcpy(&ext[-2], "\\*.*\0");
		}
	}
	/* C:\XXXX\ */
	else if (*ext == '\\') {
		isdot = 1;
		strcat (path, "*.*");
	}
	olddta = Fgetdta();
	Fsetdta(&d);
	r = Fsfirst (path, 0xff);
	Fsetdta (olddta);
	if (r < 0) {
		/* upstream: no "isdot && ENOENT -> rootdir" (sparemint bug 191) */
		__set_errno (-r);
		return -1;
	}

	if (isdot && ((d.dta_name[0] != '.') || (d.dta_name[1]))) {
		goto rootdir;
	}

	/* the entry's DOS time/date (mintlib/do_stat.c:281-282) */
	st->st_mtime = st->st_ctime = st->st_atime =
		__unixtime(d.dta_time, d.dta_date);
	if (((drv = *path) != 0) && path[1] == ':')
		st->st_dev = _toupper(drv) - 'A';
	else
		st->st_dev = Dgetdrv();

	isdir = (d.dta_attribute & FA_DIR) != 0;

	st->st_ino = __inode++;
	st->st_flags = 0;
	st->st_mode = 0644 | ((d.dta_attribute & FA_SYMLINK) ? S_IFLNK : isdir ? S_IFDIR | 0111 : S_IFREG);

	if (d.dta_attribute & FA_RDONLY)
		st->st_mode &= ~0222;	/* no write permission */
	if (d.dta_attribute & FA_HIDDEN)
		st->st_mode &= ~0444;	/* no read permission */

	/* executable extension: NewTOS's native .ntp/.nap/.ncp, compared
	 * case-insensitively -- the overlay mintlib/quickstat.c's rule (user
	 * decision 2026-10-02, TASK/PORT10), so stat() and access()/open()
	 * (which read __quickstat) agree on the exec bits */
	ext = strrchr(_path, '.');
	if (ext) {
		if (!strcasecmp(ext, ".ntp") || !strcasecmp(ext, ".nap") ||
		    !strcasecmp(ext, ".ncp")) {
			st->st_mode |= 0111;
		}
	}
	if ((st->st_mode & S_IFMT) == S_IFREG) {
		st->st_size = d.dta_size;
		/* in Unix, blocks are measured in 512 bytes */
		st->st_blocks = (st->st_size + 511) / 512;
		st->st_nlink = 1; /* we dont have hard links */
	} else {
fill_dir:
		st->st_size = 1024;
		st->st_blocks = 2;
		st->st_nlink = 2;	/* "foo" && "foo/.." */
	}

	st->st_uid = geteuid();	/* the current user owns every file */
	st->st_gid = getegid();
	st->st_blksize = 1024;

	}

	return 0;
}
