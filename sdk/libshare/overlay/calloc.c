/*
 * NewTOS overlay of libshare-0.1.1/calloc.c (TASK/LDG-M3-B2). One change:
 *
 *   calloc.c:25   __bzero (r, total)  ->  bzero (r, total)
 *
 * Reason (target): the SDK's libc has no `__bzero`. mintlib's m68k
 * string/bzero.S exported bzero, __bzero and _bzero; its AArch64 replacement
 * sdk/libc/newtos-arch/memops.c defines only `bzero` (memops.c:70-74) although
 * its header comment lists all three. With the pristine line the archive
 * leaves `__bzero` undefined and every module that pulls calloc fails to
 * link. `bzero` is the same operation (memops.c: memset(dest, 0, n)), and
 * libshare's own malloc.c:180 already calls `bzero`.
 * Everything below this comment is the original file, byte for byte, except
 * that one line.
 */
/*
 *  libshare: a dirty patch to make LDG compiled against mintlib shareable
 *
 *  by Arnaud BERCEGEAY (Feb 2004)
 */

/* from the TOS GCC library */
/* malloc, free, realloc: dynamic memory allocation */
/* 5/2/92 sb -- modified for Heat-n-Serve C to accomodate its 16-bit size_t */
/* 5/5/92 sb -- calloc() gets its own file to reduce library drag */

#include <stdlib.h>
#include <string.h>

void *
calloc (size_t n, size_t sz)
{
	register void *r;
	register size_t total;

	total = n * sz;

	r = malloc (total);
	if (r != NULL)
		bzero (r, total);

	return r;
}
