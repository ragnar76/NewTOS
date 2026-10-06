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
 * $Source: /cvsroot/windom/windom/src/objc_str3d.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/02/24 20:04:03 $
 *   $Revision: 1.1 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/objc_str3d.c (zView's patch does not
 * touch it). ONE change, upstream lines 32-38: `struct objcolor`, the
 * bit-field view ObjcStr3d() lays over TEDINFO.te_color to set its fill
 * colour, is declared in little-endian order on an `unsigned short`:
 *
 *   upstream:  unsigned borderc:4, textc:4, opaque:1, pattern:3, fillc:4
 *   here:      unsigned short fillc:4, pattern:3, opaque:1, textc:4, borderc:4
 *
 * Why: te_color is one WORD -- border colour bits 15-12, text colour 11-8,
 * opaque 7, pattern 6-4, fill colour 3-0. m68k compilers allocate bit-fields
 * from the most significant bit, so the upstream order hits exactly those
 * bits. clang on little-endian AArch64 allocates from the least significant
 * bit: the upstream order lands borderc on bits 3-0 and fillc on bits
 * 15-12, so `color->fillc = 8` would paint the BORDER colour instead of the
 * fill. Listing the fields from the low end puts every field back on its
 * documented bits, and the 2-byte unit keeps the view inside te_color (an
 * `unsigned` unit is 4 bytes and would reach into te_thickness; the same
 * kind of correction gemlib's BFOBSPEC needed:
 * sdk/gemlib/overlay/mt_gem.h.in, docs/sdk/05-abi-pitfalls.md B8/B11).
 */


#include <stddef.h>
#include <stdlib.h>
#include "globals.h"
#include "cookie.h"

struct objcolor {
	unsigned short fillc  :4;
	unsigned short pattern:3;
	unsigned short opaque :1;
	unsigned short textc  :4;
	unsigned short borderc:4;
};

void ObjcStr3d( OBJECT *tree) {
	int res;
		
	if( !get_cookie( MAGX_COOKIE, NULL)) {
		res = 0;
		while( !(tree[res].ob_flags & LASTOB)) {
			if( tree[res].ob_type == G_FTEXT &&
				!(tree[res].ob_state & (1<<15))) {
				struct objcolor *color;
			
				tree[res].ob_state |= SELECTED;
				tree[res].ob_flags |= (1<<9);
				tree[res].ob_flags |= (1<<10);
				color = (struct objcolor*)&tree[res].ob_spec.tedinfo->te_color;
				color -> fillc = 8;
			}
			res ++;
		}
	} else {
		/*  a faire */
		/* Editable FLAG10 ~FLAG9 */
		
	}
}
