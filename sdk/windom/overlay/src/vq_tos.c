/*
 * WinDom: a high level GEM library
 * Copyright (c) 1997-2003 Dominique Bereziat & Arnaud Bercegeay
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, write to the Free Software
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307 USA
 *
 * $Source: /cvsroot/windom/windom/src/vq_tos.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/03/27 18:53:22 $
 *   $Revision: 1.3 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/vq_tos.c (zView's patch does not touch
 * it). ONE change, at upstream line 64, in syshead():
 *
 *   upstream:  return (long)*((SYSHDR **)0x04f2L);
 *   here:      return (long)*((volatile unsigned int *)0x04f2L);
 *
 * Why: _sysbase at $4F2 is a 4-byte pointer on every TOS. On LP64 a
 * `SYSHDR *` is 8 bytes, so the upstream load fuses _sysbase with the two
 * WORDs after it and vq_tos() would dereference a wild address. The 4-byte
 * load yields the OSHEADER the loader materialises in page 0 (0x5C0,
 * docs/sdk/05-abi-pitfalls.md A2/A3). vq_tos() then reads os_version at
 * offset 2, which mintlib's OSHEADER has at the same offset on LP64 (only
 * fields from `reseth` on move: 05-abi-pitfalls.md B1). Caller: fsel.c:494
 * (the file-selector title needs TOS >= 1.04).
 */


#include <stdlib.h>
#include "globals.h"

/* Get the definition of OSHEADER for vq_tos() */
#ifdef __SOZOBONX__
	#include <osheader.h>
	#define SYSHDR	OSHEADER
#else
	#ifdef __GNUC__
		/* MiNTLib: from release 0.57.3, sysvars.h header is in the mint subdirectory */
		#ifndef __MINTLIB_MAJOR__ /* old mintlib */
					#define _SYSVAR_OLDLOCATION 1
		#else
			#if __MINTLIB_MAJOR__ == 0 && __MINTLIB_MINOR__ < 57
					#define _SYSVAR_OLDLOCATION 1
			#else
				#if __MINTLIB_MAJOR__ == 0 && __MINTLIB_MINOR__ == 57 && __MINTLIB_REVISION__ < 3
					#define _SYSVAR_OLDLOCATION 1
				#endif
			#endif
		#endif
		
		#if _SYSVAR_OLDLOCATION
			#include <sysvars.h>
		#else
			#include <mint/sysvars.h>
		#endif
		
		#define SYSHDR	OSHEADER
 	#endif 
#endif


/********************* TOS  ***************************/

static 
long syshead( void) {
	return (long)*((volatile unsigned int *)0x04f2L);
}

/*
 * return TOS version
 */

int vq_tos( void) {
	SYSHDR *sys = (SYSHDR *) Supexec( syshead);
	return sys->os_version;
}
