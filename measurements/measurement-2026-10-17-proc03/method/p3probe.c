/*
 * p3probe.c -- TASK/PROC03 read-only probe: what a program can observe of its
 * parent chain, environment and current directory, per launch path.
 * One source, roles by -D:
 *   P3AUTO.NCP  -DP3_AUTO   C:\AUTO\ (console, before the AES); Pexec child
 *   P3Z.NAP     -DP3_GEM    #Z autorun; Pexec child; shel_write(C:\P3Z2.NAP)
 *   P3Z2.NAP    (copy of P3Z.NAP, started by shel_write: tail "Z2")
 *   P3ACC.NAC   -DP3_ACC    C:\ACC\ accessory; Pexec child from the accessory
 * Kid (Pexec child, tail = caller's PD as 8 hex digits, length byte 8):
 *   prints own PD/p_parent/env/cwd, compares p_parent with the tail, changes
 *   the directory to \ and exits 0x42.
 * Prints "[P3 <tag>] ..." lines only; judges nothing (EmuTOS values are in
 * the RUNINFO).
 */
#include "common.h"
#include <gem.h>
#include <string.h>

extern short _app;

#ifndef P3_TAG
#define P3_TAG "?"
#endif

static void hexb(const char *what, const unsigned char *e, int n)
{
	static const char d[] = "0123456789abcdef";
	char b[4];
	int i;
	Cconws("[P3 " P3_TAG "] "); Cconws(what); Cconws(" =");
	for (i = 0; i < n; i++) {
		b[0] = ' '; b[1] = d[e[i] >> 4]; b[2] = d[e[i] & 15]; b[3] = 0;
		Cconws(b);
	}
	Cconws("\r\n");
}

static void show_pd(const char *what, const BASEPAGE *p)
{
	Cconws("[P3 " P3_TAG "] "); Cconws(what);
	Cconws(" PD "); puthex((unsigned long)p);
	Cconws(" lowtpa "); puthex(p->p_lowtpa);
	Cconws(" hitpa "); puthex(p->p_hitpa);
	Cconws(" tbase "); puthex(p->p_tbase);
	Cconws(" tlen "); puthex(p->p_tlen);
	Cconws(" parent "); puthex(p->p_parent);
	Cconws(" w28 "); puthex(p->p_reserved);
	Cconws(" env "); puthex(p->p_env);
	Cconws("\r\n");
	hexb("  p_uft[6]+lddrv+curdrv", (const unsigned char *)p->p_junk, 8);
	if (p->p_env)
		hexb("  env[0..12]", (const unsigned char *)(unsigned long)p->p_env, 12);
}

static void cwd(void)
{
	char path[128];
	short d = Dgetdrv();
	path[0] = 0;
	Dgetpath(path, 0);
	Cconws("[P3 " P3_TAG "] Dgetdrv "); puthex((unsigned long)d);
	Cconws(" Dgetpath '"); Cconws(path); Cconws("'\r\n");
}

/* walk p_parent: dereference only while the address lies below 4 GiB and the
 * walk is bounded; the caller runs this in a mode where the stand-in page is
 * readable. Parent PDs of other processes are NOT dereferenced (strict). */
static void chain(int deref_depth)
{
	const BASEPAGE *p = _base;
	int i;
	show_pd("self", p);
	cwd();
	for (i = 0; i < deref_depth && p->p_parent; i++) {
		p = (const BASEPAGE *)(unsigned long)p->p_parent;
		show_pd(i == 0 ? "parent" : "grandparent", p);
	}
	Cconws("[P3 " P3_TAG "] os_run PD "); puthex(os_run_pd()); Cconws("\r\n");
}

static void run_kid(const char *self)
{
	char tail[12];
	long rc;
	tail_hex(tail, (unsigned long)_base);
	Cconws("[P3 " P3_TAG "] Pexec kid, parent PD passed in tail = "); puthex((unsigned long)_base); Cconws("\r\n");
	rc = Pexec(0, self, tail, 0L);
	Cconws("[P3 " P3_TAG "] Pexec returned "); puthex((unsigned long)rc); Cconws("\r\n");
	cwd();
}

static int kid(void)
{
	const BASEPAGE *me = _base;
	unsigned long want = tail_value((const char *)me->p_cmdlin);
	show_pd("kid self", me);
	cwd();
	Cconws("[P3 " P3_TAG "-KID] p_parent==tail "); Cconws(me->p_parent == want ? "YES" : "NO");
	Cconws(" (want "); puthex(want); Cconws(")\r\n");
	Dsetpath("\\");
	cwd();
	return 0x42;
}

#if defined(P3_AUTO)
int main(void)
{
	if (_base->p_cmdlin[0] == 8) return kid();
	Cconws("[P3 " P3_TAG "] _app "); puthex((unsigned long)_app); Cconws("\r\n");
	chain(3);
	Dsetpath("\\SUB");
	cwd();
	run_kid("C:\\AUTO\\P3AUTO.NCP");
	Cconws("P3 AUTO DONE\r\n");
	return 0;
}
#elif defined(P3_ACC)
int main(void)
{
	short id, msg[8];
	Cconws("[P3 " P3_TAG "] _app "); puthex((unsigned long)_app); Cconws("\r\n");
	chain(3);
	id = appl_init();
	menu_register(id, "  P3 probe");
	Cconws("[P3 ACC] ap_id "); puthex((unsigned long)id); Cconws("\r\n");
	run_kid("C:\\P3Z.NAP");
	Cconws("P3 ACC DONE\r\n");
	for (;;) evnt_mesag(msg);
	return 0;
}
#else
int main(void)
{
	char *val = 0;
	if (_base->p_cmdlin[0] == 8) return kid();
	Cconws("[P3 " P3_TAG "] _app "); puthex((unsigned long)_app); Cconws("\r\n");
	chain(3);
	appl_init();
	shel_envrn(&val, "PATH=");
	Cconws("[P3 " P3_TAG "] shel_envrn(PATH=) -> "); puthex((unsigned long)val); Cconws("\r\n");
	if (_base->p_cmdlin[0] == 2) {          /* Z2: started by shel_write */
		Cconws("P3 Z2 DONE\r\n");
		appl_exit();
		return 0;
	}
	Dsetpath("\\SUB");
	cwd();
	run_kid("C:\\P3Z.NAP");
	{
		char t[3] = { 2, 'Z', '2' };
		Cconws("[P3 Z] shel_write(C:\\P3Z2.NAP)\r\n");
		shel_write(1, 1, 1, "C:\\P3Z2.NAP", t);
	}
	appl_exit();
	Cconws("P3 Z DONE\r\n");
	return 0;
}
#endif
