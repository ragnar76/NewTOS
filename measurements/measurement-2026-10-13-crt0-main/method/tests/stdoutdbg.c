#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <errno.h>
#include "common.h"
extern int __mint;
static void st(const char *w)
{
	Cconws(w);
	Cconws(" buf="); puthex((unsigned long)stdout->__buffer);
	Cconws(" bufsize="); puthex((unsigned long)stdout->__bufsize);
	Cconws(" linebuf="); puthex((unsigned long)stdout->__linebuf);
	Cconws(" userbuf="); puthex((unsigned long)stdout->__userbuf);
	Cconws(" err="); puthex((unsigned long)stdout->__error);
	Cconws(" eof="); puthex((unsigned long)stdout->__eof);
	Cconws(" seen="); puthex((unsigned long)stdout->__seen);
	Cconws(" wr="); puthex((unsigned long)stdout->__mode.__write);
	Cconws(" putlim="); puthex((unsigned long)stdout->__put_limit);
	Cconws(" iow="); puthex((unsigned long)stdout->__io_funcs.__write);
	Cconws("\r\n");
}
int main(void)
{
	int r;
	Cconws("[DBG] __mint="); puthex((unsigned long)__mint); Cconws("\r\n");
#ifdef BINMODE
	{ extern void __set_binmode(FILE *, int); __set_binmode(stdout, 1); Cconws("[DBG] stdout -> binary mode\r\n"); }
#endif
	{ extern ssize_t __stdio_text_write(void *, const char *, size_t); long w;
	  errno = 0; w = __stdio_text_write((void *)1L, "[DBG] text_write\n", 17);
	  Cconws("[DBG] __stdio_text_write rc="); puthex((unsigned long)w); Cconws(" errno="); puthex((unsigned long)errno); Cconws("\r\n"); }
#ifdef MANUAL
	Cconws("[DBG] setvbuf rc="); puthex((unsigned long)setvbuf(stdout, NULL, _IOLBF, 0)); Cconws("\r\n");
#endif
	st("[DBG] before");
	errno = 0;
	r = fputs("[DBG] fputs\n", stdout);
	Cconws("[DBG] fputs rc="); puthex((unsigned long)r); Cconws(" errno="); puthex((unsigned long)errno); Cconws("\r\n");
	st("[DBG] after fputs");
	errno = 0;
	r = printf("[DBG] printf with newline\n");
	Cconws("[DBG] printf rc="); puthex((unsigned long)r); Cconws(" errno="); puthex((unsigned long)errno); Cconws("\r\n");
	st("[DBG] after");
	Cconws("DBG: DONE\r\n");
	return 0;
}
