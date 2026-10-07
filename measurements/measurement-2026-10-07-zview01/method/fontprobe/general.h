/* fontprobe: minimal stand-in for zview/general.h -- just what custom_font.c
 * needs, so the UNCHANGED draw_text() can run on the host. vrt_cpyfm records
 * the 1-plane source form exactly as the NewTOS VDI reads it (16-bit
 * host-order words, bit 0x8000 >> col = leftmost pixel; tos-api/src/vdi/
 * backend.rs MonoBits/MonoSrc) and renders the copied columns as ASCII. */
#include <string.h>
#include <stdio.h>
typedef short int16;
typedef struct { void *fd_addr; short fd_w, fd_h, fd_wdwidth, fd_stand, fd_nplanes, fd_r1, fd_r2, fd_r3; } MFDB;
#define WHITE 0
#define BLACK 1
void vrt_cpyfm(short h, short mode, short *pxy, MFDB *src, MFDB *dst, short *col);
