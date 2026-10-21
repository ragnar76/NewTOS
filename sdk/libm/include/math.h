/*
 * math.h -- NewTOS SDK adapter: the standard <math.h> name on top of OpenLibm.
 *
 * mintlib ships no <math.h> (on m68k it came from the compiler's PML/fdlibm;
 * mintlib's include/tcmath.h is a placeholder that #errors). The SDK's libm is
 * OpenLibm (sdk/libm/, TASK/T07), whose header is <openlibm_math.h>. This file
 * gives a program written against <math.h> that header, and translates the
 * mintlib feature macros (features.h: __USE_MISC/__USE_BSD/__USE_XOPEN, all
 * set by _GNU_SOURCE) into the FreeBSD-style visibility gates OpenLibm's
 * header uses for its non-ISO parts: M_PI and friends are under
 * __BSD_VISIBLE || __XSI_VISIBLE, the BSD extras (j0, y0, gamma, ...) under
 * __BSD_VISIBLE. The ISO C99 set is always visible.
 *
 * long double: OpenLibm's header declares the `l` functions (OLM_LONG_DOUBLE);
 * libm.a contains them since TASK/SDK01 J (OpenLibm's ld128/ set,
 * docs/sdk/07-mintlib-limits.md section 4). `long double` arithmetic itself
 * (binary128, soft-float) links since TASK/SDK01 E: sdk/bin/newtos-cc links
 * compiler-rt's builtins (docs/sdk/05-abi-pitfalls.md B12).
 */
#ifndef _NEWTOS_MATH_H
#define _NEWTOS_MATH_H 1

#ifndef _FEATURES_H
# include <features.h>
#endif

#if !defined(__BSD_VISIBLE) && (defined(__USE_MISC) || defined(__USE_BSD))
# define __BSD_VISIBLE 1
#endif
#if !defined(__XSI_VISIBLE) && defined(__USE_XOPEN)
# define __XSI_VISIBLE 1
#endif

#include <openlibm_math.h>

#endif /* _NEWTOS_MATH_H */
