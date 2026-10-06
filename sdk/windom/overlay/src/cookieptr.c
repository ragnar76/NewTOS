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
 * $Source: /cvsroot/windom/windom/src/cookieptr.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/02/24 20:30:04 $
 *   $Revision: 1.1 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/cookieptr.c (zView's patch does not
 * touch it). ONE change, at upstream line 30:
 *
 *   upstream:  return *((long *)0x5a0);
 *   here:      return (long)*((volatile unsigned int *)0x5a0);
 *
 * Why: _p_cookies at $5A0 is a 4-byte LONG. On LP64 `long` is 8 bytes, so
 * the upstream load fuses it with the sysvar at $5A4 (ramtop); measured
 * 0x3BE00000000005F0 (docs/sdk/05-abi-pitfalls.md B2). The 4-byte load
 * returns the jar mirror's address (0x5F0), as on the 68k. Since the
 * set_cookie.c/new_cookie.c overlays no WinDom code calls __cookieptr (the
 * only other caller, get_cookie.c's non-MiNTLib branch, is not compiled);
 * the function stays exported and correct.
 */


#include "globals.h"

long __cookieptr(void) { 
	return (long)*((volatile unsigned int *)0x5a0);
}
