/*
 * p4probe.c -- TASK/PROC04 point 2 probe WITH expectations (EmuTOS): current
 * drive and directory per PD (docs/api/gemdos/process/_run-ownership-curdir.md
 * §4; directory/3b-dsetpath.md, 47-dgetpath.md, drive/0e-dsetdrv.md,
 * 19-dgetdrv.md). Extends the CWD part of
 * measurements/measurement-2026-10-17-proc03/method/p3probe.c.
 *
 * Card: C:\AUTO\P4AUTO.NCP, C:\P4Z.NAP (#Z), C:\P4Z2.NAP (shel_write),
 * C:\ACC\P4ACC.NAC, C:\SUB\P4E.NCP (only there), C:\SUB\P4KA.NCP (copy of
 * P4AUTO.NCP, only there), C:\SUB\P4KZ.NAP (copy of P4Z.NAP, only there),
 * C:\SUB\P4INNER.TXT (only there).
 *
 * Roles:
 *   P4AUTO.NCP  AUTO: PD+0x37 == 2, initial.p_curdrv == 2, root; Dsetpath(\SUB);
 *               Pexec(0,"P4KA.NCP") RELATIVE (exists only in \SUB) -> kid C;
 *               afterwards still \SUB, drive C:. Leaves \SUB set on exit.
 *   P4Z.NAP     #Z: starts at the ROOT (the AUTO program's \SUB is its own),
 *               PD+0x37 == 2, accdesk/AES/initial p_curdrv == 2; same kid test
 *               via "P4KZ.NAP"; then shel_write(C:\P4Z2.NAP) from \SUB.
 *   P4Z2.NAP    started by the shell after Z: starts in \SUB (sh_apdir = Z's
 *               directory, gemshlib.c:174-175 + sh_chdef/set_drvdir); then
 *               Dsetpath(\) and shel_write(EMUCON).
 *   P4E.NCP     run twice from EmuCON after "cd ..", "cd sub": starts in \SUB,
 *               PD+0x37 == 2, EmuCON PD p_curdrv == 2; moves itself to the root
 *               and to drive A: -- EmuCON must still find "p4e.ncp" relative
 *               in \SUB the second time.
 *   P4ACC.NAC   accessory: PD+0x37 == 2, Dgetdrv 2, Dgetpath "" (accdesk at
 *               load time). Its own Dsetpath semantics are B5, not probed.
 *
 * Kid protocol: tail length 9, tail[1] = 'C'. Checks inherit (\SUB, C:,
 * relative open), then Dsetpath("\") and Dsetdrv(0); Pterm 0x42 / 0x43.
 *
 * Every check prints "[P4 <tag>] ok|FAIL ..."; each role ends with
 * "P4 <tag> RESULT fails=<n>".
 */
#include "common.h"
#include <gem.h>
#include <string.h>

#ifndef P4_TAG
#define P4_TAG "?"
#endif

static void say(const char *s) { Cconws("[P4 " P4_TAG "] "); Cconws(s); }

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

static void result(void)
{
	Cconws("P4 " P4_TAG " RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
}

static unsigned curdrv_of(unsigned long pd)
{
	return ((const unsigned char *)pd)[0x37];
}

/* Dgetpath(buf, 0) compared with want; prints what it got. */
static void ckpath(const char *what, const char *want)
{
	char buf[128];
	long rc;
	memset(buf, 0x55, sizeof buf);
	rc = Dgetpath(buf, 0);
	say(strcmp(buf, want) == 0 && rc == 0 ? "ok   " : "FAIL ");
	Cconws(what); Cconws(" Dgetpath='"); Cconws(buf); Cconws("' want='"); Cconws(want); Cconws("'\r\n");
	if (rc != 0 || strcmp(buf, want) != 0) fails++;
}

static int opens(const char *name)
{
	long h = Fopen(name, 0);
	if (h > 0) Fclose((short)h);
	return h > 0;
}

static void ck_own_drive(void)
{
	ckx("own PD+0x37 (p_curdrv)", curdrv_of((unsigned long)_base), 2);
	ckx("Dgetdrv", (unsigned long)Dgetdrv(), 2);
}

static int kid(void)
{
	ckpath("kid inherits the parent's directory", "\\SUB");
	ckx("kid PD+0x37 inherited", curdrv_of((unsigned long)_base), 2);
	ck("kid opens P4INNER.TXT relative", opens("P4INNER.TXT"));
	ckx("kid Dsetpath(\\)", (unsigned long)Dsetpath("\\"), 0);
	ckpath("kid at the root", "");
	Dsetdrv(0);
	ckx("kid Dgetdrv after Dsetdrv(0)", (unsigned long)Dgetdrv(), 0);
	return fails ? 0x43 : 0x42;
}

#define IS_KID() (_base->p_cmdlin[0] == 9 && _base->p_cmdlin[1] == 'C')

/* The parent half: \SUB, a relative Pexec of a file that exists only there,
 * and the parent's state after the kid moved its own. */
static void parent_kid_test(const char *kidname)
{
	char tail[11] = { 9, 'C', '0', '0', '0', '0', '0', '0', '0', '0', 0 };
	long rc;
	ckx("Dsetpath(\\SUB)", (unsigned long)Dsetpath("\\SUB"), 0);
	ckpath("after Dsetpath", "\\SUB");
	ck("opens P4INNER.TXT relative", opens("P4INNER.TXT"));
	rc = Pexec(0, kidname, tail, 0L);
	ckx("relative Pexec of the kid (exists only in \\SUB) -> 0x42", (unsigned long)rc, 0x42);
	ckpath("parent's directory after the kid's Dsetpath(\\)", "\\SUB");
	ckx("parent's Dgetdrv after the kid's Dsetdrv(0)", (unsigned long)Dgetdrv(), 2);
	ckx("parent's PD+0x37 after the kid", curdrv_of((unsigned long)_base), 2);
	ck("parent still opens P4INNER.TXT relative", opens("P4INNER.TXT"));
}

#if defined(P4_AUTO)
int main(void)
{
	if (IS_KID()) return kid();
	ck_own_drive();
	if (_base->p_parent)
		ckx("initial_basepage p_curdrv", curdrv_of(_base->p_parent), 2);
	ckpath("AUTO starts at the root", "");
	parent_kid_test("P4KA.NCP");
	say("leaving \\SUB set at exit\r\n");
	result();
	return 0;
}
#elif defined(P4_ACC)
int main(void)
{
	short id, msg[8];
	ck_own_drive();
	ckpath("accessory: accdesk's directory at load time", "");
	id = appl_init();
	menu_register(id, "  P4 probe");
	result();
	for (;;) evnt_mesag(msg);
	return 0;
}
#elif defined(P4_EMU)
int main(void)
{
	ck_own_drive();
	if (_base->p_parent)
		ckx("EmuCON PD p_curdrv", curdrv_of(_base->p_parent), 2);
	ckpath("started by EmuCON after 'cd sub'", "\\SUB");
	ckx("Dsetpath(\\)", (unsigned long)Dsetpath("\\"), 0);
	Dsetdrv(0);
	say("moved itself to A:\\ before exit\r\n");
	result();
	return 0;
}
#else
int main(void)
{
	const BASEPAGE *acc = (const BASEPAGE *)(unsigned long)_base->p_parent;
	int z2 = _base->p_cmdlin[0] == 2;

	if (IS_KID()) return kid();
	ck_own_drive();
	if (acc && acc->p_parent) {
		const BASEPAGE *aes = (const BASEPAGE *)(unsigned long)acc->p_parent;
		ckx("accdesk p_curdrv", curdrv_of((unsigned long)acc), 2);
		ckx("AES PD p_curdrv", curdrv_of((unsigned long)aes), 2);
		if (aes->p_parent)
			ckx("initial_basepage p_curdrv", curdrv_of(aes->p_parent), 2);
	} else {
		ck("parent chain present", 0);
	}
	appl_init();
	if (z2) {
		ckpath("Z2 starts in its starter's directory (sh_apdir)", "\\SUB");
		ck("Z2 opens P4INNER.TXT relative", opens("P4INNER.TXT"));
		ckx("Z2 Dsetpath(\\)", (unsigned long)Dsetpath("\\"), 0);
		shel_write(1, 0, 1, "EMUCON", "");
		say("shel_write(EMUCON) done\r\n");
		result();
		appl_exit();
		return 0;
	}
	ckpath("Z starts at the root (AUTO's \\SUB was its own)", "");
	parent_kid_test("P4KZ.NAP");
	{
		char t[3] = { 2, 'Z', '2' };
		say("shel_write(C:\\P4Z2.NAP) from \\SUB\r\n");
		shel_write(1, 1, 1, "C:\\P4Z2.NAP", t);
	}
	result();
	appl_exit();
	return 0;
}
#endif
