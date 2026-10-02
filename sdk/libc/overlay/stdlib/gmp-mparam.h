/* gmp-mparam.h -- Compiler/machine parameter header file.

Copyright (C) 1991, 1993, 1994 Free Software Foundation, Inc.

This file is part of the GNU MP Library.

The GNU MP Library is free software; you can redistribute it and/or modify
it under the terms of the GNU Library General Public License as published by
the Free Software Foundation; either version 2 of the License, or (at your
option) any later version.

The GNU MP Library is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU Library General Public
License for more details.

You should have received a copy of the GNU Library General Public License
along with the GNU MP Library; if not, see
<https://www.gnu.org/licenses/>. */

/* Modified for MiNTLib by Guido Flohr <guido@freemint.de>.  */

/* NewTOS/AArch64 (TASK/PORT10): the limb width follows the machine.
 *
 * The pristine header hard-codes BITS_PER_MP_LIMB 32 / BYTES_PER_MP_LIMB 4 /
 * BITS_PER_LONGINT 32 -- true on m68k, where `long` is 32 bits. But the limb
 * type itself comes from stdlib/gmp.h, `typedef unsigned long int mp_limb_t`
 * (neither _SHORT_LIMB nor _LONG_LONG_LIMB is defined anywhere in the build),
 * and on LP64 that is 64 bits. With the pristine values every GMP user
 * (stdio/printf_fp.c, stdio/_itoa.c, stdlib/strtod.c, stdlib/fpioconst.c's
 * power-of-ten table, gmp/dbl2mpn.c, gmp/mpn-*.c) selected its 32-bit-limb
 * code and tables and ran them on 64-bit limbs: __printf_fp printed wrong
 * digits ("1.?00000" for 1.5) and, for some values, ran past its digit
 * buffers -- the Data Abort in __printf_fp seen in PORT06's timedemo line.
 *
 * Every one of those files already carries the BITS_PER_MP_LIMB == 64 branch
 * (it is glibc code); the only m68k-ism was this header. The values below are
 * therefore derived from the compiler's own `long` width, which is exactly the
 * width of mp_limb_t: 32 on m68k (the pristine numbers, unchanged), 64 on
 * AArch64. sdk/libc/tests/gmp-host.sh checks the table and the conversion on
 * an LP64 host.  */
#if !defined (__SIZEOF_LONG__) || (__SIZEOF_LONG__ != 4 && __SIZEOF_LONG__ != 8)
# error "gmp-mparam.h: cannot derive the limb width from __SIZEOF_LONG__"
#endif
#if defined (_SHORT_LIMB) || defined (_LONG_LONG_LIMB)
# error "gmp-mparam.h: limb width derived for mp_limb_t == unsigned long only"
#endif

#define BITS_PER_MP_LIMB (__SIZEOF_LONG__ * 8)
#define BYTES_PER_MP_LIMB __SIZEOF_LONG__
#define BITS_PER_LONGINT (__SIZEOF_LONG__ * 8)
#ifdef __MSHORT__
# define  BITS_PER_INT 16
#else
# define BITS_PER_INT 32
#endif
#define BITS_PER_SHORTINT 16
#define BITS_PER_CHAR 8
