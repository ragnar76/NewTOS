/*
 * NewTOS overlay of libshare-0.1.1/sbrk.c (TASK/LDG-M3-B2). One change:
 *
 *   sbrk.c:61   sbrk (size_t n)  ->  sbrk (intptr_t n)
 *
 * Reason (target): the mintlib the SDK's libc is built from (0.60.1)
 * declares `extern void *sbrk (__intptr_t __delta)` in <unistd.h>:934, which
 * sbrk.c includes; the 2004 mintlib libshare was written against used
 * size_t. With the pristine line the file does not compile ("conflicting
 * types for 'sbrk'"). The same change is in th-otto's in-tree copy
 * (zview/plugins/libshare/sbrk.c:61). Behaviour is unchanged for every caller
 * in libshare (malloc.c:129 passes a positive page-rounded size); n reaches
 * ldg_Malloc(long), sbrk.c:73, as before.
 * Everything below this comment is the original file, byte for byte, except
 * that one line.
 */
/*
 *  libshare: a dirty patch to make LDG compiled against mintlib shareable
 *
 *  by Arnaud BERCEGEAY (Feb 2004)
 */

/* sbrk: emulate Unix sbrk call */
/* by ERS */
/* jrb: added support for allocation from _heapbase when _stksize == -1 
	thanks to Piet van Oostrum & Atze Dijkstra for this idea and
        their diffs. */

/* WARNING: sbrk may not allocate space in continguous memory, i.e.
   two calls to sbrk may not return consecutive memory. This is
   unlike Unix.
*/

/* Further WARNING: in a split_mem model the memory addresses will NOT
   be monotonous. sigh!  (i hate these mem models as much as the next
   person. as usual, people at atari are totally oblivious to such
   brain damage, even when pointed out to them. sigh!)
*/

/*
 * support heat and serve C -- whose author continues to be adamant about
 * size_t -- big sigh!
 */

#include <errno.h>
#include <stddef.h>
#include <unistd.h>
#include <mint/osbind.h>
#include "lib.h"

#include <ldg.h>

static void *
HeapAlloc (unsigned long sz)
{
	char slush [64];
	register void *sp;
	
	sp = (void *)slush;
	sz = (sz + 7) & ~((unsigned long) 7L); /* round up request size next octet */

	if (sp < (void *)((char *) _heapbase + sz))
		return NULL;

	sp = _heapbase;
	_heapbase = (void *)((char *)_heapbase + sz);
	_stksize -= (long) sz;

	return sp;
}

/* [libshare debug] */
int __sbrk_has_been_called = 0;

/* provided for compilers with sizeof(int) == 2 */
void *
sbrk (intptr_t n)
{
	void *rval;
	
	/* [libshare debug] */
	__sbrk_has_been_called = 1;

	if ((!_split_mem) && (_heapbase != NULL)) {
		if (n) rval = HeapAlloc (n);
		else rval = _heapbase;
	}
	else {
		rval = (void *) ldg_Malloc (n);
		/* TODO: add SHM stuff here */
	}

	if (rval == NULL) {
		if(_split_mem) { 
			/* now switch over to own heap for further requests,
			 * including this one */
			_split_mem = 0;
			return sbrk (n);
		}

		__set_errno (ENOMEM);
		rval = (void *)(-1L);
	}
	return rval;
}
