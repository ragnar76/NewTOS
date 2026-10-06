/*
 * envrn.c -- ARCH03 stage 3b: shel_envrn hands an EL0 program a pointer it
 * can read. EmuTOS sh_envrn (aes/gemshlib.c:327-347) points *ppath into the
 * AES environment ad_envrn, which every program of the flat machine reads.
 * Before 3b NewTOS wrote a kernel address into the program's slot (3a
 * observation): reading it ended the program with a bus error.
 */
#include "common.h"
#include <gem.h>

int main(void)
{
	char *p = 0;
	short r;

	appl_init();
	r = shel_envrn(&p, "PATH=");
	checkhex("shel_envrn returns 1", (unsigned long)r, 1);
	Cconws("[ENVRN] value pointer ");
	puthex((unsigned long)p);
	Cconws("\r\n");
	check("PATH= found (pointer set)", p != 0);
	check("pointer is a user address (< 2 GiB)", (unsigned long)p < 0x80000000UL);
	if (p && (unsigned long)p < 0x80000000UL) {
		/* The read below is the test: it must not fault. */
		check("value readable, starts with C:", p[0] == 'C' && p[1] == ':' && p[2] == 92 && p[3] == ';');
	}
	p = (char *)1;
	shel_envrn(&p, "NOSUCHVAR=");
	checkhex("unknown variable -> NULL", (unsigned long)p, 0);
	appl_exit();
	Cconws(fails ? "ENVRN: FAIL\r\n" : "ENVRN: PASS\r\n");
	return fails;
}
