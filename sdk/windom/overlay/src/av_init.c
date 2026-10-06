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
 * $Source: /cvsroot/windom/windom/src/av_init.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/02/24 20:32:18 $
 *   $Revision: 1.1 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * base is windom-1.21.3/src/av_init.c WITH zView's patch applied (av_ping
 * copies the client name into app.pipe -- kept). ONE change, at upstream
 * line 82 (line 83 after zView's patch), in av_ping():
 *
 *   upstream:  strcpy( __av_server, *(char **)&msg[6]);
 *   here:      strcpy( __av_server, (char *)(((unsigned long)(UWORD)msg[6]
 *                                            << 16) | (UWORD)msg[7]));
 *
 * Why: VA_PROTOSTATUS carries the server's name as a 32-bit address in
 * msg[6] (high WORD) and msg[7] (low WORD) -- the AV protocol's layout, and
 * the one WinDom's own ADR(p) macro (windom.h) writes for AV_PROTOKOLL just
 * above. On the 68k the 4-byte load at &msg[6] is that value. On LP64
 * little-endian AArch64 it is an 8-byte load of msg[6..9] -- 4 bytes past
 * the end of the 8-WORD local buffer -- with the two WORDs swapped. User
 * addresses are below 2 GiB on NewTOS (docs/sdk/05-abi-pitfalls.md A2).
 */


#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "globals.h"
#include "av.h"

/*
 *	On cherche l'id du server AV :
 *		-> variable AVSERVER
 *		-> apps AVSERVER, GEMINI, THING
 *		-1: pas de server AV
 */

static
int av_server_id( void) {
	char *p, name[9];
	INT16 id=-1;
	
	shel_envrn(&p, "AVSERVER=");
	if( p) {
		sprintf(name, "%- 8s", p);
		id = appl_find( name);
	}	
	if( id != -1) return id;
	id = appl_find( "AVSERVER");
	if( id != -1) return id;
	id = appl_find( "GEMINI  ");
	if( id != -1) return id;
	id = appl_find( "THING   ");
	if( id != -1) return id;
	if( app.aes4 & AES4_APPSEARCH) {
		char name[9];
		INT16 type;
		appl_search( 0, name, &type, &id);
		
		if( appl_search( 0, name, &type, &id) == 0)
			if( type & 0x8) return 1;
		while( appl_search( 1, name, &type, &id))
			if( type & 0x8) return 1;
		id = -1;
	}
	return id;
}

/* sous fonction: interrogation du serveur */

static int av_ping( int status, long idle) {
	INT16 msg[8];
	
	strcpy( app.pipe, __av_name);
	ApplWrite( app.avid, AV_PROTOKOLL, status, 0, 0, ADR(app.pipe));
	if( AvWaitfor( VA_PROTOSTATUS, msg, idle)) {
		__av_status[0] = msg[3];
		__av_status[1] = msg[4];
		__av_status[2] = msg[5];
		strcpy( __av_server, (char *)(((unsigned long)(UWORD)msg[6] << 16) | (UWORD)msg[7]));
		return 1;
	} else
		return 0;
}

/*
 *	Initialisation du protocol AV
 *	id : id du client
 *	name: nom du client (format GEM)
 *	status: les actions support‚es par le client, peut ˆtre
 *	appel‚ plusieurs fois (avec un nouvel ‚tat)
 *			0x1: VA_SETSTATUS
 *			0x2: VA_START
 *			0x4: AV_STARTED
 *			0x8: VA_FONTCHANGED
 *			0x10: truc de quoting
 *			0x20: VA_PATH_UPDATE
 *	idle: temps max de r‚ponse du serveur en ms
 *			( 1000 semble pas mal)
 */

int AvInit( char *name, int status, long idle) {
	if( app.avid == -1) {
		app.avid = av_server_id();
		__av_name = strdup( name);
	}

	/* erreur, pas de server, bye */
	if( app.avid == -1) return -1 ;

	/* erreur, un client ne peut pas ˆtre son
	 * propre serveur, bye */
	if( app.avid == app.id) return -3 ;

	/* d‚claration du client au serveur AV */
	if( av_ping( status, idle))
		return app.avid;
	else
		return -2;	/* Ne supporte pas le protocol, bye */
}
