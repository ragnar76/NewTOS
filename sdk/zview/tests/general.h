/*
 * general.h stand-in for sdk/zview/tests/custom-font-host.c (TASK/ZVIEW01):
 * exactly what zview/custom_font.c needs to compile on the host -- int16,
 * MFDB, the two pen names it uses and a recording vrt_cpyfm. The real
 * zview/general.h pulls in WinDom, gemlib and mintlib, none of which the
 * host has.
 */
#ifndef ZVIEW_HOST_GENERAL_H
#define ZVIEW_HOST_GENERAL_H
#include <string.h>
typedef short int16;
typedef struct
{
	void *fd_addr;
	short fd_w, fd_h, fd_wdwidth, fd_stand, fd_nplanes, fd_r1, fd_r2, fd_r3;
} MFDB;
#define WHITE 0
#define BLACK 1
void vrt_cpyfm( short handle, short mode, short *pxy, MFDB *src, MFDB *dst, short *color);
#endif
