/*
 * quickstat-host-gemdos.h -- forced into the host build of mintlib's
 * __quickstat by quickstat-host.sh (user decision 2026-10-02, TASK/PORT10:
 * native executable extensions). <mint/osbind.h> is held back there
 * (-D_MINT_OSBIND_H: its GEMDOS macros are AArch64 `svc #1' traps); the
 * _DTA layout and FA_* bits still come from mintlib's own <mint/ostruct.h>,
 * and the four GEMDOS calls __quickstat makes on its Fsfirst path are routed
 * to quickstat-host-fs.c with osbind.h's argument order.
 */
#ifndef QUICKSTAT_HOST_GEMDOS_H
#define QUICKSTAT_HOST_GEMDOS_H
#include <mint/ostruct.h>
_DTA *qs_test_Fgetdta (void);
void qs_test_Fsetdta (_DTA *);
long qs_test_Fsfirst (const char *, int);
short qs_test_Dgetdrv (void);
#define Fgetdta()		qs_test_Fgetdta ()
#define Fsetdta(dta)		qs_test_Fsetdta ((dta))
#define Fsfirst(spec, attr)	qs_test_Fsfirst ((spec), (attr))
#define Dgetdrv()		qs_test_Dgetdrv ()
#endif
