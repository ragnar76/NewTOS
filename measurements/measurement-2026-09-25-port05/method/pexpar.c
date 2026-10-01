/*
 * pexpar.c -- PORT05 reproducer, PARENT half.
 *
 * TeraDesk launches QED with `x_exec(0, ...)` = `Pexec(PE_LOADGO, ...)` from
 * EL0 and STAYS ALIVE across the call (sdk/apps/teradesk/startprg.c:312,
 * exec_com()).  Every NewTOS acceptance so far launched a program the other
 * way: from the EL1 kernel (kernel/src/launch.rs, run_program), where the
 * caller owns no process slot.  This program is the smallest faithful stand-in
 * for TeraDesk's route: an EL0 GEM application that
 *
 *   1. appl_init()s,
 *   2. BLOCKS ONCE in an AES wait (evnt_timer) -- the state TeraDesk is always
 *      in when it launches: it came out of evnt_multi,
 *   3. Pexec(0, "C:\PEXKID.NAP", tail, NULL)s a second EL0 GEM application,
 *   4. and prints what it got back.
 *
 * Everything is printed with Cconws, i.e. it lands on the serial console of a
 * headless QEMU raspi4b run as well as on the screen.  The marker text is
 * unique per line so the serial log alone says exactly how far the run got.
 *
 * "[PAR] 1 main entry" is printed at the very top of main(): if the parent is
 * ever RESTARTED (rather than resumed), that banner appears a second time.
 */

/* The program this parent launches. The default is the tiny PEXKID.NAP of
 * this measurement; -DCHILD_PATH=... builds the variant that launches the REAL
 * ported application (C:\\APPS\\QED\\QED.NAP), i.e. exactly what TeraDesk
 * does, without needing a scripted click. */
#ifndef CHILD_PATH
#define CHILD_PATH "C:\\PEXKID.NAP"
#endif

/* TeraDesk changes the current directory to the program's own directory
 * before the Pexec and changes it back afterwards (startprg.c:620 / :765,
 * the single-task arm).  That is what lets a launched GEM application find
 * its `.RSC` by the bare name: `sh_find` stage 2 is the spec unchanged, i.e.
 * the current directory (aes/gemshlib.c:406-472).  A Pexec issued by an
 * application does NOT go through the AES, so `p_appdir` keeps naming the
 * LAUNCHER -- on EmuTOS as much as here.  -DCHILD_DIR=... makes this
 * reproducer do the same thing, so the run measures the launch path and not
 * a missing chdir.  */

#include <mint/osbind.h>   /* Cconws, Pexec, Pterm */
#include <gem.h>           /* appl_init, evnt_timer, appl_exit */

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

int main(void)
{
	long rc;
	char tail[2];

	Cconws("[PAR] 1 main entry\r\n");

	(void)appl_init();
	Cconws("[PAR] 2 appl_init done\r\n");

	/* The state TeraDesk is in when it launches: it has been through a
	 * blocking AES wait.  One 500 ms timer is enough to take the slot
	 * through the sleep/wake substrate exactly once. */
	(void)evnt_timer(500UL);
	Cconws("[PAR] 3 first evnt_timer returned\r\n");

	tail[0] = 0;               /* Atari command tail: length byte = 0 */
	tail[1] = '\0';

#ifdef CHILD_DIR
	Cconws("[PAR] 3b Dsetpath(" CHILD_DIR ")\r\n");
	(void)Dsetpath(CHILD_DIR);
#endif

	Cconws("[PAR] 4 calling Pexec(0, " CHILD_PATH ")\r\n");
	rc = Pexec(0, CHILD_PATH, tail, (void *)0L);
	Cconws("[PAR] 5 Pexec returned rc=");
	puthex((unsigned long)rc);
	Cconws("\r\n");

	/* Alive after the child?  Block once more, the way TeraDesk returns to
	 * its event loop after exec_com(). */
	(void)evnt_timer(500UL);
	Cconws("[PAR] 6 second evnt_timer returned\r\n");

	appl_exit();
	Cconws("[PAR] 7 appl_exit done, Pterm(0)\r\n");
	Pterm(0);
	return 0;
}
