/* s_isnanl.c -- long double version of s_isnan.c.
 * Conversion to long double by Jakub Jelinek, jj@ultra.linux.cz.
 */

/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */

/*
 * isnanl(x) returns 1 is x is nan, else 0;
 * no branching!
 */

#include "math_private.h"

#ifndef NO_LONG_DOUBLE
#if defined(__LDBL_MANT_DIG__) && __LDBL_MANT_DIG__ == 113
/* NewTOS (TASK/SDK01 J): binary128 `long double' (AArch64). The m68k code
   below reads the 96-bit extended layout through GET_LDOUBLE_WORDS; this is
   the same branch-free test on the two 64-bit halves of binary128
   (GET_LDOUBLE_WORDS64, math_ldbl.h), as glibc's ldbl-128 s_isnanl.c: NaN
   <=> exponent all ones and fraction non-zero. */
int
__isnanl (long double x)
{
	int64_t hx, lx;

	GET_LDOUBLE_WORDS64(hx, lx, x);
	hx &= 0x7fffffffffffffffLL;
	hx |= (uint64_t) (lx | (int64_t) -(uint64_t) lx) >> 63;
	hx = (int64_t) (0x7fff000000000000ULL - (uint64_t) hx);
	return (int) ((uint64_t) hx >> 63);
}
#else
int
__isnanl (long double x)
{
	int32_t se, hx, lx;

	GET_LDOUBLE_WORDS(se, hx, lx, x);
	se = (se & 0x7fff) << 1;
	lx |= hx & 0x7fffffffL;
	se |= (uint32_t) (lx | (-lx)) >> 31;
	se = 0xfffe - se;
	return (int) (((uint32_t) (se)) >> 31);
}
#endif /* binary128 */
weak_alias (__isnanl, isnanl)
#endif
