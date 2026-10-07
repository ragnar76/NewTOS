/*
 * fstat-host-gemdos.h -- forced into the host build of the lean
 * newtos-arch/do_fstat.c by fstat-host.sh (TASK/FS08 part 4). <mint/osbind.h>
 * is held back there (-D_MINT_OSBIND_H: its GEMDOS macros are AArch64
 * `svc #1' traps); the one GEMDOS call __do_fstat makes, Fseek, is routed to
 * fstat-host-fs.c with osbind.h's argument order (offset, handle, mode).
 */
#ifndef FSTAT_HOST_GEMDOS_H
#define FSTAT_HOST_GEMDOS_H
long fh_test_Fseek (long, short, short);
#define Fseek(offset, handle, mode)	fh_test_Fseek ((offset), (handle), (mode))
#endif
