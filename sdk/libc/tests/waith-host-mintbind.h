/*
 * waith-host-mintbind.h -- forced into the host build of mintlib's
 * unix/wait4.c by waith-host.sh (TASK/PORT-WAITH). <mint/mintbind.h> is held
 * back there (-D_MINT_MINTBIND_H: its MiNT macros are AArch64 `svc #1'
 * traps); the two MiNT calls wait4.c makes go to waith-probe.c instead, with
 * mintbind.h's argument order.
 */
#ifndef WAITH_HOST_MINTBIND_H
#define WAITH_HOST_MINTBIND_H
long waith_test_Pwaitpid (int, int, long *);
long waith_test_Pdomain (int);
#define Pwaitpid(p, f, r)	waith_test_Pwaitpid ((p), (f), (r))
#define Pdomain(a)		waith_test_Pdomain ((a))
#endif
