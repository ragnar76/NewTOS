/*
 * NewTOS/AArch64 equivalent of z-tools-04805b7/zview/plugins/img/xp32.S
 * (z-tools commit 04805b7, 166 lines of m68k assembly) -- the project's
 * "genuine m68k assembly gets an AArch64 equivalent" exception
 * (TASK/ZVIEW-M4, sdk/zview/build-zview.sh: the IMG codec is linked with
 * this file instead of xp32.S, which cannot be assembled for AArch64).
 * Written in C, compiled for AArch64; imgldg.c calls it unchanged through
 * its own declaration (imgldg.c:4):
 *
 *   int32 plane2packed24( int32 no_words, int32 plane_length,
 *                         int16 no_planes, void *src, void *dst,
 *                         COLOR_MAP *palette);
 *
 * What the assembly does, and how each part is reproduced here (xp32.S line
 * numbers):
 *
 *  - Arguments (:13-19): the gcc cdecl stack frame, no_words, plane_length,
 *    no_planes, src, dst, palette. Plane k of the line starts at
 *    src + k * plane_length (:28-30 for planes 1-3; pp_8_to_24 reaches
 *    planes 4-7 at +4 * plane_length, :23-25 and :122-125).
 *  - Dispatch (:32-42, table :47-56): no_planes compared as an UNSIGNED
 *    word against 8 (cmpi.w / bhi); above 8 -> not_supported; otherwise the
 *    table: 1 -> pp_1_to_8, 4 -> pp_4_to_24, 8 -> pp_8_to_24, everything
 *    else -> not_supported. not_supported returns 0 (:59-61), the three
 *    converters return 1.
 *  - Word loop (:21-22, :77-78, :109-110, :163-164): d4 = no_words - 1, the body
 *    runs, then "subq.l #1,d4; bpl" -- a do-while: no_words iterations,
 *    and exactly one when no_words <= 0.
 *  - Each plane word is a big-endian "move.w (a0)+" (the line buffer holds
 *    the file's bytes), each pixel 16 times per word (dbra with 15), the
 *    most significant bit first ("add.w dn,dn" shifts the next bit into X).
 *  - pp_1_to_8 (:63-80): one output byte per pixel, 0 or 1.
 *  - pp_4_to_24 (:83-112): colour index = plane3 bit << 3 | plane2 << 2 |
 *    plane1 << 1 | plane0; pp_8_to_24 (:115-166): index = plane7 bit << 7 |
 *    ... | plane0. Output: three bytes R, G, B of the palette entry
 *    (:103-105, :154-156). The assembly addresses the entry as palette +
 *    index * 4 ("lsl.w #2" + "lea (a6,dn.w)"): that is &palette[index],
 *    because m68k-atari-mint-gcc gives the three-byte COLOR_MAP
 *    (plugins/common/zview.h) a size of 4 (struct size rounded to 16 bits;
 *    measured with /opt/cross-mint m68k-atari-mint-gcc 7.5.0, -m68020-60:
 *    sizeof = 4). On AArch64 sizeof(COLOR_MAP) is 3 and zView and the
 *    codecs share that layout, so the entry is palette[index].
 *  - Like the assembly, every word yields 16 pixels, also past the image
 *    width in the last word of a line; the caller's row buffer has room for
 *    that (pic_load.c:88, "width + 64" pixels).
 */
#include "zview.h"

static uint16 be_word( const uint8 *p)
{
	return ( uint16)(( p[0] << 8) | p[1]);
}

int32 plane2packed24( int32 no_words, int32 plane_length, int16 no_planes, void *src, void *dst, COLOR_MAP *palette)
{
	const uint8	*plane = ( const uint8*)src;
	uint8		*out = ( uint8*)dst;
	int32		n = no_words - 1, w = 0;
	int16		c, k;

	if( ( uint16)no_planes > 8)
		return 0;

	switch( no_planes)
	{
		case 1:
			do
			{
				uint16 p0 = be_word( plane + 2 * w);

				for( c = 0; c < 16; c++)
				{
					*out++ = ( uint8)(( p0 >> 15) & 1);
					p0 <<= 1;
				}
				w++;
			} while( --n >= 0);
			return 1;

		case 4:
		case 8:
			do
			{
				uint16 p[8];

				for( k = 0; k < no_planes; k++)
					p[k] = be_word( plane + ( int32)k * plane_length + 2 * w);

				for( c = 0; c < 16; c++)
				{
					uint16 index = 0;

					for( k = no_planes - 1; k >= 0; k--)
					{
						index = ( uint16)(( index << 1) | (( p[k] >> 15) & 1));
						p[k] <<= 1;
					}

					*out++ = palette[index].red;
					*out++ = palette[index].green;
					*out++ = palette[index].blue;
				}
				w++;
			} while( --n >= 0);
			return 1;

		default:
			return 0;
	}
}
