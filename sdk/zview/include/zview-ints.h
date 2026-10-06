/*
 * zview-ints.h -- the integer types of z-tools' common/types2b.h for an LP64
 * build of zView and its codecs (TASK/ZVIEW-M4, sdk/zview/build-zview.sh).
 *
 * Every zView and codec translation unit is compiled with
 *
 *     -DHAVE_INTS_DEFINED -include sdk/zview/include/zview-ints.h
 *
 * types2b.h declares int8/uint8/int16/uint16/int32/uint32 only when
 * HAVE_INTS_DEFINED is NOT set (types2b.h:4-9 and :13-16) -- the hook the
 * upstream header itself offers for a platform whose own widths differ. Its
 * m68k typedefs are "signed long"/"unsigned long" for int32/uint32
 * (types2b.h:7-8): 32 bits on the 68k, 64 bits on LP64 AArch64. The names say
 * what the code means -- 32-bit quantities: the Eureka RAW palette read as
 * int32[768] = 3072 bytes (plugins/eureka/rawldg.c:61,117), the 32-bit
 * truecolour pixels zView's raster code writes (zvdi/raster.c), the 200 Hz
 * tick read as *(int32 *)0x4ba (chrono.c:25,45), and img_info / IMAGE fields
 * shared by zView and every codec (plugins/common/zview.h, general.h). So the
 * widths are pinned here to the m68k ones; the source stays unmodified.
 *
 * fix31 (types2b.h:10) and int64/uint64 (:18-19) keep the upstream
 * definitions, which are written in terms of these types or of long long.
 * <stdint.h> is not used: it is not on every include path of the codecs
 * (mintlib's freestanding headers), and these four widths are fixed for both
 * AArch64 ABIs the SDK knows (int = 32, short = 16, char = 8).
 */
#ifndef NEWTOS_ZVIEW_INTS_H
#define NEWTOS_ZVIEW_INTS_H

typedef signed char		int8;
typedef unsigned char	uint8;
typedef signed short	int16;
typedef unsigned short	uint16;
typedef signed int		int32;
typedef unsigned int	uint32;

#endif
