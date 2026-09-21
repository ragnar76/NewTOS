/*
 * complex.h -- NewTOS SDK adapter: the standard <complex.h> name on top of
 * OpenLibm's <openlibm_complex.h> (see math.h in this directory for why the
 * adapters exist). The C99 complex functions are in libm.a; four of them --
 * casin, casinh, cacosh, catanh and their float variants -- need the
 * compiler-rt complex-multiply helpers __muldc3/__mulsc3, which the SDK does
 * not have (sdk/libm/build/dangling.txt), so those fail to link.
 */
#ifndef _NEWTOS_COMPLEX_H
#define _NEWTOS_COMPLEX_H 1

#include <openlibm_complex.h>

#endif /* _NEWTOS_COMPLEX_H */
