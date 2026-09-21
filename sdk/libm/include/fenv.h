/*
 * fenv.h -- NewTOS SDK adapter: the standard <fenv.h> name on top of
 * OpenLibm's <openlibm_fenv.h>, which selects openlibm_fenv_aarch64.h (FPCR/
 * FPSR access; the out-of-line functions are in libm.a from aarch64/fenv.c).
 * See math.h in this directory for why the adapters exist.
 */
#ifndef _NEWTOS_FENV_H
#define _NEWTOS_FENV_H 1

#include <openlibm_fenv.h>

#endif /* _NEWTOS_FENV_H */
