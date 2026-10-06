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
 * $Source: /cvsroot/windom/windom/src/set_cookie.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/02/24 20:00:17 $
 *   $Revision: 1.1 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/set_cookie.c (zView's patch does not
 * touch it). The body of set_cookie() (upstream lines 42-56) walks the jar
 * itself; here it goes through the jar's owner instead:
 *
 *   found  = Getcookie(cookie, &old) == C_FOUND      (mintlib, SDK libc)
 *   if found && value != 0:  Ssystem(S_SETCOOKIE, cookie, value)
 *   return found ? 1 : 0                             (unchanged contract)
 *
 * Why (docs/sdk/05-abi-pitfalls.md B2): the upstream walk reads _p_cookies
 * with an 8-byte load (`*(long *)0x5a0` in __cookieptr: fuses _p_cookies
 * with the next sysvar on LP64) and steps the jar with a 16-byte stride
 * (`long` = 8 bytes; a jar slot is two 32-bit LONGs), so it finds nothing
 * or reads past the terminator. Even a correct walk would only write the
 * per-process page-0 MIRROR of the jar, which the kernel rewrites from its
 * own jar and never reads back (loader.rs populate_sysvars) -- and WinDom's
 * get_cookie() (get_cookie.c, __MINTLIB__ branch, compiled here) reads the
 * kernel jar through Getcookie(), so it would not see the change. The jar's
 * contract on NewTOS is Ssystem S_GETCOOKIE/S_SETCOOKIE
 * (tos-api/src/gemdos/ssystem.rs); that is what is used. Callers: the
 * BubbleGEM paths (bubble_call.c:50,63, bubble_modal.c:50,88), reached only
 * when a BubbleGEM server is running.
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
#include <mint/cookie.h>

int set_cookie(long cookie, long value) {
	long old;

	if (Getcookie(cookie, &old) != C_FOUND)
		return( 0);
	if (value)
		(void)Ssystem(S_SETCOOKIE, cookie, value);
	return( 1);
}
