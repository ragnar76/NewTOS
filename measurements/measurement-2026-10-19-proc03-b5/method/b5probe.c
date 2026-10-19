/*
 * b5probe.c -- TASK/PROC03 B5 option 2 (user decision 2026-10-18) probe WITH
 * expectations: what a desk accessory sees as `run`
 * (docs/api/gemdos/process/_run-ownership-curdir.md §1.2/§1.3/§6.3 D-7..D-9,
 * _parent-and-environment.md §1.1). EmuTOS pin 1a7484ab.
 *
 * One source, roles by -D:
 *   B5ACC.NAC  -DB5_ACC   accessory (C:\ACC\). Three phases:
 *     init  (accdesk is run, aes/geminit.c:894..739): *os_run != own PD, it is a
 *           stand-in with accdesk's shape (parent AES -> initial -> 0, p_tbase 0,
 *           p_hitpa p+0x100); Pexec kid K must see p_parent == that PD and
 *           return 0x42 to the accessory; *os_run unchanged afterwards.
 *     Z     (message 0x7B50 from B5Z carrying Z's PD): *os_run == Z's PD; DTA is
 *           still the accessory's own (Fgetdta == own p_dta, named deviation);
 *           Malloc a block (owner must stay the accessory); kid K sees
 *           p_parent == Z's PD and returns to the accessory; *os_run == Z after.
 *           Reply 0x7B51 with the phase's fail count in msg[3].
 *     desk  (timer, after Z ended, first *os_run that is neither accdesk nor the
 *           accessory): the desktop's PD. Kid K sees p_parent == it. The block
 *           taken during Z survives Z's Pterm: write/read it, Mfree == 0 (E15:
 *           only an owner may free; were Z the owner it would be gone -> EIMBA).
 *           -DB5_DEREF (memprot=off card only): the PD is a real basepage
 *           (p_lowtpa == PD) whose p_parent is accdesk (the desktop is a child of
 *           accdesk, _parent-and-environment.md §1.1).
 *   B5Z.NAP    -DB5_GEM   #Z program: sends its PD to the accessory, waits for the
 *                         reply, ends; also the kid binary (role 'K').
 *
 * Every check prints "[B5 <tag>] ok|FAIL ..."; "B5 ACC RESULT fails=<n>" after
 * the desk phase, "B5 Z RESULT fails=<n>" (n includes the accessory's Z phase).
 */
#include "common.h"
#include <gem.h>
#include <string.h>

#ifndef B5_TAG
#define B5_TAG "?"
#endif

#define MSG_Z2ACC 0x7B50
#define MSG_ACC2Z 0x7B51

static void say(const char *s) { Cconws("[B5 " B5_TAG "] "); Cconws(s); }

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

static void showx(const char *what, unsigned long v)
{
	say(what); Cconws(" "); puthex(v); Cconws("\r\n");
}

static long run_kid(const char *self, char role, unsigned long v)
{
	char tail[12];
	tail_hex(tail, v);
	memmove(tail + 2, tail + 1, 9);
	tail[0] = 9; tail[1] = role;
	return Pexec(0, self, tail, 0L);
}

#define IS_KID() (_base->p_cmdlin[0] == 9)

#if defined(B5_ACC)
static unsigned long accdesk, zpd;
static void *zblock;

/* A stand-in PD is readable in every protection mode (published PROT_PR). */
static void ck_accdesk_shape(unsigned long p)
{
	const BASEPAGE *a = (const BASEPAGE *)p;
	const BASEPAGE *aes, *ib;
	ckx("init: accdesk.p_hitpa == self+0x100 (D-2)", a->p_hitpa, p + 0x100);
	ckx("init: accdesk.p_tbase == 0 (D-1)", a->p_tbase, 0);
	ck("init: accdesk.p_parent != 0", a->p_parent != 0);
	if (!a->p_parent) return;
	aes = (const BASEPAGE *)(unsigned long)a->p_parent;
	ck("init: accdesk -> AES -> initial", aes->p_parent != 0);
	if (!aes->p_parent) return;
	ib = (const BASEPAGE *)(unsigned long)aes->p_parent;
	ckx("init: initial.p_parent == 0", ib->p_parent, 0);
}

static void phase_z(short zid)
{
	int before = fails;
	short reply[8];
	long rc;
	showx("Z: *os_run", os_run_pd());
	ckx("Z: *os_run == Z's PD (run = the program in front)", os_run_pd(), zpd);
	ckx("Z: Fgetdta == own p_dta (DTA stays with the accessory)", (unsigned long)Fgetdta(), _base->p_dta);
	zblock = (void *)Malloc(4096);
	ck("Z: Malloc(4096) during Z", zblock != 0);
	if (zblock) memset(zblock, 0x5A, 4096);
	rc = run_kid("C:\\B5Z.NAP", 'K', zpd);
	ckx("Z: kid K (p_parent == Z's PD) returned to the accessory", (unsigned long)rc, 0x42);
	ckx("Z: *os_run == Z's PD after the kid", os_run_pd(), zpd);
	memset(reply, 0, sizeof reply);
	reply[0] = MSG_ACC2Z; reply[1] = 0; reply[3] = (short)(fails - before);
	appl_write(zid, 16, reply);
}

static int phase_desk(void)
{
	unsigned long x = os_run_pd();
	long rc;
	if (x == accdesk || x == (unsigned long)_base)
		return 0; /* not yet the desktop: try at the next timer */
	showx("desk: *os_run", x);
	ck("desk: *os_run != 0", x != 0);
#ifdef B5_DEREF
	ckx("desk: run PD p_lowtpa == PD (a real basepage)", ((const BASEPAGE *)x)->p_lowtpa, x);
	ckx("desk: run PD p_parent == accdesk (the desktop)", ((const BASEPAGE *)x)->p_parent, accdesk);
#endif
	rc = run_kid("C:\\B5Z.NAP", 'K', x);
	ckx("desk: kid K (p_parent == run) returned to the accessory", (unsigned long)rc, 0x42);
	ckx("desk: *os_run unchanged after the kid", os_run_pd(), x);
	if (zblock) {
		unsigned char *b = zblock;
		ck("desk: Z-phase block intact after Z's Pterm", b[0] == 0x5A && b[4095] == 0x5A);
		ckx("desk: Mfree(Z-phase block) == 0 (accessory still owns it)", (unsigned long)Mfree(zblock), 0);
	} else {
		ck("desk: Z-phase block exists", 0);
	}
	Cconws("B5 ACC RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
	return 1;
}

int main(void)
{
	short id, msg[8], d, ev;
	unsigned long r;
	long rc;
	int seen_z = 0, done = 0, tries = 0;

	ckx("accessory p_parent == 0", _base->p_parent, 0);
	r = os_run_pd();
	showx("init: own PD", (unsigned long)_base);
	showx("init: *os_run", r);
	ck("init: *os_run != own PD (run = accdesk, not the accessory)", r != (unsigned long)_base);
	ck("init: *os_run != 0", r != 0);
	accdesk = r;
	if (r && r != (unsigned long)_base)
		ck_accdesk_shape(r);
	id = appl_init();
	menu_register(id, "  B5 probe");
	rc = run_kid("C:\\B5Z.NAP", 'K', accdesk);
	ckx("init: kid K (p_parent == accdesk) returned to the accessory", (unsigned long)rc, 0x42);
	ckx("init: *os_run unchanged after the kid", os_run_pd(), accdesk);
	for (;;) {
		ev = evnt_multi(MU_MESAG | (seen_z && !done ? MU_TIMER : 0), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
				msg, 1500UL, &d, &d, &d, &d, &d, &d);
		if ((ev & MU_MESAG) && msg[0] == MSG_Z2ACC) {
			zpd = ((unsigned long)(unsigned short)msg[3] << 16) | (unsigned short)msg[4];
			phase_z(msg[1]);
			seen_z = 1;
		}
		if ((ev & MU_TIMER) && seen_z && !done) {
			done = phase_desk();
			if (!done && ++tries > 20) {
				ck("desk: run became the desktop within 20 timer events", 0);
				Cconws("B5 ACC RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
				done = 1;
			}
		}
	}
	return 0;
}
#else
static int kid(void)
{
	char t[10];
	unsigned long v;
	t[0] = 8; memcpy(t + 1, _base->p_cmdlin + 2, 8); t[9] = 0;
	v = tail_value(t);
	if (_base->p_cmdlin[1] != 'K') return 0x44;
	showx("kid p_parent", _base->p_parent);
	ckx("kid p_parent == expected run", _base->p_parent, v);
	ckx("kid *os_run == own PD", os_run_pd(), (unsigned long)_base);
	return fails ? 0x43 : 0x42;
}

int main(void)
{
	short msg[8], acc, me;
	unsigned long pd = (unsigned long)_base;
	int n;
	if (IS_KID()) return kid();
	me = appl_init();
	showx("own PD", pd);
	ckx("*os_run == own PD (a program sees itself)", os_run_pd(), pd);
	acc = appl_find("B5ACC   ");
	ck("appl_find(B5ACC)", acc >= 0);
	if (acc >= 0) {
		memset(msg, 0, sizeof msg);
		msg[0] = MSG_Z2ACC; msg[1] = me;
		msg[3] = (short)(pd >> 16); msg[4] = (short)(pd & 0xFFFF);
		appl_write(acc, 16, msg);
		for (n = 0; n < 50; n++) {
			evnt_mesag(msg);
			if (msg[0] == MSG_ACC2Z) break;
		}
		ckx("accessory's Z phase fails", (unsigned long)(unsigned short)msg[3], 0);
		if (msg[0] != MSG_ACC2Z) ck("reply from the accessory", 0);
	}
	Cconws("B5 Z RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
	appl_exit();
	return 0;
}
#endif
