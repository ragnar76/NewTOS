/* m68k is big-endian.  */

#ifndef _ENDIAN_H
# error "Never use <bits/endian.h> directly; include <endian.h> instead."
#endif

#ifndef __BYTE_ORDER__
#define __BYTE_ORDER__ __ORDER_BIG_ENDIAN__
#endif
/* NewTOS (TASK/SDK01 J): the floating-point word order follows the byte
   order. gcc predefines __FLOAT_WORD_ORDER__, clang does not (it predefines
   __BYTE_ORDER__ only), so on AArch64 the m68k fallback here claimed a
   big-endian float word order for a little-endian machine. mintlib's
   math_private.h and math_ldbl.h then used the m68k word layout and
   __isnan/__isinf/__finite read the wrong half of a double (printf("%f",
   INFINITY) printed DBL_MAX's 309 digits, a NaN a number), __finitel the
   wrong half of a long double. For m68k nothing changes: __BYTE_ORDER__ is
   big-endian there (or gcc's predefinition wins). */
#ifndef __FLOAT_WORD_ORDER__
#define __FLOAT_WORD_ORDER__ __BYTE_ORDER__
#endif
