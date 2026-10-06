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
 * $Source: /cvsroot/windom/windom/src/evnt_windom.c,v $
 * Module : main event handler of WinDom
 *
 * CVS info:
 *   $Author: bereziat $
 *   $Date: 2003/07/17 15:25:36 $
 *   $Revision: 1.5 $
 */

/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/windom/build-windom.sh). The
 * pristine file is windom-1.21.3/src/evnt_windom.c with zView's patch (which
 * does not touch it). ONE change, at upstream line 56:
 *
 *   upstream:  win->menu.root == *(OBJECT**) &evnt.buff[5]
 *   here:      win->menu.root == (OBJECT *)(((unsigned long)(UWORD)evnt.buff[5]
 *                                            << 16) | (UWORD)evnt.buff[6])
 *
 * Why: an MN_SELECTED message carries the menu tree as a 32-bit address in
 * two WORDs, high WORD in msg[5], low WORD in msg[6] (AES 4 convention;
 * EmuTOS gemctrl.c:355-358, docs/api/aes/menu.md "msg[5..7]"), and WinDom's
 * own senders write it that way too (ADR(menu) = hi, lo: menu_execcmd.c:95,
 * mnscroll.c:252,413). On the big-endian 68k a 4-byte load at &buff[5] is
 * exactly hi<<16|lo. On LP64 little-endian AArch64 the same expression is an
 * 8-byte load of buff[5..8] -- past the end of the 8-WORD message into
 * EVNTvar's next fields -- with the WORDs in the wrong order, so a window
 * menu's MN_SELECTED would never find its window. The arithmetic form is
 * the 68k meaning spelled portably; every NewTOS user address is below
 * 2 GiB (docs/sdk/05-abi-pitfalls.md A2), so 32 bits carry it whole.
 */


#include <stddef.h>
#include <stdlib.h>
#include "av.h"
#include "globals.h"
#include "cookie.h"

/*
 *	Traitement des ‚v‚nements messages - interne
 *	Valeur retourn‚e: 0 - continuer le traitement dans EvntWindom()
 *					  1 - non
 */

static 
int _do_evnt_msg( int *evnt_res) {
	INT16 top, dum;
	INT16 x, y, w, h;
	EV_MSG *msg;
	WINDOW *win;
	
	/* Here, we support the AES4 convention for MN_SELECTED object. 
     * This message (which can be binded to a window) does not have
	 * the window identificator in evnt.buff[3]. The way to find
	 * the targeted is different, so we process this case here */
	if( evnt.buff[0] == MN_SELECTED) {
		/* get first the menu desktop callback */
		msg = EvntFind( NULL, MN_SELECTED);
		/* We have to find the target window */
		for( win = wglb.first; win; win=win->next) {
			if( win->menu.root == (OBJECT *)(((unsigned long)(UWORD)evnt.buff[5] << 16) | (UWORD)evnt.buff[6])) {
				msg = EvntFind( win, MN_SELECTED);
				break;
			}
		} 
	} else {
		/* Other messages: we verify if the message addresses a window */
		dum = evnt.buff[0];
		msg = EvntFind( NULL, evnt.buff[0]);
		if( msg && (msg -> flags & EVM_ISWIN)) {
			win = WindHandle( evnt.buff[3]);
			msg = EvntFind( win, evnt.buff[0]);
		} else
			win = NULL;
		}

	/* Ev‚nements fenˆtres */
	if( win && msg) {
		
		/* 2/ Cas sp‚ciaux : ‚v‚nements sensibles ou
			  				 ‚mulation de certains messages */
		switch ( evnt.buff[0]) {
		case WM_REDRAW:
			_do_update( win);
			break;
		case WM_TOPPED:
			/* ‚mulation WF_BEVENT/BEVENT_WORK
			 * lorsque le systŠme ne le supporte pas */
			if( win->status & WS_UNTOPPABLE
				&& !(app.aes4 & AES4_BEVENT)) {
				wind_get( win->handle, WF_WORKXYWH, &x, &y, &w, &h);
				if( evnt.mx >= x && evnt.my >= y) {
					*evnt_res |= MU_BUTTON;
					evnt.mbut |= 0x01;
				}
				break;
			}
			/* Cas des fenˆtres modales */
			if( is_modal()) {
				EvntExec( wglb.appfront, WM_TOPPED);
				break;
			}
			/* s‚lection en arriŠre plan des menus et des toolbars */
			if( win->status & WS_MENU) {
				dum = objc_find( win->menu.root, 0, MAX_DEPTH, evnt.mx, evnt.my);
				if( dum != -1 && win->menu.root[dum].ob_type == G_TITLE ) {
					*evnt_res |= MU_BUTTON;
					evnt.mbut |= 0x01;
					break;
				}
			}
			if( win->status & WS_TOOLBAR) {
				dum = objc_find( win->tool.root, 0, MAX_DEPTH, evnt.mx, evnt.my);
				if( dum != -1 && win->tool.root[dum].ob_flags & (SELECTABLE|TOUCHEXIT) )
				{
					*evnt_res |= MU_BUTTON;
					evnt.mbut |= 0x01;
					break;
				}
			}

			if( win->status & WS_ICONIFY)
				std_tpd( win);
			else {				
				WindGet( win, WF_WORKXYWH, &x, &y, &w, &h);
				if( CONF(app)->flag & FLG_BUTMOUSE && IS_IN( evnt.mx, evnt.my, x, y, w, h)) {
					*evnt_res |= MU_BUTTON;
					evnt.mbut |= 0x01;
				} else 
					EvntExec( win, evnt.buff[0]);
			}
			break;

		case WM_BOTTOMED:
			if( !(win -> status & WS_MODAL))
				WindSet( win, WF_BOTTOM, evnt.buff[3], 0, 0, 0);
			break;
		
		case WM_ICONIFY:
			if( win -> status & WS_MODAL && win == wglb.appfront)
				/* on transforme le message */
				snd_msg( win, WM_ALLICONIFY, evnt.buff[4], evnt.buff[5], evnt.buff[6], evnt.buff[7]);
			else
				EvntExec( win, WM_ICONIFY);
			break;
		case WM_MOVED:
			/* Mettre … jour le menu */
			if( win -> status & WS_MENU &&
				!( SYSMENU(app) && win->attrib & MENUBAR) ) {
				wind_get( win->handle, WF_CURRXYWH, &x, &y, &dum, &dum);
				win->menu.root->ob_x += evnt.buff[4]-x;
				win->menu.root->ob_y += evnt.buff[5]-y;
			}
			/* Mettre … jour le toolbar */
			if( win -> status & WS_TOOLBAR ) {
				wind_get( win->handle, WF_CURRXYWH, &x, &y, &dum, &dum);
				win->tool.root->ob_x += evnt.buff[4]-x;
				win->tool.root->ob_y += evnt.buff[5]-y;
			}
			if( win -> status & WS_ICONIFY)
				std_mvd( win);
			else  {
				if( !(app.aes4 & AES4_BOTTOM) && (evnt.mkstate & 0x02 || evnt.mkstate & 0x01)) {
					snd_msg( win, WM_BOTTOMED,0,0,0,0);
					return 1;
				}
				EvntExec( win, evnt.buff[0]);
			}
			break;
		case WM_FULLED:
			if( win -> status & WS_MENU &&
				!( SYSMENU(app) && win->attrib & MENUBAR) ) {
				wind_get( win->handle, WF_CURRXYWH, &x, &y, &dum, &dum);
				win->menu.root->ob_x += evnt.buff[4]-x;
				win->menu.root->ob_y += evnt.buff[5]-y;
			}
			if( win -> status & WS_TOOLBAR ) {
				wind_get( win->handle, WF_CURRXYWH, &x, &y, &dum, &dum);
				win->tool.root->ob_x += evnt.buff[4]-x;
				win->tool.root->ob_y += evnt.buff[5]-y;
			}
			EvntExec( win, evnt.buff[0]);
			break;
		case WM_CLOSED:
			/* -> ‚mulation l'ic“nification <- */
			if( evnt.mkstate & (K_LSHIFT|K_RSHIFT|K_CTRL) 
				&& (app.aes4 & AES4_SMALLER) == 0 ) {
				give_iconifyxywh( &x, &y, &w, &h);
				if( evnt.mkstate & 0x03)
					snd_msg( win, WM_ICONIFY, x, y, w, h);
				else
					snd_msg( win, WM_ALLICONIFY, x, y, w, h);
			} else if( app.aes4 & AES4_APPLCONTROL 
					   && evnt.mkstate & (K_LSHIFT|K_RSHIFT)) {
				/* -> masquer application <- */
				ApplControl( app.id, APC_HIDE);
			} else {
				if( is_modal() && win != wglb.appfront)
					return 0;
				EvntExec( win, evnt.buff[0]);
			}
			break;
		
		/* ici pas de break, car on fait un EvntExec() apres  */
		case WM_FORM:
		case WM_TOOLBAR:
			if( *evnt_res & MU_BUTTON)
				*evnt_res &= ~MU_BUTTON;
		default:
			EvntExec( win, evnt.buff[0]);
			break;
		}
	} 

	/* Case of Menu binding are treated now : before
	 * the MN_SELECTED callback evaluation */
	if( evnt.buff[0] == MN_SELECTED) {
		if( win)
			menu_bind( win, evnt.buff[4], evnt.buff[3]);
		else if( app.mnbind)
			menu_bind( NULL, evnt.buff[4], evnt.buff[3]);	
	}

	/* Ev‚nements autres */
	if( !win && msg)
		EvntExec( NULL, evnt.buff[0]);

	/* Ev‚nements sp‚ciaux */
	if( !msg) {
	 	switch( evnt.buff[0]) {
		case AP_BUTTON:
			/* WM_BUTTON devient un MU_BUTTON */
			*evnt_res 	 &= ~MU_MESAG;
			*evnt_res 	 |= MU_BUTTON;
			evnt.mx   	  = evnt.buff[3];
			evnt.my   	  = evnt.buff[4];
			evnt.mbut 	  = evnt.buff[5];
			evnt.mkstate  = evnt.buff[6];
			evnt.nb_click = evnt.buff[7];
			break;
		case AP_KEYBD:
			/* WM_KEYBD devient un MU_KEYBD */
			*evnt_res 	&= ~MU_MESAG;
			*evnt_res 	|= MU_KEYBD;
			evnt.mkstate = evnt.buff[3];
			evnt.keybd 	 = evnt.buff[4];
			break;
		}
	} 

	/* Gestion de la palette */
	/* WM_UNTOPPED = on passe en arriŠre plan
	 * WM_ONTOP	   = une fenetre d'une autre application passe
	 *	   			 en premier plan (cas multi application) 
	 */
	if( !(CONF(app)->flag & FLG_NOPAL) && (
		   evnt.buff[ 0] == WM_UNTOPPED 
		|| evnt.buff[ 0] == WM_ONTOP 
		|| evnt.buff[ 0] == WM_TOPPED)) {
		
		wind_get( 0, WF_TOP, &top, &dum, &dum, &dum);
		if ( (win = WindHandle( top)) == NULL) 	/* C'est pas … nous */
			w_setpal( app.palette);	
		else
			w_setpal( win -> graf . palette);
	}

	return 0;
}

/***********************************************************************/

int EvntWindom( int evntflag) {
	int evnt_res;
	INT16 top, dum;
	int mu_button=0;
	int mu_keybd=0;
	WINDOW *win, *mwin;
	INT16 x, y, w, h;
	EV_MSG *evmsg;

	/*
	 * Sont consider‚ comme messages:
	 *	- le menu systeme
	 *	- les menus en fenetres
	 *	- les toolbars
	 *  - les formulaires
	 *  - les raccourcis claviers
	 *	MU_BUTTON et MU_KEYBD sont impos‚s pour ces ‚venements
	 *		aussi il n'y a pas de message MU_BUTTON ou MU_KEYBD
	 *		retourn‚.
	 *  le mˆme travail est	 … faire pour EvntMesag 
	 */
	
	if( (evntflag & MU_BUTTON) == 0) {
		mu_button = 1;
		evntflag |= MU_BUTTON;
	}
	if( (evntflag & MU_KEYBD) == 0) {
		mu_keybd = 1;
		evntflag |= MU_KEYBD;
	}


EVNT:
	evnt_res = evnt_multi( evntflag,
			evnt.bclick, evnt.bmask, evnt.bstate,
			evnt.m1_flag, evnt.m1_x, evnt.m1_y, evnt.m1_w, evnt.m1_h,
			evnt.m2_flag, evnt.m2_x, evnt.m2_y, evnt.m2_w, evnt.m2_h, 
			evnt.buff,
			evnt.timer,
			&evnt.mx, &evnt.my, &evnt.mbut, &evnt.mkstate,
			&evnt.keybd, &evnt.nb_click);

	if(	(evnt_res & MU_MESAG) && (_do_evnt_msg( &evnt_res) == 1))
		goto EVNT;
	
	/* fenˆtre frontale */
	wind_get( 0, WF_TOP, &top, &dum, &dum, &dum);
	wglb.front = WindHandle( top);


	/* fenˆtre sous la souris */
	mwin = WindHandle( wind_find( evnt.mx, evnt.my));

	/* Gestion des ‚venements boutons */

	if( evnt_res & MU_BUTTON) {

		/* Cas d'une fenˆtre modale */
		if( is_modal() && mwin != wglb.appfront) {
			/* On toppe la fenˆtre modale */
			snd_msg( wglb.appfront, WM_TOPPED, 0, 0, 0, 0);
			goto ICI;
		}

		evmsg = EvntFind( mwin, WM_XBUTTON);
				
		if( mwin && evnt.mbut & 0x01) {	
			/* Cas d'une fenˆtre iconifi‚e */
			if( mwin ->status & WS_ICONIFY) {
				if( evnt.nb_click == 2) {
					WindGet( mwin, WF_UNICONIFY, &x, &y, &w, &h);
					snd_msg( mwin, WM_UNICONIFY, x, y, w, h);
				}
				goto ICI;
			}
			/* On clique sur un menu */
			if( mwin -> status & WS_MENU && is_menu( mwin)) {
#ifdef BETAMENU
				int frm_menu_beta( WINDOW *);
				if( frm_menu_beta( mwin) != -1)
#else
				if( frm_menu( mwin) != -1)
#endif
					goto ICI; /* on passe a la suite du traitement */
				else
					EvntExec( mwin, WM_TOPPED); /* on top si on ne selectionne rien */
			}
			/* On clique sur un toolbar */
			if( mwin -> status & WS_TOOLBAR) {
				int res;

#if GFORM
				res = frm_buttn_ev( mwin, OC_TOOLBAR);
#else
				res = objc_find( mwin->tool.root, ROOT, MAX_DEPTH, 
								 evnt.mx, evnt.my);
#endif
				 
				if( res != -1) {
					snd_msg( mwin, WM_TOOLBAR, res, evnt.keybd, 0, 0);
					goto ICI;
				}
			}
			/* formulaire */
			if( mwin -> status & WS_FORM) {
				WindGet( mwin, WF_WORKXYWH, &x, &y, &w, &h);
				if( IS_IN(evnt.mx, evnt.my, x, y, w, h)) {
					EvntExec( mwin, WM_XBUTTON);
					goto ICI;
				}
			}
		}

		/* Les autres cas */
		if( !mu_button && mwin && evmsg) {
			WindGet( mwin, WF_WORKXYWH, &x, &y, &w, &h);
			if( IS_IN(evnt.mx, evnt.my, x, y, w, h))
				EvntExec( mwin, WM_XBUTTON);
		}
		
		/* Pour que EvntAttach( NULL) fonctionne  */
		EvntExec( NULL, WM_XBUTTON);
	}

ICI:
	/* Gestion des ‚v‚nements clavier */
	if( evnt_res & MU_KEYBD) {

		/* Gestion du mode FLG_KEYMOUSE */
		if( CONF(app)->flag & FLG_KEYMOUSE)
			win = WindHandle( wind_find( evnt.mx, evnt.my));
		else
			win = wglb.front;

		evmsg = EvntFind( win, WM_XKEYBD);

		/* Cas d'une fenˆtre modale */
		if( is_modal() && win != wglb.appfront)
			win = NULL;

		/* Gestion des raccourcis clavier */
		if( !(CONF(app)->flag & FLG_NOKEYMENU) 
			&& (menu_exec_cmd( win) || menu_exec_cmd( NULL)))
			;
		else if( win && win->status & WS_TOOLBAR
			     && (dum = frm_keybd_ev( win->tool.root)) != -1) {
			objc_offset( win->tool.root, dum, &w, &h);
			x = evnt.mx; y = evnt.my;
			evnt.mx = w + win->tool.root[dum].ob_width/2-1;
			evnt.my = h + win->tool.root[dum].ob_height/2-1;
			dum = frm_buttn_ev( win, OC_TOOLBAR);
			evnt.mx = x; evnt.my = y;
			if( dum != -1) {
				snd_msg( win, WM_TOOLBAR, dum, evnt.keybd, 0, 0);
			}
		} else if ( win && (!mu_keybd || win->status & WS_FORM)
					    && evmsg)
			EvntExec( win, WM_XKEYBD);

		/* Pour que EvntAttach( NULL) fonctionne  */
		EvntExec( NULL, WM_XKEYBD);
	}

	
	if( evnt_res & MU_TIMER) {
		win = wglb.first;
		while (1) {
			EvntExec( win, WM_XTIMER);
			if( !win) break;
			win = win -> next;
		}
	}
	
	if( evnt_res & MU_M1) {
		win = wglb.first;
		while (1) {
			EvntExec( win, MU_M1);
			if( !win) break;
			win = win -> next;
		}
	}

	if( evnt_res & MU_M2) {
		win = wglb.first;
		while (1) {
			EvntExec( win, MU_M2);
			if( !win) break;
			win = win -> next;
		}
	}
	
	if( mu_button) 	evnt_res &= ~MU_BUTTON;
	if( mu_keybd) 	evnt_res &= ~MU_KEYBD;
	return evnt_res;
}
