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
 * $Source: /cvsroot/windom/windom/src/new_cookie.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/02/24 20:04:08 $
 *   $Revision: 1.1 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/new_cookie.c (zView's patch does not
 * touch it). The body of new_cookie() (upstream lines 42-62) appends a slot
 * to the jar by hand; here it asks the jar's owner:
 *
 *   return Ssystem(S_SETCOOKIE, id, value) == 0 ? 1 : 0;
 *
 * Why: the same three defects as set_cookie.c (see its header; 05-abi-
 * pitfalls.md B2): an 8-byte read of _p_cookies, a 16-byte stride over
 * 8-byte jar slots, and a write into the per-process page-0 mirror that
 * never reaches the kernel jar get_cookie() reads. S_SETCOOKIE adds the
 * cookie (FreeMiNT semantics, tos-api/src/gemdos/ssystem.rs) and fails only
 * on an invalid tag or a full jar -- the upstream "no room" case returns 0
 * as before. One difference, recorded rather than hidden: when the tag is
 * ALREADY in the jar, upstream appends a second, shadowed slot (get_cookie
 * keeps finding the old value) while S_SETCOOKIE updates the one slot.
 * new_cookie() has no caller inside WinDom.
 */


#ifdef __PUREC__
	/* Pure C possäde des librairies non standard */
	#include <tos.h>
#else
	/* Gnu C, Sozobon, Lattice */
#ifdef __SOZOBONX__
#include <atari.h>  /* MUST be included before osbind.h */
#endif
	#include <osbind.h>
	#include <fcntl.h>
#endif
#include <stdio.h>
#include <stdlib.h>
#include "globals.h"

#include <mint/mintbind.h>
#include <mint/ssystem.h>

int new_cookie (long id, long value) {
	return (Ssystem(S_SETCOOKIE, id, value) == 0) ? 1 : 0;
}
