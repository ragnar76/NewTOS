/*
 * LDG : Gem Dynamical Libraries
 * Copyright (c) 1997-2010 Olivier Landemarre, Dominique Bereziat & Arnaud Bercegeay
 *
 * Low-level functions to load and unload a LDG-library
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
 * $Id$
 */

 /*   13 fev 2010 : Patch from Mikro  : Fix memory protection issue + some clean up       */

/*
 * NewTOS overlay (LP64 ABI correction; built by sdk/ldg/build-ldg.sh, the
 * pristine file is src/devel/ldgload.c of sdk/ldg/ldg-svn-r133.tar.xz).
 *
 * The ONLY change vs. the pristine ldgload.c is the read of the dummy
 * basepage's environment pointer at ldgload.c:76 (`ldg_global_env =
 * module->p_env`). With the SDK's LP64 basepage.h overlay
 * (sdk/libc/overlay/include/mint/basepage.h:30-47) p_env is a fixed 32-bit
 * field, so assigning it to a `char *` is an int->pointer conversion
 * (-Wint-conversion, a default ERROR under this clang). It is cast through
 * `(char *)(uintptr_t)` -- NewTOS EL0 VAs are < 2^32, so the 32-bit value is
 * the full environment VA (same fix as sdk/overlays/teradesk/environm.c).
 *
 * The other p_env uses (ldgload.c:133,162,169,204) need no change: they only
 * pass the value to Mfree(), whose binding converts its argument to `long`
 * (sdk/libc/overlay/include/mint/osbind.h, `Mfree`), so the zero-extended
 * 32-bit address reaches the trap unchanged. LDG logic is otherwise untouched.
 */
#include <stdint.h>

#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#if defined(__PUREC__) || defined(__VBCC__)
#include <tos.h>
/*#ifndef BASEPAGE
#define	BASEPAGE BASPAG
#endif*/
#endif
#if defined(__GNUC__) || defined(__SOZOBONX__)
# if (__GNUC__ > 2) || ( __GNUC_MINOR__ > 8 )
#  include <mint/basepage.h>
# else
#  include <basepage.h>
# endif
# include <osbind.h>
# include <mintbind.h>
#endif

#include "../options.h"
#include "../../include/ldg.h"
#include "global.h"

/*
 * Charger un module et retourne l'adresse
 * de la structure LDG du module.
 * apid: id du lanceur.
 */





LDG *ldg_load( int apid, const char *path) {
	LDG *ldg = NULL;
	BASEPAGE *module=NULL;
	char *offsetxt, smallbloc[24], *mbloc=NULL;
  char *ldg_global_env=NULL;
	long size_env;

	/* 
	 * L'adresse du descripteur de la lib (LDG) 
	 * est fournit par la librairie (peut etre un
	 * programme en memoire protegee) et est transmise
	 * entre lib et client via une variable d'environnement !
	 */
   
	module = (BASEPAGE *)Pexec( 5, NULL, "", NULL);  /* pour recuperer l'environnment du programme appelant on cree un basepage bidon */
	if(module)
	{
   	ldg_global_env = (char *)(uintptr_t)module->p_env;  /* recuperation de l'environnement */
    
    if(ldg_global_env)
		{	char *env;
			size_env=0L;
			env = ldg_global_env; 
			while(*env)
			{
				while (*env)
				{
					size_env++;
					env++;
				}
				env++;
				size_env++;
			}
			if(size_env)
			{
				mbloc=(char *)Malloc(size_env+24);
			}
		}
	}
  
	if(mbloc)
	{
		offsetxt = mbloc;
	}
	else
	{
		offsetxt = smallbloc;
	}
#ifdef __GNUC__
	sprintf(offsetxt,"OFFSETLDG=%ld%c%c",(long)&ldg,'\0','\0');
#else
	strcpy(offsetxt,"OFFSETLDG=");
  	ltoa( (long)&ldg, offsetxt+10, 10);
  	offsetxt[strlen( offsetxt) + 1] = '\0';
#endif
	if(mbloc)
	{	/* recopie de l'environnement */
		char *pt, *env;
		pt = offsetxt;
		env = ldg_global_env;
		while (*pt) pt++;
		pt++;
		while(*env)
		{
			while (*env)
			{
				*pt++ = *env++;
			}
			*pt++ = *env++;
		}
		*pt = 0;
	}
	if(module)
	{
		Mfree( module->p_env);
    Mfree( module);
		module=NULL;
	}
	module = (BASEPAGE *)Pexec( 3, (char *)path, NULL, offsetxt); /* Load, don't go */
	if(mbloc) Mfree(mbloc);
	if( (long)module < 0L) {
		set_error( LDG_ERR_EXEC);
		return NULL;	/* echec de Pexec */
	}
	
	/* Vider les caches */
#ifndef __mcoldfire__
	/* Why is this necessary? Pexec(3) should flush the caches internally. */
	/* Anyway, this is never necessary on modern ColdFire operating systems. */
	ldg_cpush();  
#endif

	(void) Pexec( 4, NULL, module, NULL); /* Just go */

	/* Vider les caches */
#ifndef __mcoldfire__
	/* Why is this necessary? The caches were flushed before Pexec(4). */
	ldg_cpush();  
#endif
	Pwait();

	if(ldg==NULL)  {
    	set_error( LDG_BAD_FORMAT);
    	Mfree( module->p_env);
    	Mfree( module);
    	return NULL;	/* Pas une librairie */
	}

	if( ldg -> magic != LDG_COOKIE) {
 		set_error( LDG_BAD_FORMAT);
    	Mfree( module->p_env);
    	Mfree( module);
    	return NULL;	/* mauvais format de librairie */
  	}
  	
	ldg -> id = (short)(apid&0xFFFF);    /* contient maintenant l'id du lanceur */
	ldg -> baspag = module;
	
	/* depuis version 1.00 */
 	if( ldg -> vers_ldg >= 0x100) {
 		if( !strrchr( path, '\\')) {
 			char currpath[PATHLEN];
			
			currpath[0]=Dgetdrv()+'A';
			currpath[1]=':';
			currpath[2]='\\';
			currpath[3]='\0';
			Dgetpath( currpath+2, 0);
			strncpy( ldg->path, currpath, PATHLEN-1);
			strcat( ldg->path,"\\");
			strcat( ldg->path, path); 
		}
		else
			strncpy( ldg->path,path,PATHLEN-1);
	}
	return ldg;
}

/*
 * Enlever un module de la m‚moire
 */

int ldg_unload( LDG *ldg) {
	if( ldg) {
		if( ldg->close) (*ldg->close)();
		Mfree( ((BASEPAGE *)ldg->baspag)->p_env);
		Mfree( ldg->baspag);
		/* At this point we can't use 'ldg' structure anymore.
		 * Formally we've released only ldg->baspag but 'ldg' points
		 * to static LDG struct in the module, i.e. it's part of
		 * process TPA which was just released.
		 */
	}
	return 0;
}

/* EOF */
