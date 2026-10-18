/*
 * p3s1probe.c -- TASK/PROC03 stage 1 + B4 probe WITH expectations (EmuTOS,
 * docs/api/gemdos/process/_parent-and-environment.md §1.1/§2/§3,
 * _run-ownership-curdir.md §2, §6.4 D-1..D-5). Derived from
 * measurements/measurement-2026-10-17-proc03/method/p3probe.c (read-only
 * probe), which it replaces for this run.
 *
 * One source, roles by -D:
 *   P3AUTO.NCP  -DP3_AUTO  C:\AUTO\ (before the AES): chain AUTO -> initial -> 0
 *   P3Z.NAP     -DP3_GEM   #Z autorun: chain Z -> accdesk -> AES -> initial -> 0,
 *                          stand-in fields, Pexec kid, B4 cases, D-4 (AES p_cmdlin
 *                          = ad_stail), shel_write(C:\P3Z2.NAP, tail "Z2")
 *   P3Z2.NAP    copy of P3Z.NAP started by shel_write (tail "Z2"): chain as Z,
 *                          shel_write(EMUCON), then a kid that bends its p_parent
 *                          to accdesk (B4: unwind through Z2 to the shell)
 *   P3ACC.NAC   -DP3_ACC   accessory: p_parent 0, env, Pexec kid
 *   P3E.NCP     -DP3_EMU   started by the in-kernel EmuCON ("run p3e.ncp"):
 *                          chain E -> EmuCON -> accdesk -> AES -> initial -> 0
 *
 * Kid protocol (Pexec child = the same program file, tail = <role char> +
 * 8 hex digits, length byte 9):
 *   K v  plain kid: p_parent == v ?                         -> Pterm(0x42)
 *   U v  starts grandkid "A v" (v = Z's PD); if it is ever resumed: FAIL, 0x99
 *   A v  grandkid: p_parent = v; Pterm(0x77)   (B4: v names an ancestor)
 *   V v  starts grandkid "X v"; expects rc 0x55 -> 0x42, else 0x43
 *   X v  grandkid: p_parent = v; Pterm(0x55)   (B4: v invalid / not resumable)
 *   R v  kid of Z2: p_parent = v (accdesk); Pterm(0x66) (B4: unwind to the shell)
 *
 * Every check prints "[P3S1 <tag>] ok|FAIL ..."; each role ends with
 * "P3S1 <tag> RESULT fails=<n>".
 */
#include "common.h"
#include <gem.h>
#include <string.h>

extern short _app;

#ifndef P3_TAG
#define P3_TAG "?"
#endif

#define P3_PF_STD 7UL
static const unsigned char aes_env_bytes[12] =
	{ 'P','A','T','H','=',0,'C',':','\\',0,0,0 };

static void say(const char *s) { Cconws("[P3S1 " P3_TAG "] "); Cconws(s); }

static void ck(const char *what, int ok)
{
	say(ok ? "ok   " : "FAIL "); Cconws(what); Cconws("\r\n");
	if (!ok) fails++;
}

static void ckx(const char *what, unsigned long got, unsigned long want)
{
	say(got == want ? "ok   " : "FAIL "); Cconws(what);
	Cconws(" got="); puthex(got); Cconws(" want="); puthex(want); Cconws("\r\n");
	if (got != want) fails++;
}

static void show_pd(const char *what, const BASEPAGE *p)
{
	say(what);
	Cconws(" PD "); puthex((unsigned long)p);
	Cconws(" lowtpa "); puthex(p->p_lowtpa);
	Cconws(" hitpa "); puthex(p->p_hitpa);
	Cconws(" tbase "); puthex(p->p_tbase);
	Cconws(" xdta "); puthex(p->p_dta);
	Cconws(" parent "); puthex(p->p_parent);
	Cconws(" flags "); puthex(p->p_reserved);
	Cconws(" env "); puthex(p->p_env);
	Cconws("\r\n");
}

static int env_is_aes(unsigned long env)
{
	return env && memcmp((const void *)env, aes_env_bytes, 11) == 0;
}

/* A libc program's own environment: the start-up code joins the TOS form
 * "PATH=\0C:\\" into "PATH=C:\\" in place (sdk/libc/tests/cmdargs-host.c:353-356),
 * so either form is the kernel's copy of the AES environment. */
static int own_env_ok(unsigned long env)
{
	const char *e = (const char *)env;
	if (!env || memcmp(e, "PATH=", 5) != 0) return 0;
	return (e[5] == 0 && memcmp(e + 6, "C:\\", 4) == 0) || memcmp(e + 5, "C:\\", 4) == 0;
}

static void result(void)
{
	Cconws("P3S1 " P3_TAG " RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
}

/* Depth as the spec counts it (§1.1): the PDs from the program up to and
 * including the 0 terminator. Only stand-ins are dereferenced (readable in
 * every protection mode); the caller passes how many levels are stand-ins. */
static int depth(const BASEPAGE *p)
{
	int n = 1;
	while (p->p_parent && n < 10) {
		p = (const BASEPAGE *)(unsigned long)p->p_parent;
		n++;
	}
	return n + 1;
}

/* A stand-in PD created like aes_run_rom_program does (spec §2.3/§2.4) with
 * the NewTOS decisions D-1 (p_tbase 0), D-2 (p_hitpa p+0x100), D-3 (p_xdta
 * &p_cmdlin). */
static void ck_rom_pd(const char *name, const BASEPAGE *p, unsigned long parent,
		      unsigned long parent_env)
{
	unsigned long a = (unsigned long)p;
	char w[64];
	show_pd(name, p);
	strcpy(w, name); strcat(w, ".p_lowtpa == self"); ckx(w, p->p_lowtpa, a);
	strcpy(w, name); strcat(w, ".p_hitpa == self+0x100 (D-2)"); ckx(w, p->p_hitpa, a + 0x100);
	strcpy(w, name); strcat(w, ".p_tbase == 0 (D-1)"); ckx(w, p->p_tbase, 0);
	strcpy(w, name); strcat(w, ".p_tlen/dlen/blen == 0"); ck(w, !p->p_tlen && !p->p_dlen && !p->p_blen);
	strcpy(w, name); strcat(w, ".p_xdta == &p_cmdlin (D-3)"); ckx(w, p->p_dta, a + 0x80);
	strcpy(w, name); strcat(w, ".p_parent"); ckx(w, p->p_parent, parent);
	strcpy(w, name); strcat(w, ".p_flags == PF_STANDARD"); ckx(w, p->p_reserved, P3_PF_STD);
	strcpy(w, name); strcat(w, ".p_env = own copy of parent env"); ck(w, p->p_env && p->p_env != parent_env);
	strcpy(w, name); strcat(w, ".env bytes = PATH=\\0C:\\\\\\0\\0"); ck(w, env_is_aes(p->p_env));
	strcpy(w, name); strcat(w, ".p_uft = default handles");
	ck(w, memcmp(p->p_junk, "\xff\xff\xfe\xfd\xff\xff", 6) == 0);
}

static void ck_aes_initial(const BASEPAGE *aes)
{
	const BASEPAGE *ib = (const BASEPAGE *)(unsigned long)aes->p_parent;
	show_pd("AES", aes);
	ckx("AES.p_lowtpa == self", aes->p_lowtpa, (unsigned long)aes);
	ckx("AES.p_flags", aes->p_reserved, P3_PF_STD);
	ck("AES env bytes", env_is_aes(aes->p_env));
	ck("AES.p_parent != 0", aes->p_parent != 0);
	if (!aes->p_parent) return;
	show_pd("initial", ib);
	ckx("initial.p_parent == 0", ib->p_parent, 0);
	ckx("initial.p_flags", ib->p_reserved, P3_PF_STD);
	ck("initial env = double_nul", ib->p_env && ((const char *)(unsigned long)ib->p_env)[0] == 0
	   && ((const char *)(unsigned long)ib->p_env)[1] == 0);
}

static long run_kid(const char *self, char role, unsigned long v)
{
	char tail[12];
	tail_hex(tail, v);              /* tail[0]=8, tail[1..8] = hex */
	memmove(tail + 2, tail + 1, 9); /* make room for the role char */
	tail[0] = 9; tail[1] = role;
	return Pexec(0, self, tail, 0L);
}

static unsigned long kid_value(const BASEPAGE *me)
{
	char t[10];
	t[0] = 8; memcpy(t + 1, me->p_cmdlin + 2, 8); t[9] = 0;
	return tail_value(t);
}

static int kid(const char *self)
{
	BASEPAGE *me = _base;
	char role = me->p_cmdlin[1];
	unsigned long v = kid_value(me);
	long rc;
	switch (role) {
	case 'K':
		ckx("kid p_parent == caller PD", me->p_parent, v);
		return fails ? 0x43 : 0x42;
	case 'U':
		rc = run_kid(self, 'A', v);
		say("FAIL B4 unwind: the kid between was resumed, rc="); puthex((unsigned long)rc); Cconws("\r\n");
		return 0x99;
	case 'A':
		me->p_parent = (unsigned int)v;
		Pterm(0x77);
		return 0;
	case 'V':
		rc = run_kid(self, 'X', v);
		say("B4 invalid p_parent "); puthex(v); Cconws(": kid got rc="); puthex((unsigned long)rc); Cconws("\r\n");
		return rc == 0x55 ? 0x42 : 0x43;
	case 'X':
		me->p_parent = (unsigned int)v;
		Pterm(0x55);
		return 0;
	case 'R':
		me->p_parent = (unsigned int)v;
		Pterm(0x66);
		return 0;
	}
	return 0x44;
}

#define IS_KID() (_base->p_cmdlin[0] == 9)

#if defined(P3_AUTO)
int main(void)
{
	const BASEPAGE *ib;
	if (IS_KID()) return kid("C:\\AUTO\\P3AUTO.NCP");
	show_pd("self", _base);
	ckx("depth AUTO -> initial -> 0", depth(_base), 3);
	ib = (const BASEPAGE *)(unsigned long)_base->p_parent;
	if (ib) {
		show_pd("initial", ib);
		ckx("initial.p_parent", ib->p_parent, 0);
		ckx("initial.p_flags", ib->p_reserved, P3_PF_STD);
	}
	ck("own env = 00 00", _base->p_env && !((char *)(unsigned long)_base->p_env)[0]
	   && !((char *)(unsigned long)_base->p_env)[1]);
	ckx("Pexec kid K", run_kid("C:\\AUTO\\P3AUTO.NCP", 'K', (unsigned long)_base), 0x42);
	result();
	return 0;
}
#elif defined(P3_ACC)
int main(void)
{
	short id, msg[8];
	ckx("_app", (unsigned long)_app, 0);
	show_pd("self", _base);
	ckx("accessory p_parent == 0", _base->p_parent, 0);
	ck("accessory env bytes (accdesk copy)", env_is_aes(_base->p_env));
	id = appl_init();
	menu_register(id, "  P3S1 probe");
	ckx("accessory Pexec kid K (caller = accessory today, B5 open)",
	    run_kid("C:\\P3Z.NAP", 'K', (unsigned long)_base), 0x42);
	result();
	for (;;) evnt_mesag(msg);
	return 0;
}
#elif defined(P3_EMU)
int main(void)
{
	const BASEPAGE *emu = (const BASEPAGE *)(unsigned long)_base->p_parent;
	const BASEPAGE *acc;
	show_pd("self", _base);
	ckx("depth E -> EmuCON -> accdesk -> AES -> initial -> 0", depth(_base), 6);
	ck("own env bytes (EmuCON copy)", own_env_ok(_base->p_env));
	if (emu && emu->p_parent) {
		acc = (const BASEPAGE *)(unsigned long)emu->p_parent;
		ck_rom_pd("EmuCON", emu, (unsigned long)acc, acc->p_env);
		ck_rom_pd("accdesk", acc, acc->p_parent,
			  ((const BASEPAGE *)(unsigned long)acc->p_parent)->p_env);
		ck_aes_initial((const BASEPAGE *)(unsigned long)acc->p_parent);
	} else {
		ck("parent chain present", 0);
	}
	result();
	return 0;
}
#else
static const char *self_path(void)
{
	return _base->p_cmdlin[0] == 2 ? "C:\\P3Z2.NAP" : "C:\\P3Z.NAP";
}

int main(void)
{
	const BASEPAGE *acc = (const BASEPAGE *)(unsigned long)_base->p_parent;
	const BASEPAGE *aes = 0;
	char *val = 0;
	long rc;
	int z2 = _base->p_cmdlin[0] == 2;

	if (IS_KID()) return kid(z2 ? "C:\\P3Z2.NAP" : "C:\\P3Z.NAP");
	ckx("_app", (unsigned long)_app, 1);
	show_pd("self", _base);
	ckx("depth X -> accdesk -> AES -> initial -> 0", depth(_base), 5);
	ck("own env bytes (copy of ad_envrn)", own_env_ok(_base->p_env));
	if (acc && acc->p_parent) {
		aes = (const BASEPAGE *)(unsigned long)acc->p_parent;
		ck_rom_pd("accdesk", acc, (unsigned long)aes, aes->p_env);
		ck_aes_initial(aes);
		ck("own env is not accdesk's block", _base->p_env != acc->p_env);
	} else {
		ck("parent chain present", 0);
	}
	appl_init();
	shel_envrn(&val, "PATH=");
	if (aes)
		ckx("shel_envrn(PATH=) in the AES PD's env (+6)", (unsigned long)val, aes->p_env + 6);

	if (z2) {
		/* D-4: ad_stail is the AES PD's p_cmdlin; Z wrote "\2Z2". */
		if (aes)
			ck("D-4 AES.p_cmdlin = ad_stail = 02 'Z' '2'",
			   aes->p_cmdlin[0] == 2 && aes->p_cmdlin[1] == 'Z' && aes->p_cmdlin[2] == '2');
		shel_write(1, 0, 1, "EMUCON", "");
		say("shel_write(EMUCON) done; kid R bends p_parent to accdesk\r\n");
		result();
		rc = run_kid(self_path(), 'R', (unsigned long)acc);
		say("FAIL B4 unwind to the shell: Z2 resumed, rc="); puthex((unsigned long)rc); Cconws("\r\n");
		fails++;
		result();
		appl_exit();
		return 0;
	}

	if (aes)
		ckx("D-4 AES.p_cmdlin[0] == #Z tail length (0)", (unsigned long)aes->p_cmdlin[0], 0);
	ckx("Pexec kid K", run_kid(self_path(), 'K', (unsigned long)_base), 0x42);
	rc = run_kid(self_path(), 'U', (unsigned long)_base);
	ckx("B4 grandkid bends p_parent to Z: Z resumes with its rc", (unsigned long)rc, 0x77);
	ckx("B4 p_parent 0x12345000 (not a PD): falls back to the kid",
	    (unsigned long)run_kid(self_path(), 'V', 0x12345000UL), 0x42);
	ckx("B4 p_parent 0: falls back to the kid",
	    (unsigned long)run_kid(self_path(), 'V', 0), 0x42);
	if (aes)
		ckx("B4 p_parent = AES stand-in (no resume point): falls back",
		    (unsigned long)run_kid(self_path(), 'V', (unsigned long)aes), 0x42);
	{
		char t[3] = { 2, 'Z', '2' };
		say("shel_write(C:\\P3Z2.NAP)\r\n");
		shel_write(1, 1, 1, "C:\\P3Z2.NAP", t);
	}
	if (aes)
		ck("D-4 after shel_write AES.p_cmdlin = 02 'Z' '2'",
		   aes->p_cmdlin[0] == 2 && aes->p_cmdlin[1] == 'Z' && aes->p_cmdlin[2] == '2');
	result();
	appl_exit();
	return 0;
}
#endif
