/*
 * pparent.c -- TASK/PPARENT probe: p_parent and p_env of the running program,
 * per launch path, against EmuTOS (references/emutos @ 1a7484ab;
 * docs/api/gemdos/process/_parent-and-environment.md):
 *
 *   p_parent = run at proc_go (bdos/proc.c:541): an AUTO program
 *     &initial_basepage (bios/bios.c:693, bdos/bdosmain.c:339), a desktop /
 *     #Z program the AES PD (aes/gemshlib.c:645), an accessory 0 (never
 *     proc_go'ed, util/gemdos.c:34), a Pexec child its caller.
 *   p_env = a copy of run->p_env or of the given block (alloc_env,
 *     bdos/proc.c:413-432): AUTO {0,0} (bdosmain.c:81,341), the AES's
 *     default_env "PATH=\0X:\\\0\0" (bios/bios.c:626-636) for the rest.
 *   ad_envrn = the AES PD's p_env (aes/gemstart.S:78-96): shel_envrn points
 *     into the parent's (= AES PD's) environment.
 *
 * Built per role (build-pparent.sh):
 *   PPAUTO.NCP  -DPP_AUTO            C:\AUTO\ (console, before the AES)
 *   PPZ.NAP     -DPP_GEM -DPP_TAG="Z"    the #Z autorun program, runs itself
 *               -DPP_SELF="C:\\PPZ.NAP"  as a Pexec child (tail = its PD)
 *   PPDESK.NAP  -DPP_GEM -DPP_TAG="DESK" double-clicked on the desktop
 *               -DPP_SELF="C:\\PPD\\PPDESK.NAP"
 *   PPACC.NAC   -DPP_ACC             C:\ACC\ (a libc accessory)
 * Every role prints "[PP <tag>] ..." lines and "PPARENT <tag>: PASS|FAIL";
 * the child prints "PPARENT <tag>-KID: PASS|FAIL" and exits 0x42 on PASS.
 * The values are read through the program's own pointers: the stand-in
 * chain must be readable in every protection mode (the stand-in page is
 * published read-only). A child does NOT read its parent's basepage (with
 * memprot=strict it may not: a process's TPA is PROT_P) -- it compares its
 * p_parent with the value the parent passed in the command tail.
 */
#include "common.h"
#include <gem.h>
#include <string.h>

extern short _app;

#ifndef PP_TAG
#define PP_TAG "?"
#endif

static unsigned long ul(unsigned long v) { return v & 0xffffffffUL; }

static void putenv_bytes(const char *what, const unsigned char *e, int n)
{
	static const char d[] = "0123456789abcdef";
	char b[4];
	int i;
	Cconws("[PP " PP_TAG "] ");
	Cconws(what);
	Cconws(" =");
	for (i = 0; i < n; i++) {
		b[0] = ' ';
		b[1] = d[e[i] >> 4];
		b[2] = d[e[i] & 15];
		b[3] = 0;
		Cconws(b);
	}
	Cconws("\r\n");
}

static void show_pd(const char *what, const BASEPAGE *p)
{
	Cconws("[PP " PP_TAG "] ");
	Cconws(what);
	Cconws(" PD ");
	puthex((unsigned long)p);
	Cconws(" p_lowtpa ");
	puthex(p->p_lowtpa);
	Cconws(" p_hitpa ");
	puthex(p->p_hitpa);
	Cconws(" p_parent ");
	puthex(p->p_parent);
	Cconws(" p_flags ");
	puthex(p->p_reserved);
	Cconws(" p_env ");
	puthex(p->p_env);
	Cconws("\r\n");
}

/* default_env for the boot drive (bios/bios.c:626-636), envsize 11 */
static void default_env(unsigned char *e)
{
	unsigned short bootdev = *(volatile unsigned short *)0x446UL;
	memset(e, 0, 12);
	memcpy(e, "PATH=", 5);
	e[6] = (unsigned char)('A' + bootdev);
	e[7] = ':';
	e[8] = '\\';
}

/* TASK/PORT09: a libc program's start-up (cmdargs.c = mintlib parseargs)
 * joins the desktop form "PATH=\0X:\\\0" IN PLACE to "PATH=X:\\"
 * (crtinit.c:332-345), so after main() is entered its own block -- and a
 * Pexec(.., NULL) child's copy of it -- reads "PATH=X:\\\0\0". The kernel's
 * delivery is unchanged; what the program finds is mintlib's rewrite. */
static void joined_env(unsigned char *e)
{
	unsigned short bootdev = *(volatile unsigned short *)0x446UL;
	memset(e, 0, 12);
	memcpy(e, "PATH=", 5);
	e[5] = (unsigned char)('A' + bootdev);
	e[6] = ':';
	e[7] = '\\';
}

extern char **environ;

static const unsigned char *envp(const BASEPAGE *p)
{
	return (const unsigned char *)(unsigned long)p->p_env;
}

#if defined(PP_AUTO)
int main(void)
{
	const BASEPAGE *me = _base;
	const BASEPAGE *ib = (const BASEPAGE *)(unsigned long)me->p_parent;

	show_pd("self", me);
	putenv_bytes("own env", envp(me), 4);
	check("a program to the start-up code (_app == 1)", _app == 1);
	check("p_parent != 0 (not an accessory)", ib != 0);
	if (ib) {
		static const signed char dh[6] = { -1, -1, -2, -3, -1, -1 };
		show_pd("parent", ib);
		putenv_bytes("parent env", envp(ib), 2);
		checkhex("initial_basepage: p_parent", ib->p_parent, 0);
		checkhex("initial_basepage: p_flags = PF_STANDARD", ib->p_reserved, 7);
		check("initial_basepage: p_env = double_nul",
		      ib->p_env != 0 && envp(ib)[0] == 0 && envp(ib)[1] == 0);
		check("initial_basepage: p_uft = default_handle",
		      memcmp((const void *)ib->p_junk, dh, 6) == 0);
	}
	check("own env = {0, 0} (copy of double_nul)",
	      me->p_env != 0 && envp(me)[0] == 0 && envp(me)[1] == 0);
	check("own env is a copy, not the parent's block",
	      ib == 0 || me->p_env != ib->p_env);
	Cconws(fails ? "PPARENT " PP_TAG ": FAIL\r\n" : "PPARENT " PP_TAG ": PASS\r\n");
	return fails;
}
#elif defined(PP_ACC)
int main(void)
{
	const BASEPAGE *me = _base;
	unsigned char want[12];
	short id, msg[8];

	default_env(want);
	show_pd("self", me);
	putenv_bytes("own env", envp(me), 12);
	check("an accessory to the start-up code (_app == 0)", _app == 0);
	checkhex("p_parent == 0 (never proc_go'ed)", me->p_parent, 0);
	check("own env = default_env (copy of ad_envrn)",
	      me->p_env != 0 && memcmp(envp(me), want, 11) == 0);
	Cconws(fails ? "PPARENT " PP_TAG ": FAIL\r\n" : "PPARENT " PP_TAG ": PASS\r\n");
	id = appl_init();
	menu_register(id, "  PPARENT probe");
	for (;;)
		evnt_mesag(msg);
	return 0;
}
#else /* PP_GEM */
static int kid(void)
{
	const BASEPAGE *me = _base;
	unsigned long want = tail_value((const char *)me->p_cmdlin);
	unsigned char env[12];

	joined_env(env);
	show_pd("kid self", me);
	putenv_bytes("kid env", envp(me), 12);
	check("kid: a program (_app == 1)", _app == 1);
	checkhex("kid: p_parent = the caller's PD", ul(me->p_parent), ul(want));
	check("kid: env = a copy of the caller's (PATH=X:\\ as the caller's start-up left it)",
	      me->p_env != 0 && memcmp(envp(me), env, 10) == 0);
	check("kid: environ = {PATH=X:\\}", environ && environ[0]
	      && memcmp(environ[0], env, 9) == 0 && environ[1] == 0);
	Cconws(fails ? "PPARENT " PP_TAG "-KID: FAIL\r\n" : "PPARENT " PP_TAG "-KID: PASS\r\n");
	return fails ? 1 : 0x42;
}

int main(void)
{
	const BASEPAGE *me = _base;
	const BASEPAGE *aes, *ib;
	unsigned char want[12];
	char tail[12];
	char *val = 0;
	long rc;

	if (me->p_cmdlin[0] == 8)
		return kid();

	default_env(want);
	show_pd("self", me);
	putenv_bytes("own env", envp(me), 12);
	check("a program to the start-up code (_app == 1)", _app == 1);
	aes = (const BASEPAGE *)(unsigned long)me->p_parent;
	check("p_parent != 0 (not an accessory)", aes != 0);
	if (aes) {
		show_pd("parent (AES PD)", aes);
		putenv_bytes("parent env", envp(aes), 12);
		checkhex("AES PD: p_lowtpa = itself", ul(aes->p_lowtpa), ul((unsigned long)aes));
		checkhex("AES PD: p_flags = PF_STANDARD", aes->p_reserved, 7);
		checkhex("AES PD: p_xdta = &p_cmdlin", ul(aes->p_dta), ul((unsigned long)aes + 0x80));
		check("AES PD: p_env = default_env",
		      aes->p_env != 0 && memcmp(envp(aes), want, 12) == 0);
		ib = (const BASEPAGE *)(unsigned long)aes->p_parent;
		check("AES PD: p_parent != 0 (initial_basepage)", ib != 0);
		if (ib) {
			show_pd("grandparent (initial_basepage)", ib);
			checkhex("initial_basepage: p_parent", ib->p_parent, 0);
			check("initial_basepage: p_env = double_nul",
			      ib->p_env != 0 && envp(ib)[0] == 0 && envp(ib)[1] == 0);
		}
		check("own env is a copy, not the AES's block", me->p_env != aes->p_env);
	}
	{
		unsigned char j[12];

		joined_env(j);
		check("own env = default_env (copy of ad_envrn), as the start-up "
		      "joined it: PATH=X:\\",
		      me->p_env != 0 && memcmp(envp(me), j, 11) == 0);
		check("environ = {PATH=X:\\}", environ && environ[0]
		      && memcmp(environ[0], j, 9) == 0 && environ[1] == 0);
	}

	/* ad_envrn is the AES PD's p_env (aes/gemstart.S:78-96) */
	appl_init();
	shel_envrn(&val, "PATH=");
	Cconws("[PP " PP_TAG "] shel_envrn(PATH=) -> ");
	puthex((unsigned long)val);
	Cconws("\r\n");
	/* The AES answers the NUL after "PATH=" (p_env + 5); gemlib's
	 * mt_shel_envrn steps over it for "PATH=" (a_shel_envrn.c:40-50, "bug
	 * of TOS"), so the program sees p_env + 6 = "X:\\". */
	if (aes)
		checkhex("shel_envrn points into the AES PD's p_env (+5, +1 by gemlib)",
			 ul((unsigned long)val), ul(aes->p_env + 6));
	check("shel_envrn value: X:\\ (after gemlib's PATH= fix)",
	      val != 0 && memcmp(val, want + 6, 4) == 0);
	appl_exit();

	/* a Pexec child: p_parent = me (proc_go), env = a copy of mine */
	tail_hex(tail, (unsigned long)me);
	rc = Pexec(0, PP_SELF, tail, 0L);
	checkhex("Pexec child (" PP_SELF ") passed", (unsigned long)rc, 0x42);

	Cconws(fails ? "PPARENT " PP_TAG ": FAIL\r\n" : "PPARENT " PP_TAG ": PASS\r\n");
	return fails;
}
#endif
