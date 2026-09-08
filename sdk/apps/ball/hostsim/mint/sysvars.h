#ifndef HOST_SYSVARS_H
#define HOST_SYSVARS_H
#define _hz_200 ((const void *)0x4baL)
#define _vbclock ((const void *)0x462L)
/* The two the memory report reads. Same addresses the real header uses, so
 * ball.c names them the same way on both sides. */
#define phystop ((const void *)0x42eL)
#define ramtop  ((const void *)0x5a4L)
#define get_sysvar(x) host_sysvar((const void *)(x))
long host_sysvar(const void *v);
long host_hz200(void);
#endif
