/*
 * malloc-host-gemdos.h -- forced into the host build of mintlib's malloc.c
 * (the overlay sdk/libc/overlay/mintlib/malloc.c) by malloc-host.sh
 * (TASK/MEM05 part B). <mint/osbind.h> is held back there
 * (-D_MINT_OSBIND_H: its GEMDOS macros are AArch64 `svc #1' traps); the one
 * GEMDOS call malloc.c makes -- Mfree of an emptied hunk in __free -- goes
 * to malloc-host.c with osbind.h's argument order. __sbrk (unix/sbrk.c, the
 * GEMDOS Malloc per hunk) is supplied by malloc-host.c as well.
 */
#ifndef MALLOC_HOST_GEMDOS_H
#define MALLOC_HOST_GEMDOS_H
long mh_Mfree (void *);
#define Mfree(p)	mh_Mfree ((void *) (p))
#endif
