/*
 * complex.h -- NewTOS SDK adapter: the standard <complex.h> name on top of
 * OpenLibm's <openlibm_complex.h> (see math.h in this directory for why the
 * adapters exist). The C99 complex functions are in libm.a. casin, cacos,
 * casinh, cacosh, catanh and their float variants call the compiler-rt
 * complex-multiply helpers __muldc3/__mulsc3, and C99 complex `*`/`/` in a
 * program does too; since TASK/SDK01 E they come from
 * sdk/compiler-rt/libclang_rt.builtins.a, which sdk/bin/newtos-cc links
 * (docs/sdk/05-abi-pitfalls.md B12). The inverse functions are OpenLibm's
 * old Cephes code -- see docs/sdk/07-mintlib-limits.md section 4 for their
 * accuracy.
 */
#ifndef _NEWTOS_COMPLEX_H
#define _NEWTOS_COMPLEX_H 1

#include <openlibm_complex.h>

#endif /* _NEWTOS_COMPLEX_H */
