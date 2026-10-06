/*
 * LDG : Gem Dynamical Libraries
 * Copyright (c) 1997-2004 Olivier Landemarre, Dominique Bereziat & Arnaud Bercegeay
 *
 * Some usefull functions (for LDG kernel & client application)
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
 * Patch Coldfire Vincent Riviere
 */

/*
 * NewTOS overlay (AArch64/LP64 retarget; built by sdk/ldg/build-ldg.sh, the
 * pristine file is src/devel/ldgutil.c of sdk/ldg/ldg-svn-r133.tar.xz).
 * Two functions differ from the pristine ldgutil.c; everything else is
 * byte-identical.
 *
 * 1. ldg_cookie (ldgutil.c:53-70). The original reads the _p_cookies sysvar
 *    as `*(long *)0x5a0` under Supexec and walks the jar as pairs of `long`.
 *    On LP64 a `long` is 8 bytes while the jar pointer and every jar entry
 *    are 32-bit words (tos-api/src/bdos/process/loader.rs, cookie jar
 *    setup), so the raw access yields a wrong jar pointer and a wrong
 *    stride. NewTOS exposes the jar through Ssystem(S_GETCOOKIE); mintlib's
 *    Getcookie() (sdk/libc/newtos-arch/getcookie.c) is that call. The
 *    function keeps its LDG contract: 1 and *value = cookie value on a hit,
 *    0 on a miss (value untouched on a miss, as in the original loop).
 *
 * 2. ldg_cpush (ldgutil.c:205-225). The original executes a data array of
 *    m68k opcodes (NOP; CPUSHA BC; RTS) through Supexec when the _CPU cookie
 *    is > 30 -- m68k assembly, the one case the toolchain may retarget. On
 *    NewTOS the kernel already makes a loaded program's code visible to
 *    instruction fetch inside Pexec(3): loader.rs calls
 *    mmu::sync_code_cache (tos-api/src/bdos/process/loader.rs:599; DC CVAU,
 *    DSB ISH, IC IVAU, DSB ISH, ISB in tos-api/src/bdos/process/mmu.rs:1564),
 *    exactly as EmuTOS (bdos/proc.c:352-356) and FreeMiNT (k_exec.c:686-687)
 *    do -- ldgload.c:146 itself notes "Pexec(3) should flush the caches
 *    internally". EL0 could not do the I-cache part anyway: SCTLR_EL1.UCI
 *    is left clear (kernel/src/arch/aarch64/boot.rs:189-198), so IC IVAU at
 *    EL0 traps (docs/analysis/arch03-stage0a-armarm-verification.md, E1).
 *    ldg_cpush therefore has nothing left to do and is an empty function;
 *    the symbol stays because ldgload.c and clients call it (ldg.h:165-167).
 */
#include <mint/cookie.h>

#include <string.h>
#include <stdlib.h>
#include <stdarg.h>
#include <stdio.h>
#include "../version.h"
#include "../options.h"
#include "../../include/ldg.h"
#include "global.h"

/*#include <gem.h>*/
#if defined(__PUREC__) || defined(__VBCC__)
#include <tos.h>
#endif
#if defined(__GNUC__) || defined(__SOZOBONX__)
#include <osbind.h>
#define BASPAG	BASEPAGE
#endif

struct _ldg_version ldg_version = { LDG_NAME, LDG_NUM};

#define MagX_COOKIE     0x4D616758L	/* 'MagX' */
#define MiNT_COOKIE     0x4D694E54L	/* 'MiNT' */


/*
 * @TODO : get a better version of get_cookie (from WinDom or Ssytem()'s Mint)
 */

int	ldg_cookie (long cookie, long *value) {
  long v;

  /* NewTOS: the jar is read through Ssystem(S_GETCOOKIE) via Getcookie(),
   * see the overlay note at the top of this file (item 1). */
  if (Getcookie(cookie, &v) == C_FOUND) {
    if (value) *value = v;
    return( 1);
  }
  return( 0);
}

/*
 * Gestion des erreurs : le code erreur est
 * ‚crit dans le champ du cookie LDGM d‚di‚ …
 * cet usage.
 */

void set_error( int code) {
	LDG_INFOS *cook;
	
	if( ldg_cookie( LDG_COOKIE, (long*)&cook) && cook->version >= 0x0210)
		cook->error = (short)(code&0xFFFF);
}

#if 0
/*
 *	Fonction de d‚bugage 
 *  nouvelle version : si la chaine <format> est
 *	au format d'une alerte GEM, on utilisera form_alert
 */

void ldg_debug( char *format, ...) {
	char *path, debug[255];
	va_list arglist;
	FILE *log;
	
	/* Provisoire, on pourrait aussi utiliser le cookie
	 * car ‡a serait plus souple */
	shel_envrn( &path, "LDG_DEBUG=");

	/* format */
	if( format) {
		va_start( arglist, format);
		vsprintf( debug, format, arglist);
		va_end( arglist);
		if( *format == '[')
			form_alert( 1, debug);
		else {
			log = fopen( path, "a");
			if( log) fputs( debug, log);
		} 
	} else {
		log = fopen( path, "w");
		if(log)	fclose( log);
	}
}

#endif

void *ldg_Malloc( long size) { 
	void *bloc = 0L; 
	static short flag = 0;	/* vaut 1,-1 ou 0 */
	if( flag == 0) {

	union {
#ifdef __SOZOBONX__
		struct {
			unsigned minor:8;
			unsigned major:8;
		} braker;
#else
		struct {
			unsigned int minor:8;
			unsigned int major:8;
		} braker;
#endif
		short all;
	} versn;
		
		versn.all = (short)Sversion();
		/* Mxalloc() … partir de GEMDOS 0 r‚vision 19 */
		/* flag = ((versn.braker.major>0) || (versn.braker.minor>=0x19))?1:-1; */
		/* Franchement il y a aucune difference */

		if((versn.braker.major>0) || (versn.braker.minor>=0x19))		
			flag = -1;
		else 
			flag=1;
 	}
	if( flag == -1) {
		if( ldg_cookie( MagX_COOKIE, NULL) || ldg_cookie( MiNT_COOKIE, NULL))
		/* allocation globale pr‚f‚rentiellement en TT Ram */
    		bloc = (void *)Mxalloc( size, 3+8+32);
		else
			bloc = (void *)Mxalloc( size, 3);
	} else
		bloc = (void *)Malloc( size);
	return( bloc);
}

void *ldg_Calloc(long count,long size) { 
	void *bloc = 0L; 
	char *pt = 0L; 
	long i, taille;
  
	taille = count*size;
	bloc = ldg_Malloc( taille);
	if(bloc != 0L) {
		pt = (char *)bloc;
		for( i= 0L; i < taille; i++)
			*pt++ = 0;
	}
	return( bloc);
}

void *ldg_Realloc( void *oldblk, long oldsize, long newsize) {
	void *newblk;
	
	newblk = ldg_Malloc( newsize);
	if( newblk) {
		memcpy( newblk, oldblk, oldsize);
		ldg_Free( oldblk);
		return newblk;
	}
	return NULL;
}

int ldg_Free(void *memory) {
	return( Mfree(memory));
}

#ifndef __mcoldfire__

/*
 *  ldg_cpush
 *
 * vidage des caches data et instruction du processeur
 * par l'appel … la fonction asm "cpush".
 * ce code peut etre appel‚ … partir du 68040 et doit ˆtre
 * ex‚cut‚ en mode superviseur (d'o— l'utilisation de Supexec).
 *
 * 1er mouture le 17 avril 2002 par Arnaud BERCEGEAY <bercegeay@atari.org>
 */

void ldg_cpush(void) {
	/* NewTOS: intentionally empty -- the kernel synchronised the caches
	 * for the loaded image inside Pexec(3), see the overlay note at the
	 * top of this file (item 2). */
}

#endif

/* EOF */
