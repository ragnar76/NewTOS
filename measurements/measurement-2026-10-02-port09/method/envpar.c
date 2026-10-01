/*
 * envpar.c -- PORT09 probe, PARENT half.
 *
 * Does exactly what TeraDesk's single-task launch does for a program type
 * with PT_ARGV (sdk/apps/teradesk/startprg.c:543-567 builds the environment,
 * :650-655 the command tail, :312 issues the call): an EL0 program calls
 *
 *     Pexec(0, child, tail, env)
 *
 * with tail[0] = 127 (the ARGV signal) and an explicit environment block
 * that ends in "ARGV=\0<program>\0<arg1>\0...\0\0" (environm.c
 * make_argv_env()).  The child (envkid.c, or ARGVTEST.NAP) reports what
 * arrived.  Everything goes over Cconws = the serial console.
 *
 * -DCHILD_PATH=... selects the child; default C:\ENVKID.NAP.
 */
#ifndef CHILD_PATH
#define CHILD_PATH "C:\\ENVKID.NAP"
#endif

#include <mint/osbind.h>

static void puthex(unsigned long v)
{
	static const char d[] = "0123456789abcdef";
	char b[19];
	int i;

	b[0] = '0';
	b[1] = 'x';
	for (i = 0; i < 16; i++)
		b[2 + i] = d[(v >> ((15 - i) * 4)) & 0xf];
	b[18] = '\0';
	Cconws(b);
}

/* 14 arguments, 196 characters when joined with blanks -- more than the 125
   a command tail can carry; one of them contains a blank, which only the
   ARGV route can deliver as ONE argument. */
static const char env[] =
	"PATH=C:\\\0"
	"ARGV=\0"
	CHILD_PATH "\0"
	"alpha-one\0bravo-two\0charlie-three\0delta-four\0echo-five\0"
	"foxtrot-six\0golf-seven\0hotel-eight\0india-nine\0juliet-ten\0"
	"kilo-eleven\0lima-twelve\0mike thirteen has blanks\0"
	"november-fourteen-is-the-last-argument\0"
	"\0";

int main(void)
{
	long rc;
	/* What TeraDesk puts in the tail: the first 125 characters of the
	   blank-joined line, NUL-padded, length byte forced to 127. */
	static char tail[128] =
		"\177alpha-one bravo-two charlie-three delta-four echo-five "
		"foxtrot-six golf-seven hotel-eight india-nine juliet-ten kilo-el";

	Cconws("[EPAR] 1 main entry, env @ ");
	puthex((unsigned long)env);
	Cconws("\r\n");
	Cconws("[EPAR] 2 calling Pexec(0, " CHILD_PATH ", tail[0]=127, env)\r\n");
	rc = Pexec(0, CHILD_PATH, tail, env);
	Cconws("[EPAR] 3 Pexec returned rc=");
	puthex((unsigned long)rc);
	Cconws("\r\n");
	Pterm(0);
	return 0;
}
