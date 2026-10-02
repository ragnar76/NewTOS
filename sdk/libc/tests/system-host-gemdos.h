/*
 * system-host-gemdos.h -- forced into the host build of the system() overlay
 * by system-host.sh (TASK/PORT10). <mint/osbind.h> is held back there
 * (-D_MINT_OSBIND_H: its GEMDOS macros are AArch64 `svc #1' traps), and the
 * six GEMDOS calls posix/system.c makes are routed to system-host.c's
 * recorders instead, with osbind.h's argument order.
 */
#ifndef SYSTEM_HOST_GEMDOS_H
#define SYSTEM_HOST_GEMDOS_H
long system_test_Fopen (const char *, int);
long system_test_Fcreate (const char *, int);
long system_test_Fdup (int);
long system_test_Fforce (int, int);
long system_test_Fclose (int);
long system_test_Fseek (long, int, int);
#define Fopen(p, m)	system_test_Fopen ((p), (m))
#define Fcreate(p, a)	system_test_Fcreate ((p), (a))
#define Fdup(h)		system_test_Fdup ((h))
#define Fforce(s, h)	system_test_Fforce ((s), (h))
#define Fclose(h)	system_test_Fclose ((h))
#define Fseek(o, h, w)	system_test_Fseek ((o), (h), (w))
#endif
