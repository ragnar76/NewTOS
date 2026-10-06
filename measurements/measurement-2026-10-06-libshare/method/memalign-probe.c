/* Link probe (TASK/LDG-M3-B2): a "module" that uses every allocator entry
   point, including the aligned ones that in mintlib 0.60.1 call __malloc
   directly. Linked like an .NLD by link-probe.sh; only the link map matters,
   the program is never run. */
#include <stdlib.h>
#include <malloc.h>
#include <unistd.h>
int main (void)
{
	void *p = malloc (10), *q = calloc (2, 3), *r;
	void *a = memalign (64, 10), *v = valloc (10), *al = aligned_alloc (64, 64);
	r = realloc (p, 20);
	if (posix_memalign (&p, 64, 10) == 0) free (p);
	free (q); free (r); free (a); free (v); free (al);
	(void) sbrk (0); (void) chdir ("\\");
	return 0;
}
