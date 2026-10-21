/*
 * Written by J.T. Conklin <jtc@netbsd.org>.
 * Change for long double by Jakub Jelinek <jj@ultra.linux.cz>
 * Public domain.
 */

/*
 * isinfl(x) returns 1 if x is inf, -1 if x is -inf, else 0;
 * no branching!
 */

#include "math_private.h"

#ifndef NO_LONG_DOUBLE
#if defined(__LDBL_MANT_DIG__) && __LDBL_MANT_DIG__ == 113
/* NewTOS (TASK/SDK01 J): binary128 `long double' (AArch64). The m68k code
   below reads the 96-bit extended layout through GET_LDOUBLE_WORDS; this is
   the same branch-free test on the two 64-bit halves of binary128
   (GET_LDOUBLE_WORDS64, math_ldbl.h), as glibc's ldbl-128 s_isinfl.c:
   +inf -> 1, -inf -> -1, everything else -> 0. */
int
__isinfl (long double x)
{
	int64_t hx, lx;

	GET_LDOUBLE_WORDS64(hx, lx, x);
	lx |= (hx & 0x7fffffffffffffffLL) ^ 0x7fff000000000000LL;
	lx |= (int64_t) -(uint64_t) lx;
	return (int)(~(lx >> 63) & (hx >> 62));
}
#else
int
__isinfl (long double x)
{
	int32_t se, hx, lx;

	GET_LDOUBLE_WORDS(se, hx, lx, x);
	lx |= (hx & 0x7fffffffL) | ((se & 0x7fff) ^ 0x7fff);
	lx |= -lx;
	se &= 0x8000;
	return (int)(~(lx >> 31) & (1 - (se >> 14)));
}
#endif /* binary128 */
weak_alias (__isinfl, isinfl)
#endif
