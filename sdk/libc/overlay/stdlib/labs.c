/* labs -- long integer absolute value.
 *
 * NewTOS overlay over mintlib/stdlib/labs.c.
 *
 * The pristine file, on a non-__MSHORT__ target, defines labs as
 *   strong_alias(abs, labs)
 * i.e. an alias to abs() which lives in a *different* translation unit
 * (stdlib/abs.c). Under the GCC-2.7+ branch of mintlib/libc-symbols.h,
 * strong_alias expands to `__attribute__((alias("abs")))`. clang (unlike the
 * m68k gcc the pristine build used) requires an alias target to be defined in
 * the SAME translation unit; a cross-TU alias attribute is silently dropped, so
 * the pristine labs.o exports no `labs` symbol at all (verified: `nm` shows only
 * `U abs`). QED's window.c (do_arrow) calls labs(), which then fails to link.
 *
 * This is a clang-vs-gcc toolchain incompatibility with cross-TU alias
 * attributes, not a QED problem. The honest fix is to give labs a real
 * definition here -- exactly what the pristine file already does on its
 * __MSHORT__ branch. Same value in every case, no alias.
 */

#include <stdlib.h>

#ifdef labs
#undef labs
#endif

long
labs(long x)
{
	return x < 0 ? -x : x;
}
