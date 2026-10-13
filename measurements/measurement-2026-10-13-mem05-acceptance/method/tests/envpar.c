/*
 * envpar.c -- MEM03 acceptance, PARENT: an EL0 program starts a child with
 * an explicit environment (Pexec 0, TeraDesk's ARGV launch shape), the child
 * reads it (envkid.c) and answers 0x42 when every record arrived.
 * Recipe after measurements/measurement-2026-10-02-port09/method/envpar.c.
 */
#include "common.h"

static const char env[] =
	"PATH=C:\\\0"
	"ARGV=\0"
	"C:\\ENVKID.NAP\0"
	"alpha-one\0mike thirteen has blanks\0"
	"\0";

int main(void)
{
	long rc;
	static char tail[128] = "\177alpha-one mike thirteen has blanks";

	Cconws("[MEM03] parent: Pexec(0, C:\\ENVKID.NAP, tail, env @ ");
	puthex((unsigned long)env);
	Cconws(")\r\n");
	rc = Pexec(0, "C:\\ENVKID.NAP", tail, env);
	checkhex("child read its environment (rc)", (unsigned long)rc, 0x42);
	Cconws(fails ? "MEM03: FAIL\r\n" : "MEM03: PASS\r\n");
	return fails;
}
