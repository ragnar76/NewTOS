/*
 * envrn.c -- ARCH03 stage 3b: shel_envrn hands an EL0 program a pointer it
 * can read. EmuTOS sh_envrn (aes/gemshlib.c:327-347) points *ppath into the
 * AES environment ad_envrn, which every program of the flat machine reads.
 * Before 3b NewTOS wrote a kernel address into the program's slot (3a
 * observation): reading it ended the program with a bus error.
 *
 * TASK/PPARENT copy: ad_envrn is now EmuTOS's -- the AES PD's p_env, a copy
 * of default_env "PATH=\0X:\\\0\0" (bios/bios.c:626-636) -- so the AES
 * answers the NUL of the TOS quirk after "PATH=", which gemlib's
 * mt_shel_envrn steps over: the program sees "X:\" (X = bootdev, $446);
 * the 3b expectation "C:\;" was NewTOS's old built-in default.
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
		/* the AES answers the NUL after "PATH=", gemlib's mt_shel_envrn
		 * steps over it (a_shel_envrn.c:40-50): "X:\\" */
		unsigned short bootdev = *(volatile unsigned short *)0x446UL;
		check("value readable: X:\\ (EmuTOS default_env, gemlib PATH= fix)",
		      p[0] == 'A' + bootdev && p[1] == ':' && p[2] == 92 && p[3] == 0);
	}
	p = (char *)1;
	shel_envrn(&p, "NOSUCHVAR=");
	checkhex("unknown variable -> NULL", (unsigned long)p, 0);
	appl_exit();
	Cconws(fails ? "ENVRN: FAIL\r\n" : "ENVRN: PASS\r\n");
	return fails;
}
