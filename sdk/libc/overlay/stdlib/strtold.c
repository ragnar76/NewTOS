/* NewTOS (TASK/SDK01 J): DENORM_EXP = MIN_EXP - 1 is the m68k 96-bit
   extended format's hook (its denormals share the bias of the normal
   numbers). AArch64 `long double' is IEEE 754 binary128, whose denormals
   have biased exponent 0 = strtod.c's default DENORM_EXP (MIN_EXP - 2), so
   the hook stays off there. */
#if defined(__MINT__) && !(defined(__LDBL_MANT_DIG__) && __LDBL_MANT_DIG__ == 113)
# define DENORM_EXP (MIN_EXP - 1)
#endif

#include "math_private.h"

#if !defined(__NO_LONG_DOUBLE_MATH) && !defined(__mcoldfire__)
/* The actual implementation for all floating point sizes is in strtod.c.
   These macros tell it to produce the `long double' version, `strtold'.  */

# define FLOAT		long double
# define FLT		LDBL
# ifdef USE_IN_EXTENDED_LOCALE_MODEL
#  define STRTOF	__strtold_l
# else
#  define STRTOF	strtold
# endif
# define MPN2FLOAT	__mpn_construct_long_double
# define FLOAT_HUGE_VAL	__builtin_infl()
# define FLOAT_NAN_VAL	__builtin_nanl("")
# if defined(__LDBL_MANT_DIG__) && __LDBL_MANT_DIG__ == 113
/* binary128 (NewTOS, TASK/SDK01 J): the same rule as the double and m68k
   macros -- the n-char-sequence of "nan(...)" becomes the fraction, and an
   empty one gives the quiet bit (fraction bit 111, the top of mantissa0).
   The 64-bit MANT fills the low 64 of the 112 fraction bits. */
#  define SET_MANTISSA(flt, mant) \
  do { union ieee854_long_double u;					      \
       u.d = (flt);							      \
       u.ieee.mantissa0 = (mant) == 0 ? 0x8000 : 0;			      \
       u.ieee.mantissa1 = 0;						      \
       u.ieee.mantissa2 = ((mant) >> 32) & 0xffffffff;			      \
       u.ieee.mantissa3 = (mant) & 0xffffffff;				      \
       (flt) = u.d;							      \
  } while (0)
# else
# define SET_MANTISSA(flt, mant) \
  do { union ieee854_long_double u;					      \
       u.d = (flt);							      \
       if ((mant & 0x7fffffffffffffffULL) == 0)				      \
	 mant = 0x4000000000000000ULL;					      \
       u.ieee.mantissa0 = (((mant) >> 32) & 0x7fffffff) | 0x80000000;	      \
       u.ieee.mantissa1 = (mant) & 0xffffffff;				      \
       (flt) = u.d;							      \
  } while (0)
# endif

# include "strtod.c"

#else
# include <stdlib.h>

double __strtod_internal (const char *nptr, char **endptr, int group);
long double __strtold_internal (const char *nptr, char **endptr, int group);

/* There is no `long double' type, use the `double' implementations.  */
long double
__strtold_internal (const char *nptr, char **endptr, int group)
{
  return __strtod_internal (nptr, endptr, group);
}

long double
strtold (const char *nptr, char **endptr)
{
  return __strtod_internal (nptr, endptr, 0);
}
#endif
