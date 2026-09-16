/*
 * Minimal stand-in for QED's src/global.h, holding exactly what set.c needs.
 * The three SET lines are quoted VERBATIM from sdk/apps/qed/src/types.h:13-16
 * so the host build sees the same declarations as the target build.
 */
#ifndef _qed_global_h_
#define _qed_global_h_

#include <string.h>
#include <stddef.h>
#include <stdbool.h>

#ifndef FALSE
#  define FALSE 0
#  define TRUE  1
#endif

/* ---- verbatim from sdk/apps/qed/src/types.h:13-16 ---------------------- */
#define SETSIZE	32							/* Size of a set in longs */
#define SETMAX		(SETSIZE*32 - 1)		/* Max count of elements of a set */

typedef unsigned long	SET[SETSIZE];
/* ----------------------------------------------------------------------- */

#endif
