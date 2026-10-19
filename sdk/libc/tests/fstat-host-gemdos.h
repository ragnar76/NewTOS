/*
 * fstat-host-gemdos.h -- forced into the host build of the lean
 * newtos-arch/do_fstat.c and of mintlib's own mintlib/isatty.c by
 * fstat-host.sh (TASK/FS08 part 4, TASK/LIBC-TTY). <mint/osbind.h> and
 * <mint/mintbind.h> are held back there (-D_MINT_OSBIND_H, -D_MINT_MINTBIND_H:
 * their GEMDOS macros are AArch64 `svc #1' traps); the GEMDOS calls the two
 * files make are routed to fstat-host-fs.c with the bindings' argument order:
 *   Fseek (offset, handle, mode)  -- __do_fstat and __isatty
 *   Fcntl (handle, arg, cmd)      -- __isatty's TIOCGPGRP probe
 *   Ffstat64 (handle, stat)       -- __sys_fstat (TASK/SDK01 A)
 */
#ifndef FSTAT_HOST_GEMDOS_H
#define FSTAT_HOST_GEMDOS_H
long fh_test_Fseek (long, short, short);
long fh_test_Fcntl (short, void *, short);
#define Fseek(offset, handle, mode)	fh_test_Fseek ((offset), (handle), (mode))
#define Fcntl(handle, arg, cmd)		fh_test_Fcntl ((handle), (arg), (cmd))
long fh_test_Ffstat64 (short, void *);
#define Ffstat64(handle, stat)		fh_test_Ffstat64 ((handle), (stat))
#endif
