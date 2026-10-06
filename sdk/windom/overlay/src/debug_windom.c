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
 * $Source: /cvsroot/windom/windom/src/debug_windom.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/02/24 20:26:07 $
 *   $Revision: 1.1 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/debug_windom.c (zView's patch does not
 * touch it). ONE change, at upstream line 63, in the AP_DEBUG message:
 *
 *   upstream:  *(char **)&evnt.buff[4] = debug;
 *   here:      evnt.buff[4] = (INT16)((unsigned long)debug >> 16);
 *              evnt.buff[5] = (INT16)(unsigned long)debug;
 *
 * Why: the 68k store puts the 32-bit address in buff[4] (high WORD) and
 * buff[5] (low WORD), the layout every other pointer in a WinDom message
 * has (ADR(p) = hi, lo, windom.h). On LP64 little-endian AArch64 the same
 * expression is an 8-byte store over buff[4..7], low WORD first. User
 * addresses are below 2 GiB on NewTOS (docs/sdk/05-abi-pitfalls.md A2).
 */



#include <stdarg.h>	
#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include "globals.h"

void DebugWindom(  const char *format, ...) {
	char path[255];
	static char debug[255];
	char type[120], p;
	va_list arglist;
	
	p = ConfInquire( "windom.debug", "%s,%s", type, path);
	if( p>0) {
		/* format */
		va_start( arglist, format);
		vsprintf( debug, format, arglist);
		va_end( arglist);

		if( !stricmp( type, "alert")) {
			char alert[255];
			sprintf( alert, "[1][%s][Okay]", debug);
			form_alert( 1, alert);	
		} else
		if( !stricmp( type, "debug") && p == 2) {
			int id, ans = 0;
			
			if( _AESnumapps != 1)
				ShelWrite( path, "", NULL, 1, 0);
			id = appl_find( "DEBUG   ");
			if( id > 0) {
				evnt.buff[0] = AP_DEBUG;
				evnt.buff[1] = app.id;
				evnt.buff[2] = 0;
				evnt.buff[3] = wglb.front?wglb.front->handle:0;
				evnt.buff[4] = (INT16)((unsigned long)debug >> 16);
				evnt.buff[5] = (INT16)(unsigned long)debug;
				appl_write( id, 16, evnt.buff);
				/* On attend la r‚ponse */
				do {
					evnt_mesag( evnt.buff);
					if( evnt.buff[0] == AP_TERM) {
						ans = 1;
						ApplWrite( app.id, AP_TERM, 0, 0, 0, 0, 0);
					}
					if( evnt.buff[0] == AP_DEBUG)
						ans = 1;
				} while( !ans);
			}
		} else
		if( !stricmp( type, "log") && p == 2) {
			FILE *fp;
			static int first = 1;
			
			fp = fopen( path, first?"w":"a");
			first = 0;
			fputs( debug, fp);
			fputs( "\n", fp);
			fclose( fp);
		}
	}
}

/* EOF */
