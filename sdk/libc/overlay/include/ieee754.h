/* Copyright (C) 1992, 1995, 1996 Free Software Foundation, Inc.
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

/* Adapted to MiNTLib by Guido Flohr <guido@freemint.de>.  */

#ifndef _IEEE754_H

#define _IEEE754_H 1

#include <features.h>

#include <endian.h>

__BEGIN_DECLS

union ieee754_float
  {
    float f;

    /* This is the IEEE 754 single-precision format.  */
    struct
      {
#if	__BYTE_ORDER == __BIG_ENDIAN
	unsigned negative:1;
	unsigned exponent:8;
	unsigned long mantissa:23;
#endif				/* Big endian.  */
#if	__BYTE_ORDER == __LITTLE_ENDIAN
	unsigned long mantissa:23;
	unsigned exponent:8;
	unsigned negative:1;
#endif				/* Little endian.  */
      } ieee;

    /* This format makes it easier to see if a NaN is a signalling NaN.  */
    struct
      {
#if	__BYTE_ORDER == __BIG_ENDIAN
	unsigned negative:1;
	unsigned exponent:8;
	unsigned quiet_nan:1;
	unsigned long mantissa:22;
#endif				/* Big endian.  */
#if	__BYTE_ORDER == __LITTLE_ENDIAN
	unsigned long mantissa:22;
	unsigned quiet_nan:1;
	unsigned exponent:8;
	unsigned negative:1;
#endif				/* Little endian.  */
      } ieee_nan;
  };

#define IEEE754_FLOAT_BIAS	0x7f /* Added to exponent.  */


union ieee754_double
  {
    double d;

    /* This is the IEEE 754 double-precision format.  */
    struct
      {
#if	__BYTE_ORDER == __BIG_ENDIAN
	unsigned negative:1;
	unsigned exponent:11;
	/* Together these comprise the mantissa.  */
	unsigned long mantissa0:20;
	unsigned long mantissa1:32;
#endif				/* Big endian.  */
#if	__BYTE_ORDER == __LITTLE_ENDIAN
	/* Together these comprise the mantissa.  */
	unsigned long mantissa1:32;
	unsigned long mantissa0:20;
	unsigned exponent:11;
	unsigned negative:1;
#endif				/* Little endian.  */
      } ieee;

    /* This format makes it easier to see if a NaN is a signalling NaN.  */
    struct
      {
#if	__BYTE_ORDER == __BIG_ENDIAN
	unsigned negative:1;
	unsigned exponent:11;
	unsigned quiet_nan:1;
	/* Together these comprise the mantissa.  */
	unsigned long mantissa0:19;
	unsigned long mantissa1:32;
#else
	/* Together these comprise the mantissa.  */
	unsigned long mantissa1:32;
	unsigned long mantissa0:19;
	unsigned quiet_nan:1;
	unsigned exponent:11;
	unsigned negative:1;
#endif
      } ieee_nan;
  };

#define IEEE754_DOUBLE_BIAS	0x3ff /* Added to exponent.  */


#if defined (__LDBL_MANT_DIG__) && __LDBL_MANT_DIG__ == 113
/* NewTOS (TASK/SDK01 J): AArch64 `long double' is IEEE 754 binary128
   (AAPCS64): 1 sign bit, 15 exponent bits (bias 0x3fff), 112 fraction bits
   with an implicit leading one -- not the m68k 96-bit extended format below,
   which is the only one mintlib knows. This is glibc's ldbl-128 layout of the
   same union (sysdeps/ieee754/ldbl-128/ieee754.h), little-endian only, since
   that is the only byte order NewTOS has. mantissa0 holds the 16 most
   significant fraction bits, mantissa3 the 32 least significant. All fields
   are `unsigned int' so that no field straddles a 32-bit unit.  */
union ieee854_long_double
  {
    long double d;

    /* This is the IEEE 754 binary128 (quad-precision) format.  */
    struct
      {
	unsigned int mantissa3:32;
	unsigned int mantissa2:32;
	unsigned int mantissa1:32;
	unsigned int mantissa0:16;
	unsigned int exponent:15;
	unsigned int negative:1;
      } ieee;

    /* This format makes it easier to see if a NaN is a signalling NaN.  */
    struct
      {
	unsigned int mantissa3:32;
	unsigned int mantissa2:32;
	unsigned int mantissa1:32;
	unsigned int mantissa0:15;
	unsigned int quiet_nan:1;
	unsigned int exponent:15;
	unsigned int negative:1;
      } ieee_nan;
  };

#if __BYTE_ORDER != __LITTLE_ENDIAN
# error "ieee854_long_double: the binary128 layout is little-endian only"
#endif

#define IEEE854_LONG_DOUBLE_BIAS 0x3fff

#else /* m68k 96-bit extended */

union ieee854_long_double
  {
    long double d;

    /* This is the IEEE 854 double-extended-precision format.  */
    struct
      {
#if	__BYTE_ORDER == __BIG_ENDIAN
	unsigned negative:1;
	unsigned exponent:15;
	unsigned empty:16;
	unsigned long mantissa0:32;
	unsigned long mantissa1:32;
#endif
#if	__BYTE_ORDER == __LITTLE_ENDIAN
	unsigned long mantissa1:32;
	unsigned long mantissa0:32;
	unsigned exponent:15;
	unsigned negative:1;
	unsigned empty:16;
#endif
      } ieee;

    /* This is for NaNs in the IEEE 854 double-extended-precision format.  */
    struct
      {
#if	__BYTE_ORDER == __BIG_ENDIAN
	unsigned negative:1;
	unsigned exponent:15;
	unsigned empty:16;
	unsigned one:1;
	unsigned quiet_nan:1;
	unsigned long mantissa0:30;
	unsigned long mantissa1:32;
#endif
#if	__BYTE_ORDER == __LITTLE_ENDIAN
	unsigned long mantissa1:32;
	unsigned long mantissa0:30;
	unsigned quiet_nan:1;
	unsigned one:1;
	unsigned exponent:15;
	unsigned negative:1;
	unsigned empty:16;
#endif
      } ieee_nan;
  };

#define IEEE854_LONG_DOUBLE_BIAS 0x3fff

#endif /* binary128 / m68k extended */

__END_DECLS

#endif /* ieee754.h */
