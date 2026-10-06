/*
 * NewTOS/AArch64 overlay (TASK/LDG-M3, sdk/ldg/build-ldg-client.sh). The
 * pristine file is <LDG r133>/demo/client/client.c. The ONLY change: the
 * module file names the client opens end in ".nld" instead of ".ldg" --
 * four string literals, upstream lines 153, 155 (LdgQuit) and 173-174
 * (main):
 *
 *   upstream:  "..\\libs\\ex_lib.ldg"   "..\\libs\\popup.ldg"
 *   here:      "..\\libs\\ex_lib.nld"   "..\\libs\\popup.nld"
 *
 * Why: a native LDG module is a NewTOS ELF and is named .NLD; .LDG is
 * reserved for m68k modules and the later emulator (ADR-018 amendment
 * 2026-10-06, docs/MEMORY.md, item 2). Item 4 of that amendment settles
 * clients that name modules by suffix: a toolchain overlay, the application
 * source stays pristine -- this file. The suffix is the only thing a C
 * program cannot be told from outside; every line of client logic, the
 * relative "..\\libs\\" layout included, is upstream's.
 */
/*
 * LDG 2.xx demo program
 *
 * Example of LDG-client (using WinDom & Gemlib 43) 
 * D.B‚r‚ziat (c) 2003
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windom.h>
#include <ldg.h>
#ifdef __PUREC__
#include <ldg\ex_lib.h>
#include <ldg\popup.h>
#else
#include <ldg/ex_lib.h>
#include <ldg/popup.h>
#endif
#include "client.h"

/* Global variables
 */

LDG *ldg_math, *ldg_popup;
#define OPE_INFO	1
#define OPE_SIN		2
#define OPE_COS		3
int ope = OPE_INFO;

OBJECT *get_tree( int ind) {
	OBJECT *tree;
	rsrc_gaddr( 0, ind, &tree);
	return tree;
}

/* Compute button */

void compute( WINDOW *win, int index) {
	Real r;
	double f;
	
	switch( ope) {
	case OPE_INFO:
		FormAlert( 1, mathinfo());
		break;
	case OPE_COS:
		r.value = atof(ObjcString(FORM(win),INPUT,NULL));
		f = cosinus( &r);
		sprintf( ObjcString(FORM(win), INPUT, NULL), "%5f", (float)f);
		ObjcDraw( OC_FORM, win, INPUT, 0);
		break;
	case OPE_SIN:
		r.value = atof(ObjcString(FORM(win),INPUT,NULL));
		f = sinus( &r);
		sprintf( ObjcString(FORM(win), INPUT, NULL), "%5f", (float)f);
		ObjcDraw( OC_FORM, win, INPUT, 0);
		break;
	}
	ObjcChange( OC_FORM, win, index, ~SELECTED, TRUE);
}

/* Call popup */

void call_popup( WINDOW *win) {
	OBJECT *tree;
	short x, y;

	ObjcChange( OC_FORM, win, RAC_POPUP, SELECTED, TRUE);
	rsrc_gaddr( 0, POPUP, &tree);
	objc_offset( FORM(win), BUT_POPUP, &x, &y);
	ope = _popup( tree, x, y, -1, ldg_global);
	switch( ope) {
	case OPE_INFO:
		strcpy( ObjcString( FORM(win), BUT_POPUP, NULL), "Info");
		break;
	case OPE_COS:
		strcpy( ObjcString( FORM(win), BUT_POPUP, NULL), "Cosinus");
		break;
	case OPE_SIN:
		strcpy( ObjcString( FORM(win), BUT_POPUP, NULL), "Sinus");
		break;
	}
	if( ope > 0)
		ObjcDraw( OC_FORM, win, BUT_POPUP, 0);
	ObjcChange( OC_FORM, win, RAC_POPUP, ~SELECTED, TRUE);

}

/* Close button */

void close_dial( WINDOW *win, int index) {
  ApplWrite( app.id, WM_CLOSED, win->handle, 0, 0, 0, 0);
  ObjcChange( OC_FORM, win, index, ~SELECTED, TRUE);
}

/* Handle AP_TERM message */

void ApTerm( void) {
	/* close all windows */
	while( wglb.first) {
		ApplWrite( app.id, WM_DESTROY,  wglb.first->handle, 0, 0, 0, 0);
		EvntWindom( MU_MESAG);
	}
	/* Discharge librairies */
	ldg_close( ldg_popup, ldg_global);
	ldg_close( ldg_math, ldg_global);
	RsrcXtype( FALSE, NULL, 0);
	RsrcFree();
	ApplExit();
	exit( 0);
}

/*
 *	Load LDG and report trace in a dialog box
 */

void LoadLDGinit( void) {
	strcpy( ObjcString( get_tree( LOAD_LDG), LOAD_NAME, NULL), "");
	strcpy( ObjcString( get_tree( LOAD_LDG), LOAD_INFO, NULL), "");
	FormBegin( get_tree( LOAD_LDG), NULL);
}

LDG* LoadLDG( char *lib, void (*init)(LDG *)) {
	LDG *ldg;
	char *p = strrchr( lib, '\\');
	strcpy( ObjcString( get_tree( LOAD_LDG), LOAD_NAME, NULL), p?(p+1):lib);
	objc_draw( get_tree( LOAD_LDG), LOAD_NAME, 0, app.x, app.y, app.w, app.h);
	ldg = ldg_open( lib, ldg_global);
	if( ldg) {
		strncpy( ObjcString( get_tree( LOAD_LDG), LOAD_INFO, NULL), ldg->infos, 21);
		if( init) (*init)(ldg);
		objc_draw( get_tree( LOAD_LDG), LOAD_INFO, 0, app.x, app.y, app.w, app.h);
	} else
		FormAlert( 1, "[1][Error %d with|%s][OK]", ldg_error(), p?(p+1):lib);
	return ldg;
}

void LoadLDGexit( void) {
	FormEnd( get_tree( LOAD_LDG), NULL);
}

/*
 * LDG_QUIT : this message indicates a library has just
 * shutdown. The client should reload it !
 */

void LdgQuit( void) {
	LDG *ldg = *(LDG **)&evnt.buff[3];
	if( FormAlert( 1, "[1][A library has shutdowned|Reload it?][Yes|No]") == 1) {
		LoadLDGinit();
		if( ldg == ldg_math)
			ldg_math  = LoadLDG( "..\\libs\\ex_lib.nld", init_ldg_math);
		else if( ldg == ldg_popup)
			ldg_popup = LoadLDG( "..\\libs\\popup.nld", init_ldg_popup);
		LoadLDGexit();
	}
}

/* Main Part */

void main( void) {
	OBJECT *dial;
	WINDOW *win;

	/* Init GEM */
	ApplInit();
	RsrcLoad( "client.rsc");
	RsrcXtype( TRUE, NULL, 0);

	/* Loading librairies */
	LoadLDGinit();
	ldg_math  = LoadLDG( "..\\libs\\ex_lib.nld", init_ldg_math);
	ldg_popup = LoadLDG( "..\\libs\\popup.nld", init_ldg_popup);
	LoadLDGexit();

	/* Create dialog */
	rsrc_gaddr( 0, INFO_LDG, &dial);
	win = FormCreate( dial, WAT_FORM, NULL, "LDG Client", NULL, 1, 0);
	ObjcAttach( OC_FORM, win, BUT_POPUP, BIND_FUNC, call_popup);
	ObjcAttach( OC_FORM, win, RAC_POPUP, BIND_FUNC, call_popup);
	ObjcAttach( OC_FORM, win, OK, BIND_FUNC, close_dial);
	ObjcAttach( OC_FORM, win, CALCUL, BIND_FUNC, compute);
	
	/* Trap GEM mesag */
	EvntAttach( NULL, AP_TERM, ApTerm);
	EvntAttach( NULL, LDG_QUIT, LdgQuit);
	
	/* Handle GEM events */	
	for( ;;) EvntWindom( MU_MESAG);
}

