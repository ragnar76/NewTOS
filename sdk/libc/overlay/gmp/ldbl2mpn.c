/* Copyright (C) 1995, 1996, 1997, 1998 Free Software Foundation, Inc.
   This file is part of the GNU C Library.

   The GNU C Library is free software; you can redistribute it and/or
   modify it under the terms of the GNU Library General Public License as
   published by the Free Software Foundation; either version 2 of the
   License, or (at your option) any later version.

   The GNU C Library is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Library General Public License for more details.

   You should have received a copy of the GNU Library General Public
   License along with the GNU C Library; if not, see
   <https://www.gnu.org/licenses/>.  */

#include <features.h>
#include "gmp.h"
#include "gmp-impl.h"
#include "longlong.h"
#include <ieee754.h>
#include <float.h>
#include <stdlib.h>

#ifndef __NO_LONG_DOUBLE_MATH

#if LDBL_MANT_DIG == 113

/* NewTOS (TASK/SDK01 J): `long double' is IEEE 754 binary128 on AArch64
   (union ieee854_long_double in the overlay <ieee754.h>). Same contract as
   the m68k version below and as __mpn_extract_double: the significand is
   returned right-aligned in RES_PTR with its leading one at bit
   LDBL_MANT_DIG - 1 (bit 112) for normal AND denormal numbers, and *EXPT is
   the unbiased exponent of that leading one. This is the algorithm of glibc's
   sysdeps/ieee754/ldbl-128/ldbl2mpn.c for 64-bit limbs (the only limb width
   of the LP64 build, stdlib/gmp-mparam.h).  */

#if BITS_PER_MP_LIMB != 64
# error "binary128 __mpn_extract_long_double: 64-bit limbs only"
#endif

/* Bits above the format's top bit in the high limb: 64 - (113 - 64) = 15.  */
#define NUM_LEADING_ZEROS (BITS_PER_MP_LIMB - (LDBL_MANT_DIG - BITS_PER_MP_LIMB))

mp_size_t
__mpn_extract_long_double (mp_ptr res_ptr, mp_size_t size,
			   int *expt, int *is_neg,
			   long double value)
{
  union ieee854_long_double u;
  u.d = value;

  (void) size;

  *is_neg = u.ieee.negative;
  *expt = (int) u.ieee.exponent - IEEE854_LONG_DOUBLE_BIAS;

  res_ptr[0] = ((mp_limb_t) u.ieee.mantissa2 << 32) | u.ieee.mantissa3;
  res_ptr[1] = ((mp_limb_t) u.ieee.mantissa0 << 32) | u.ieee.mantissa1;

  if (u.ieee.exponent == 0)
    {
      /* A biased exponent of zero is a special case.
	 Either it is a zero or it is a denormal number.  */
      if (res_ptr[0] == 0 && res_ptr[1] == 0)
	/* It's zero.  */
	*expt = 0;
      else
	{
	  /* It is a denormal number, meaning it has no implicit leading
	     one bit, and its exponent is in fact the format minimum.
	     Normalize: move the leading one to bit 112.  */
	  int cnt;

	  if (res_ptr[1] != 0)
	    {
	      /* Leading one in the high limb, at or below bit 47: shift by
		 cnt >= 1 (bit 48 is the implicit bit and is clear here).  */
	      count_leading_zeros (cnt, res_ptr[1]);
	      cnt -= NUM_LEADING_ZEROS;
	      res_ptr[1] = res_ptr[1] << cnt
			   | (res_ptr[0] >> (BITS_PER_MP_LIMB - cnt));
	      res_ptr[0] <<= cnt;
	      *expt = LDBL_MIN_EXP - 1 - cnt;
	    }
	  else
	    {
	      /* Leading one in the low limb: shift by
		 BITS_PER_MP_LIMB - NUM_LEADING_ZEROS + cnt (49 + cnt).  */
	      count_leading_zeros (cnt, res_ptr[0]);
	      if (cnt >= NUM_LEADING_ZEROS)
		{
		  res_ptr[1] = res_ptr[0] << (cnt - NUM_LEADING_ZEROS);
		  res_ptr[0] = 0;
		}
	      else
		{
		  res_ptr[1] = res_ptr[0] >> (NUM_LEADING_ZEROS - cnt);
		  res_ptr[0] <<= BITS_PER_MP_LIMB - (NUM_LEADING_ZEROS - cnt);
		}
	      *expt = LDBL_MIN_EXP - 1
		      - (BITS_PER_MP_LIMB - NUM_LEADING_ZEROS) - cnt;
	    }
	}
    }
  else
    /* Add the implicit leading one bit for a normalized number.  */
    res_ptr[1] |= (mp_limb_t) 1 << (LDBL_MANT_DIG - 1 - BITS_PER_MP_LIMB);

  return 2;
}

#else /* m68k 96-bit extended */

#ifdef __mcoldfire__
#  define NO_LONG_DOUBLE 1
#endif

/* Convert a `long double' in IEEE854 standard double-precision format to a
   multi-precision integer representing the significand scaled up by its
   number of bits (64 for long double) and an integral power of two
   (MPN frexpl). */

mp_size_t
__mpn_extract_long_double (mp_ptr res_ptr, mp_size_t size,
			   int *expt, int *is_neg,
			   long double value)
{
#ifdef NO_LONG_DOUBLE
  union ieee754_double u;
#else
  union ieee854_long_double u;
#endif
  u.d = value;

  (void) size;

  *is_neg = u.ieee.negative;
#ifdef NO_LONG_DOUBLE
  *expt = (int) u.ieee.exponent - IEEE754_DOUBLE_BIAS;
#else
  *expt = (int) u.ieee.exponent - IEEE854_LONG_DOUBLE_BIAS;
#endif

#if BITS_PER_MP_LIMB == 32
  res_ptr[0] = u.ieee.mantissa1; /* Low-order 32 bits of fraction.  */
  res_ptr[1] = u.ieee.mantissa0; /* High-order 32 bits.  */
  #define N 2
#elif BITS_PER_MP_LIMB == 64
  /* Hopefully the compiler will combine the two bitfield extracts
     and this composition into just the original quadword extract.  */
  res_ptr[0] = ((unsigned long int) u.ieee.mantissa0 << 32) | u.ieee.mantissa1;
  #define N 1
#else
  #error "mp_limb size " BITS_PER_MP_LIMB "not accounted for"
#endif

  if (u.ieee.exponent == 0)
    {
      /* A biased exponent of zero is a special case.
	 Either it is a zero or it is a denormal number.  */
      if (res_ptr[0] == 0 && res_ptr[N - 1] == 0) /* Assumes N<=2.  */
	/* It's zero.  */
	*expt = 0;
      else
	{
          /* It is a denormal number, meaning it has no implicit leading
  	     one bit, and its exponent is in fact the format minimum.  */
	  int cnt;

	  if (res_ptr[N - 1] != 0)
	    {
	      count_leading_zeros (cnt, res_ptr[N - 1]);
	      if (cnt != 0)
		{
#if N == 2
	          res_ptr[N - 1] = res_ptr[N - 1] << cnt
			           | (res_ptr[0] >> (BITS_PER_MP_LIMB - cnt));
	          res_ptr[0] <<= cnt;
#else
	          res_ptr[N - 1] <<= cnt;
#endif
		}
#ifdef NO_LONG_DOUBLE
	      *expt = DBL_MIN_EXP - 1 - cnt;
#else
	      *expt = LDBL_MIN_EXP - 1 - cnt;
#endif
	    }
	  else
	    {
	      count_leading_zeros (cnt, res_ptr[0]);
	      res_ptr[N - 1] = res_ptr[0] << cnt;
	      res_ptr[0] = 0;
#ifdef NO_LONG_DOUBLE
	      *expt = DBL_MIN_EXP - 1 - BITS_PER_MP_LIMB - cnt;
#else
	      *expt = LDBL_MIN_EXP - 1 - BITS_PER_MP_LIMB - cnt;
#endif
	    }
	}
    }

  return N;
}

#endif	/* binary128 / m68k extended */

#endif	/* __NO_LONG_DOUBLE_MATH */
