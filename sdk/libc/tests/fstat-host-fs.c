/*
 * fstat-host-fs.c -- the GEMDOS side and the mintlib-typed probes of
 * fstat-host (TASK/FS08 part 4: st_blksize, and through it the stdio buffer
 * size, for a regular file).
 *
 * Compiled by fstat-host.sh with the mintlib include path and DEFS of
 * sdk/libc/build-libc.sh, so `struct stat', FILE and BUFSIZ are mintlib's.
 * It stands in for the one GEMDOS call the lean __do_fstat makes:
 *
 *   Fseek  -- a table of open handles, each with a size and a position;
 *             SEEK_SET/CUR/END move the position and return it, as GEMDOS
 *             Fseek does (docs/api/gemdos/file/42-fseek.md). A handle not in
 *             the table answers EIHNDL (-37). A negative handle (a BIOS
 *             device, e.g. -1 = CON:) answers 0 here, i.e. it LOOKS seekable,
 *             so the check that devices keep BUFSIZ proves that the shim
 *             classifies by handle number, not by Fseek success.
 *
 * fh_probe_stat() and fh_probe_stream() hand the harness plain integers, so
 * the harness (fstat-host.c) can use the host's headers.
 */
#include <string.h>
#include <stdio.h>
#include <sys/stat.h>
#include "lib.h"

struct fh { short h; long size; long pos; };
static struct fh tab[] = {
	{ 3, 4096L, 0 },		/* a standard handle redirected to a file */
	{ 6, 10485760L, 0 },		/* a regular file, 10 MiB */
	{ 7, 0L, 0 },			/* an empty regular file */
	{ 8, 300003L, 0 },		/* a regular file, odd size */
};
#define NTAB ((int)(sizeof tab / sizeof tab[0]))

long
fh_test_Fseek (long off, short h, short mode)
{
	int i;
	if (h < 0)
		return 0;
	for (i = 0; i < NTAB; i++) {
		long np;
		if (tab[i].h != h)
			continue;
		np = mode == SEEK_SET ? off : mode == SEEK_CUR ? tab[i].pos + off : tab[i].size + off;
		if (np < 0 || np > tab[i].size)
			return -64;		/* ERANGE, as GEMDOS */
		tab[i].pos = np;
		return np;
	}
	return -37;				/* EIHNDL */
}

void
fh_set_pos (short h, long pos)
{
	int i;
	for (i = 0; i < NTAB; i++)
		if (tab[i].h == h)
			tab[i].pos = pos;
}

long
fh_get_pos (short h)
{
	int i;
	for (i = 0; i < NTAB; i++)
		if (tab[i].h == h)
			return tab[i].pos;
	return -1;
}

long fh_bufsiz (void) { return BUFSIZ; }

/* __do_fstat(fd): returns its result; *mode, *size, *blksize from the stat. */
int
fh_probe_stat (int fd, int *is_reg, int *is_chr, long *size, long *blksize)
{
	struct stat st;
	int r = __do_fstat (fd, &st, 1);
	*is_reg = S_ISREG (st.st_mode) ? 1 : 0;
	*is_chr = S_ISCHR (st.st_mode) ? 1 : 0;
	*size = (long) st.st_size;
	*blksize = (long) st.st_blksize;
	return r;
}

static int
fh_dummy_seek (void *cookie, fpos_t *pos, int whence)
{
	(void) cookie; (void) pos; (void) whence;
	return 0;
}

/* mintlib's own __stdio_init_stream (stdio/stdio_init.c) on a fresh FILE
 * whose cookie is fd -- what fopen/fdopen run before the first transfer --
 * returns the __bufsize it chose; *linebuf and *seekable as it left them. */
long
fh_probe_stream (int fd, int *linebuf, int *seekable)
{
	FILE f;
	memset (&f, 0, sizeof f);
	f.__cookie = (void *) (long) fd;
	f.__io_funcs.__seek = fh_dummy_seek;	/* as __stdio_check_funcs sets it */
	__stdio_init_stream (&f);
	*linebuf = f.__linebuf;
	*seekable = f.__io_funcs.__seek != NULL;
	return (long) f.__bufsize;
}
