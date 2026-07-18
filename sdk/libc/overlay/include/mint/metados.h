/*
	Metados

	Copyright (C) 2002	Patrice Mandin

	This library is free software; you can redistribute it and/or
	modify it under the terms of the GNU Lesser General Public
	License as published by the Free Software Foundation; either
	version 2.1 of the License, or (at your option) any later version.

	This library is distributed in the hope that it will be useful,
	but WITHOUT ANY WARRANTY; without even the implied warranty of
	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
	Lesser General Public License for more details.

	You should have received a copy of the GNU Lesser General Public
	License along with this library; if not, see
    <https://www.gnu.org/licenses/>.
*/

#ifndef _METADOS_H
#define _METADOS_H

#include <mint/falcon.h>	/* for trap_14_xxx macros */

__BEGIN_DECLS

#ifndef OSBIND_CLOBBER_LIST
#define OSBIND_CLOBBER_LIST	"d1", "d2", "a0", "a1", "a2", "cc", "memory"
#endif

#ifndef trap_14_wwlwl
/*
 * NewTOS (AArch64) retarget -- reuses the __newtos_trapN helpers from
 * <mint/osbind.h>. Each argument -> one register (x0..x7), fn id -> x8,
 * result <- x0; `w`/`l` chars pick the per-position (short)/(long) cast.
 */
#define trap_14_wwlwl(n, a, b, c, d)					\
	__newtos_trap4(14, (n), (short)(a), (long)(b), (short)(c), (long)(d))
#endif

#ifndef trap_14_wwwll
#define trap_14_wwwll(n, a, b, c, d)					\
	__newtos_trap4(14, (n), (short)(a), (short)(b), (long)(c), (long)(d))
#endif

/*--- Functions prototypes ---*/

#define Metainit(buffer)	\
	(void)trap_14_wl((short)0x30,(long)buffer)
#define Metaopen(drive,buffer)	\
	(long)trap_14_wwl((short)0x31,(short)drive,(long)buffer)
#define Metaclose(drive)	\
	(void)trap_14_ww((short)0x32,(short)drive)
#define Metaread(drive,buffer,first_block,nb_blocks)	\
	(long)trap_14_wwllw((short)0x33,(short)drive,(long)buffer,(long)first_block,(short)nb_blocks)
#define Metawrite(drive,buffer,first_block,nb_blocks)	\
	(long)trap_14_wwllw((short)0x34,(short)drive,(long)buffer,(long)first_block,(short)nb_blocks)
#define Metaseek(drive,dummy,offset)	\
	(long)trap_14_wwll((short)0x35,(short)(drive),(long)(dummy),(long)(offset))
#define Metastatus(drive,buffer)	\
	(long)trap_14_wwl((short)0x36,(short)(drive),(long)(buffer))
#define Metaioctl(drive,magic,opcode,buffer)	\
	(long)trap_14_wwlwl((short)(0x37),(short)(drive),(long)(magic),(short)(opcode),(long)(buffer))

#define Metastartaudio(drive,dummy,metatracks_t_p)	\
	(long)trap_14_wwwl((short)(0x3b),(short)(drive),(short)(dummy),(long)(metatracks_t_p))
#define Metastopaudio(drive)	\
	(long)trap_14_ww((short)(0x3c),(short)(drive))
#define Metasetsongtime(drive,dummy,start,end)	\
	(long)trap_14_wwwll((short)(0x3d),(short)(drive),(short)(dummy),(long)(start),(long)(end))
#define Metagettoc(drive,dummy,metatocentry_t_p)	\
	(long)trap_14_wwwl((short)(0x3e),(short)(drive),(short)(dummy),(long)(metatocentry_t_p))
#define Metadiscinfo(drive,metadiscinfo_t_p)	\
	(long)trap_14_wwl((short)(0x3f),(short)(drive),(long)(metadiscinfo_t_p))

#define METADOS_IOCTL_MAGIC	0x4643544CL /* 'FCTL' */

#define CDROM_LEADOUT_CDAR	0xa2

/*--- Types ---*/

typedef struct {
	unsigned short version;
	unsigned long magic;
	char *log2phys;
} metainfo_t __attribute__((packed));

typedef struct {
	unsigned long drives_map;
	char *version;
	unsigned long reserved;		
	metainfo_t *info;
} metainit_t __attribute__((packed));

typedef struct {
	char *name;
	unsigned long reserved[3];
} metaopen_t __attribute__((packed));

typedef struct {
	char ext_status[32];
} metastatus_t __attribute__((packed));

typedef struct {
	unsigned char count;
	unsigned char first;
} metatracks_t __attribute__((packed));

typedef struct {	/* TOC entry for MetaGetToc() function */
	unsigned char track;
	unsigned char minute;
	unsigned char second;
	unsigned char frame;
} metatocentry_t __attribute__((packed));

typedef struct {
	unsigned char reserved;
	unsigned char minute;
	unsigned char second;
	unsigned char frame;
} metadisc_msf_t __attribute__((packed));

typedef struct {	/* Discinfo for MetaDiscInfo() function */
	unsigned char disctype, first, last, current;
	metadisc_msf_t	relative;
	metadisc_msf_t	absolute;
	metadisc_msf_t	end;
	unsigned char index, reserved1[3];
	unsigned long reserved2[123];
} metadiscinfo_t __attribute__((packed));

__END_DECLS

#endif /* _METADOS_H */
