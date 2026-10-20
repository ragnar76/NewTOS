/*
 * stat-host-gemdos.h -- forced into the host build of the NewTOS
 * newtos-arch/do_stat.c (__sys_stat, __do_stat) by stat-host.sh
 * (TASK/SDK01 F: stat()/lstat() timestamps). <mint/osbind.h> and
 * <mint/mintbind.h> are held back there (-D_MINT_OSBIND_H,
 * -D_MINT_MINTBIND_H: their GEMDOS macros are AArch64 `svc #1' traps); the
 * _DTA layout and FA_* bits still come from mintlib's own <mint/ostruct.h>,
 * and the GEMDOS calls do_stat.c makes are routed to stat-host-fs.c with the
 * bindings' argument order.
 */
#ifndef STAT_HOST_GEMDOS_H
#define STAT_HOST_GEMDOS_H
#include <mint/ostruct.h>
_DTA *st_test_Fgetdta (void);
void st_test_Fsetdta (_DTA *);
long st_test_Fsfirst (const char *, int);
short st_test_Dgetdrv (void);
long st_test_Fstat64 (int, const char *, void *);
#define Fgetdta()		st_test_Fgetdta ()
#define Fsetdta(dta)		st_test_Fsetdta ((dta))
#define Fsfirst(spec, attr)	st_test_Fsfirst ((spec), (attr))
#define Dgetdrv()		st_test_Dgetdrv ()
#define Fstat64(flag, name, st)	st_test_Fstat64 ((flag), (name), (st))
#endif
