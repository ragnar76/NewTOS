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
 * $Source: /cvsroot/windom/windom/src/shel_write.c,v $
 *
 * CVS info:
 *   $Author: bercegeay $
 *   $Date: 2003/10/03 18:21:24 $
 *   $Revision: 1.2 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/shel_write.c (zView's patch does not
 * touch it). ONE change, at upstream line 65, in ShelWrite()'s VA_START
 * branch:
 *
 *   upstream:  *(char **)&msg[3] = file;
 *   here:      msg[3] = (INT16)((unsigned long)file >> 16);
 *              msg[4] = (INT16)(unsigned long)file;
 *
 * Why: VA_START carries the command line as a 32-bit address, high WORD in
 * msg[3], low WORD in msg[4] (AV protocol; WinDom's own call_stguide.c:42
 * sends VA_START with ADR(app.pipe) = hi, lo). On the 68k the 4-byte store
 * at &msg[3] writes exactly that. On LP64 little-endian AArch64 it is an
 * 8-byte store over msg[3..6] with the low WORD first -- a receiver reading
 * the protocol's hi/lo pair gets the halves swapped. User addresses are
 * below 2 GiB on NewTOS (docs/sdk/05-abi-pitfalls.md A2). The following
 * msg[5..7] = 0 stores are unchanged.
 */



#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include "globals.h"
#include "av.h"

/*
 *  Ex‚cuter une application GEM ou TOS en parallŠle 
 *  gere le protocol AV
 *  Sous MonoTos, on utilise Pexec() en
 *	g‚rant le repertoire courant.
 */

int ShelWrite( char *prg, char *cmd, void *env, int av, int single) {
	static char file[FILE_SIZE];
	char *p;
	int res, mode;
	char path[FILE_SIZE], path2[FILE_SIZE];
	int drv;

	strcpy( file, cmd);
	if( av) {
		INT16 id_app, msg[8];
		char aesname[9], name[64];

		/* extraire le nom du programme */
		p = strrchr( prg, '\\');
		strcpy( name, (p?(p+1):prg));
		p = strrchr( name, '.');
		if( p) *p = '\0';
		strupr( name);
		sprintf( aesname, "%-8s", name);
		id_app = appl_find( aesname);
		if( id_app >= 0) {
			msg[0] = VA_START;
			msg[1] = app.id;
			msg[2] = 0;
			msg[3] = (INT16)((unsigned long)file >> 16);
			msg[4] = (INT16)(unsigned long)file;
		    msg[5] = 0;
    	    msg[6] = 0;
    	    msg[7] = 0;
    	   	appl_write(id_app, 16, msg);
    	    return id_app;
		}
	}
	
	strcpy( file+1, cmd);
	*file = strlen(cmd);


		/* Sauver chemin courant */
		drv = Dgetdrv();
		Dgetpath( path, 0);
		
		/* Mettre le chemin courant a celui du programme */		
		if( prg[1] == ':')
			Dsetdrv( toupper(prg[0])-'A');
		p = strchr( prg, '\\');
		strcpy( path2, p?p:prg);
		p = strrchr( path2, '\\');
		if(p) *p = '\0';
		Dsetpath( path2);
	
	if( _AESnumapps != 1 && single == 0) {
		p = strchr( prg, '.');
		if( p) {
			if( !stricmp(p+1, "ttp") ||
				!stricmp(p+1, "tos"))
				mode = 0;
			else
				mode = 1;
		} else
			mode = 0; /* Cas Minix ?? */
		
		if( env) {
			SHELW shelenv;

			shelenv.newcmd=prg;
			shelenv.env=env;
			
			res = shel_write(0x801,mode,100, (char *)&shelenv, file);
		} else
			res = shel_write(1,mode,100, prg, file);
		
	} else {
		WINDOW *scan;
		INT16 x, y, w, h;
		
		/* Fermer toutes les fenˆtres et changer le chemin
		 * courant pour celui de l'application  */
		for( scan=wglb.first; scan; scan = scan->next) {
			if( scan->status & WS_OPEN) {
				WindClose( scan);
				scan->status |= WS_PEXEC;
			}
		}
		/* Enlever le menu  */
		if( app.menu) menu_bar( app.menu, 0);
		

		res=(int)Pexec( 0, prg, (void *) file, env);
		
		
		/* Le menu */
		if( app.menu) menu_bar( app.menu, 1);

		/* Les fenˆtres	*/
		for( scan=wglb.first; scan; scan = scan->next) {
			if( scan->status & WS_PEXEC) {
				WindGet( scan, WF_PREVXYWH, &x, &y, &w, &h);
				WindOpen( scan, x, y, w, h);
				scan->status &= ~WS_PEXEC;
			}
		}
	}

		/* Restaurer le chemin courant */
		Dsetdrv( drv);
		Dsetpath( path);

	return res;
}
