/*  system.c -- MiNTLib.
    Copyright (C) 1999 Guido Flohr <guido@freemint.de>

    This file is part of the MiNTLib project, and may only be used
    modified and distributed under the terms of the MiNTLib project
    license, COPYMINT.  By continuing to use, modify, or distribute
    this file you indicate that you have read the license and
    understand and accept it fully.
*/

/*  Emulation (that is called in absence of a shell) was written by 
    Eric R. Smith.  The POSIX-compliant branch is inspired by the
    GNU libc.  
    
    Somebody should check if the POSIX branch works under MagiC.  */

/* NewTOS/AArch64 (TASK/PORT10, user decision 2026-10-02): this overlay drops
 * mintlib's POSIX branch of system() -- the /bin/sh / $SHELL probe and the
 * sigaction + vfork + execve + waitpid sequence it selects -- and keeps
 * everything else of the file verbatim. system() is therefore always the
 * TOS branch ("emulation" below): the line is split into words, < and >/>>
 * redirect handles 0/1 via Fopen/Fcreate + Fdup/Fforce, and the program runs
 * through spawnvp(P_WAIT, ...), i.e. a synchronous Pexec(0) -- what mintlib
 * itself does on TOS without a shell.
 *
 * Why: the POSIX branch does not link on NewTOS. mintlib's vfork is m68k
 * assembler (unix/vfork.S, never assembled here) and on TOS it is tfork =
 * Pexec(5) + Pexec(4/6) on code in the parent's own space; NewTOS answers
 * Pexec modes 5/6/7 with EINVFN (TASK/PROC01). waitpid/wait4 also pull
 * __waitval/__waittime from mintlib/thread.c, which build-libc.sh excludes.
 *
 * Observable deviation from mintlib: ONLY when access("/bin/sh", X_OK)
 * succeeds or $SHELL is set. Pristine mintlib would then try the shell via
 * vfork/execve; this overlay runs the line through the TOS branch anyway.
 * system(NULL) still returns 1, as in mintlib.
 *
 * TASK/PROC01 (Pexec 5/6/7, vfork ported to AArch64, thread.c LP64-clean)
 * removes this overlay again, so that system() is mintlib's unchanged. */
#include <process.h>
#include <limits.h>
#include <string.h>
#include <ctype.h>
#include <stddef.h>
#include <stdlib.h>
#include <unistd.h>
#include <signal.h>
#include <errno.h>

#include <sys/wait.h>
#include <sys/types.h>
#include <sys/file.h>
#include <mint/osbind.h>

#include "lib.h"

static int system_hack(const char* s);

/* Execute LINE as a shell command, returning its status.  */
int system (const char *line)
{
  if (line == NULL)
    return 1;

  return (system_hack (line));
}

/* Emulation code following.  */
#define isquote(c) ((c) == '\"' || (c) == '\'' || (c) == '`')
#define ARG_ERR	   ( (Argentry *) -1L )

/* struct. used to build a list of arguments for the command */

typedef struct argentry {
	struct argentry *next;
	char	string[1];
} Argentry;

static Argentry *_argalloc (const char *s);
static void _argfree (Argentry *p);
static Argentry *_parseargs (const char *s);

/* allocate an Argentry that will hold the string "s" */

static Argentry *_argalloc(const char *s)
{
	Argentry *x;

	x = (Argentry *) malloc((size_t)(sizeof(Argentry) + strlen(s) + 1));
	if (!x)
		return ARG_ERR;
	x->next = (Argentry *) 0;
	strcpy(x->string, s);
	return x;
}

/* free a list of Argentries */

static void _argfree(Argentry *p)
{
	Argentry *oldp;

	while (p) {
		oldp = p;
		p = p->next;
		free(oldp);
	}
}

/* parse a string into a list of Argentries. Words are defined to be
 * (1) any sequence of non-blank characters
 * (2) any sequence of characters starting with a ', ", or ` and ending
 *     with the same character. These quotes are stripped off.
 * (3) any spaces after an unquoted > or < are skipped, so
 *     "ls > junk" is parsed as 'ls' '>junk'.
 */

static Argentry *_parseargs(const char *s)
{
	Argentry *cur, *res;
	char buf[1024];
	char *t, quote;

	res = cur = _argalloc("");

	for(;;) {
		t = buf;
again:
		while (isspace(*s)) s++;
		if (!*s) break;
		if (isquote(*s)) {
			quote = *s++;
			while (*s && *s != quote)
				*t++ = *s++;
			if (*s) s++;	/* skip final quote */
		}
		else {
			while (*s && !isspace(*s)) {
				*t++ = *s++;
				if (isquote(*s))
					goto again;
			}
			if (*s && ( *(s-1) == '>' || *(s-1) == '<' ))
				goto again;
		}
		*t = 0;
		cur->next = _argalloc(buf);
		if ((cur = cur->next) == NULL)	  /* couldn't alloc() */
			return ARG_ERR;
	}
	cur->next = (Argentry *) 0;
	cur = res; res = res->next; free(cur);
	return res;
}


/* Here is system() itself.
 * FIXME: we probably should do wildcard expansion.
 * also, should errno get set here??
 */

int system_hack (const char *s)
{
	Argentry *al, *cur;
	char **argv, *p;
	int  argc, i;
	char const *infile, *outfile;
	int  infd = 0, outfd = 1, append = 0;
	int oldin = 0, oldout = 1;	/* hold the Fdup'd in, out */
	char path[PATH_MAX];
	int retval;

	if (!s)		/* check for system() supported ?? */
		return 1;
	al = _parseargs(s);		/* get a list of args */
	if (al == ARG_ERR) {		/* not enough memory */
		__set_errno (ENOMEM);
		return -1;
	}

	infile = outfile = "";

/* convert the list returned by _parseargs to the normal char *argv[] */
	argc = i = 0;
	for (cur = al; cur; cur = cur->next)
		argc++;
	if ((argv = (char **) malloc((size_t)(argc * sizeof(char *))))
		== NULL)
	{
		__set_errno (ENOMEM);
		return -1;
	}
	for (cur = al; cur; cur = cur->next) {
		p = cur->string;
		if (*p == '>') {
			outfile = p+1;
			if (*outfile == '>') {
				outfile++;
				append = 1;
			}
			else
			append = 0;
		}
		else if (*p == '<') {
			infile = p+1;
		}
		else
			argv[i++] = p;
	}
 	argv[i] = (char *)0;
 
/* now actually run the program */

	if (*infile) {
		(void)_unx2dos(infile,path, sizeof (path));
		infd = (int)Fopen(path, 0);
		if (infd < __SMALLEST_VALID_HANDLE) {
			perror(infile);
			return(2);
		}
		oldin = (int)Fdup(0);
		(void)Fforce(0, infd);
	}
	if (*outfile) {
		(void)_unx2dos(outfile,path, sizeof (path));
		if (append) {
			outfd = (int)Fopen(path, 2);
			if (outfd < __SMALLEST_VALID_HANDLE)
				outfd = (int)Fcreate(path, 0);
			else
				(void)Fseek(0L, outfd, 2);
		}
		else
			outfd = (int)Fcreate(path, 0);
		if (outfd < __SMALLEST_VALID_HANDLE) {
			perror(outfile);
			return(2);
		}
		oldout = (int)Fdup(1);
		(void)Fforce(1, outfd);
	}

	retval = spawnvp(P_WAIT, argv[0], argv);

	if (*infile) {
		(void)Fforce(0,oldin);
		(void)Fclose(oldin);
		(void)Fclose(infd);
	}
	if (*outfile) {
		(void)Fforce(1,oldout);
		(void)Fclose(oldout);
		(void)Fclose(outfd);
	}
	free(argv);
	_argfree(al);
	return retval;
}
