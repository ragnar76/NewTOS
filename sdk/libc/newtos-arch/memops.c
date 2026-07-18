/*
 * memops.c -- NewTOS/AArch64 arch shim for the freestanding mem* routines.
 *
 * mintlib provides memcpy/memmove/memset/bcopy/bzero only as m68k assembly
 * (string/bcopy.S, string/memset.S, string/bzero.S), which the AArch64
 * backend cannot assemble.  These are pure architecture-dependent block
 * routines with no observable TOS-API contract beyond ISO C semantics, so we
 * reimplement them cleanly in C for AArch64 with the exact public symbol set
 * the m68k versions exported (see the `.globl` lists in those files):
 *
 *   bcopy.S : bcopy, __bcopy, _bcopy, memcpy, memmove
 *   memset.S: memset, __memset, _memset
 *   bzero.S : bzero, __bzero, _bzero
 *
 * The whole libc is built with -fno-builtin, so the compiler will not turn
 * these copy/fill loops back into self-referential mem* libcalls.
 */

#include <stddef.h>

void *
memcpy (void *dest, const void *src, size_t n)
{
	unsigned char *d = (unsigned char *) dest;
	const unsigned char *s = (const unsigned char *) src;

	while (n--)
		*d++ = *s++;
	return dest;
}

void *
memmove (void *dest, const void *src, size_t n)
{
	unsigned char *d = (unsigned char *) dest;
	const unsigned char *s = (const unsigned char *) src;

	if (d == s || n == 0)
		return dest;

	if (d < s) {
		while (n--)
			*d++ = *s++;
	} else {
		d += n;
		s += n;
		while (n--)
			*--d = *--s;
	}
	return dest;
}

void *
memset (void *dest, int c, size_t n)
{
	unsigned char *d = (unsigned char *) dest;

	while (n--)
		*d++ = (unsigned char) c;
	return dest;
}

/* BSD aliases exported by the original m68k routines. */
void
bcopy (const void *src, void *dest, size_t n)
{
	(void) memmove (dest, src, n);
}

void
bzero (void *dest, size_t n)
{
	(void) memset (dest, 0, n);
}
