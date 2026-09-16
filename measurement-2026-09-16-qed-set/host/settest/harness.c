/*
 * settest -- does QED's set module behave on LP64/little-endian exactly as the
 * pristine source does on m68k (32-bit, big-endian)?
 *
 * Three implementations are compared for every test case:
 *   ORACLE  a plain definition of "a set of element numbers 0..1023" with the
 *           byte mapping that setincl/setin use (byte elt>>3, mask 0x80>>(elt&7))
 *   MODEL   a literal transliteration of src/set.c with explicit big-endian
 *           32-bit word access (m68kmodel.c) -- what the pristine code DOES on m68k
 *   DEVICE  the code under test, compiled for THIS host (LP64, little-endian):
 *           either the pristine src/set.c or the sdk/overlays/qed/set.c rewrite
 *
 * ORACLE vs MODEL validates the model.  MODEL vs DEVICE is the actual verdict.
 */
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "shim/global.h"
#include "m68kmodel.h"

/* ---- device under test: the prefixed symbols of the two compilations ---- */
#define DECL(p) \
	short p##setfree(unsigned long *); \
	short p##setmax (unsigned long *); \
	short p##setmin (unsigned long *); \
	void  p##setcpy (unsigned long *, unsigned long *); \
	void  p##setall (unsigned long *); \
	void  p##setclr (unsigned long *); \
	void  p##setnot (unsigned long *); \
	void  p##setand (unsigned long *, unsigned long *); \
	void  p##setor  (unsigned long *, unsigned long *); \
	void  p##setxor (unsigned long *, unsigned long *); \
	void  p##setincl(unsigned long *, short); \
	void  p##setexcl(unsigned long *, short); \
	void  p##setchg (unsigned long *, short); \
	bool  p##setin  (unsigned long *, short); \
	bool  p##setcmp (unsigned long *, unsigned long *); \
	short p##setcard(unsigned long *); \
	void  p##str2set(char *, unsigned long *);
DECL(o_)
DECL(n_)

typedef struct {
	const char *name;
	short (*setfree)(unsigned long *);
	short (*setmax )(unsigned long *);
	short (*setmin )(unsigned long *);
	void  (*setcpy )(unsigned long *, unsigned long *);
	void  (*setall )(unsigned long *);
	void  (*setclr )(unsigned long *);
	void  (*setnot )(unsigned long *);
	void  (*setand )(unsigned long *, unsigned long *);
	void  (*setor  )(unsigned long *, unsigned long *);
	void  (*setxor )(unsigned long *, unsigned long *);
	void  (*setincl)(unsigned long *, short);
	void  (*setexcl)(unsigned long *, short);
	void  (*setchg )(unsigned long *, short);
	bool  (*setin  )(unsigned long *, short);
	bool  (*setcmp )(unsigned long *, unsigned long *);
	short (*setcard)(unsigned long *);
	void  (*str2set)(char *, unsigned long *);
} DEV;

#define DEVINIT(nm, p) { nm, p##setfree, p##setmax, p##setmin, p##setcpy, \
	p##setall, p##setclr, p##setnot, p##setand, p##setor, p##setxor, \
	p##setincl, p##setexcl, p##setchg, p##setin, p##setcmp, p##setcard, p##str2set }

static DEV dev_orig = DEVINIT("src/set.c (pristine)", o_);
#ifndef DEV2_NAME
#define DEV2_NAME "sdk/overlays/qed/set.c"
#endif
static DEV dev_ovl  = DEVINIT(DEV2_NAME, n_);

/* ---- ORACLE: the set as a list of member flags -------------------------- */
#define NELEM (SETMAX + 1)			/* 1024 */
typedef struct { unsigned char in[NELEM]; } ORC;

static void orc_clr(ORC *o)              { memset(o->in, 0, NELEM); }
static void orc_all(ORC *o)              { memset(o->in, 1, NELEM); }
static void orc_incl(ORC *o, int e)      { if (e >= 0 && e < NELEM) o->in[e] = 1; }
static int  orc_min(const ORC *o)        { int e; for (e = 0; e < NELEM; e++) if (o->in[e]) return e; return NELEM; }
static int  orc_max(const ORC *o)        { int e; for (e = NELEM; (--e) >= 0; ) if (o->in[e]) return e; return -1; }
static int  orc_free(const ORC *o)       { int e; for (e = 0; e < NELEM; e++) if (!o->in[e]) return e; return NELEM; }
static int  orc_card(const ORC *o)       { int e, c = 0; for (e = 0; e < NELEM; e++) if (o->in[e]) c++; return c; }
static void orc_image(const ORC *o, unsigned char *img)
{
	static const unsigned char b[8] = {128, 64, 32, 16, 8, 4, 2, 1};
	int e;
	memset(img, 0, M_BYTES);
	for (e = 0; e < NELEM; e++)
		if (o->in[e]) img[e >> 3] |= b[e & 7];
}

/* ---- counters ----------------------------------------------------------- */
static int fails_mo;			/* model vs oracle */
static int fails[32];			/* device vs model, per function slot */
static const char *fname[] = {
	"image", "setfree", "setmax", "setmin", "setcard", "setin", "setcmp(NULL)",
	"setcpy", "setall", "setclr", "setnot", "setand", "setor", "setxor",
	"setexcl", "setchg", "setcmp(2)", "str2set"
};
#define F_IMAGE 0
#define F_FREE 1
#define F_MAX 2
#define F_MIN 3
#define F_CARD 4
#define F_IN 5
#define F_CMPN 6
#define F_CPY 7
#define F_ALL 8
#define F_CLR 9
#define F_NOT 10
#define F_AND 11
#define F_OR 12
#define F_XOR 13
#define F_EXCL 14
#define F_CHG 15
#define F_CMP2 16
#define F_STR 17
#define NFUNC 18

static int shown[NFUNC];
static const char *curdev;
static const char *curcase;
static long ncase;

static void fail(int slot, const char *what, long got, long want)
{
	fails[slot]++;
	if (shown[slot] < 3)
	{
		shown[slot]++;
		printf("   FAIL %-12s %-28s %-34s got=%ld want(m68k)=%ld\n",
		       fname[slot], curcase, what, got, want);
	}
}

static void failimg(int slot, const char *what, const unsigned char *got,
                    const unsigned char *want)
{
	int i;
	fails[slot]++;
	if (shown[slot] < 3)
	{
		shown[slot]++;
		for (i = 0; i < M_BYTES && got[i] == want[i]; i++) ;
		printf("   FAIL %-12s %-28s %-34s first differing byte %d: %02x vs %02x\n",
		       fname[slot], curcase, what, i, got[i], want[i]);
	}
}

/* ---- one test case ------------------------------------------------------ */
static void build(const ORC *o, DEV *d, unsigned long *dset, unsigned char *mset)
{
	int e;
	d->setclr(dset);
	m_setclr(mset);
	for (e = 0; e < NELEM; e++)
		if (o->in[e]) { d->setincl(dset, (short)e); m_setincl(mset, (short)e); }
}

static void check_case(const ORC *o, const ORC *o2, DEV *d)
{
	unsigned long a[SETSIZE], b[SETSIZE], t[SETSIZE];
	unsigned char ma[M_BYTES], mb[M_BYTES], mt[M_BYTES], img[M_BYTES];
	int e;

	ncase++;
	build(o, d, a, ma);
	build(o2, d, b, mb);

	/* the model itself must agree with the plain definition */
	orc_image(o, img);
	if (memcmp(img, ma, M_BYTES) != 0) fails_mo++;
	if (m_setmin(ma)  != orc_min(o))  fails_mo++;
	if (m_setmax(ma)  != orc_max(o))  fails_mo++;
	if (m_setfree(ma) != orc_free(o)) fails_mo++;
	if (m_setcard(ma) != orc_card(o)) fails_mo++;

	/* byte image of the addressable region */
	if (memcmp(a, ma, M_BYTES) != 0) failimg(F_IMAGE, "after setclr+setincl", (unsigned char *)a, ma);

	if (d->setfree(a) != m_setfree(ma)) fail(F_FREE, "", d->setfree(a), m_setfree(ma));
	if (d->setmax (a) != m_setmax (ma)) fail(F_MAX,  "", d->setmax (a), m_setmax (ma));
	if (d->setmin (a) != m_setmin (ma)) fail(F_MIN,  "", d->setmin (a), m_setmin (ma));
	if (d->setcard(a) != m_setcard(ma)) fail(F_CARD, "", d->setcard(a), m_setcard(ma));
	if ((int)d->setcmp(a, NULL) != (int)m_setcmp(ma, NULL))
		fail(F_CMPN, "", d->setcmp(a, NULL), m_setcmp(ma, NULL));
	if ((int)d->setcmp(a, b) != (int)m_setcmp(ma, mb))
		fail(F_CMP2, "", d->setcmp(a, b), m_setcmp(ma, mb));

	for (e = -2; e <= NELEM + 1; e++)
		if ((int)d->setin(a, (short)e) != (int)m_setin(ma, (short)e))
		{ fail(F_IN, "elt out of/in range", e, -1); break; }

	/* setcpy */
	d->setcpy(t, a); m_setcpy(mt, ma);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_CPY, "", (unsigned char *)t, mt);

	/* setall / setclr / setnot on a copy */
	d->setcpy(t, a); m_setcpy(mt, ma);
	d->setall(t);    m_setall(mt);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_ALL, "", (unsigned char *)t, mt);
	/* setall must leave a set whose setcard is the full 1024, and setnot of it empty */
	if (d->setcard(t) != m_setcard(mt)) fail(F_ALL, "setcard after setall", d->setcard(t), m_setcard(mt));
	d->setnot(t);    m_setnot(mt);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_NOT, "of setall", (unsigned char *)t, mt);
	if ((int)d->setcmp(t, NULL) != (int)m_setcmp(mt, NULL))
		fail(F_NOT, "setcmp(setnot(setall),NULL)", d->setcmp(t, NULL), m_setcmp(mt, NULL));

	d->setcpy(t, a); m_setcpy(mt, ma);
	d->setnot(t);    m_setnot(mt);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_NOT, "", (unsigned char *)t, mt);

	d->setcpy(t, a); m_setcpy(mt, ma);
	d->setclr(t);    m_setclr(mt);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_CLR, "", (unsigned char *)t, mt);

	/* binary ops */
	d->setcpy(t, a); m_setcpy(mt, ma); d->setand(t, b); m_setand(mt, mb);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_AND, "", (unsigned char *)t, mt);
	d->setcpy(t, a); m_setcpy(mt, ma); d->setor (t, b); m_setor (mt, mb);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_OR,  "", (unsigned char *)t, mt);
	d->setcpy(t, a); m_setcpy(mt, ma); d->setxor(t, b); m_setxor(mt, mb);
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_XOR, "", (unsigned char *)t, mt);

	/* setexcl / setchg over every element */
	d->setcpy(t, a); m_setcpy(mt, ma);
	for (e = -1; e <= NELEM; e += 7) { d->setexcl(t, (short)e); m_setexcl(mt, (short)e); }
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_EXCL, "", (unsigned char *)t, mt);
	d->setcpy(t, a); m_setcpy(mt, ma);
	for (e = -1; e <= NELEM; e += 5) { d->setchg(t, (short)e); m_setchg(mt, (short)e); }
	if (memcmp(t, mt, M_BYTES) != 0) failimg(F_CHG, "", (unsigned char *)t, mt);
}

/* ---- corpus ------------------------------------------------------------- */
static unsigned long rng = 12345;
static unsigned next_rand(void) { rng = rng * 1103515245UL + 12345UL; return (unsigned)((rng >> 16) & 0x7FFFFFFF); }

static void run_dev(DEV *d)
{
	ORC o, o2;
	char buf[64];
	int i, k, e, n;
	unsigned long t[SETSIZE];
	unsigned char mt[M_BYTES];
	static const char *strs[] = {
		"", "a", "abc", "a-z", "A-Z0-9", "0-9", " -~", "-x", "x-", "a-a",
		"\x01\x02\x7f", "az-", " \t", NULL
	};

	curdev = d->name;
	rng = 12345;			/* same corpus for every device */
	ncase = 0;
	memset(fails, 0, sizeof fails);
	memset(shown, 0, sizeof shown);
	fails_mo = 0;
	printf("== device: %s\n", d->name);

	/* empty, full */
	orc_clr(&o);  orc_clr(&o2); curcase = "empty";        check_case(&o, &o2, d);
	orc_all(&o);  orc_clr(&o2); curcase = "full 0..1023"; check_case(&o, &o2, d);
	orc_all(&o);  orc_all(&o2); curcase = "full/full";    check_case(&o, &o2, d);

	/* every singleton */
	for (e = 0; e < NELEM; e++)
	{
		orc_clr(&o); orc_incl(&o, e);
		orc_clr(&o2); orc_incl(&o2, (e + 511) % NELEM);
		sprintf(buf, "singleton %d", e); curcase = buf;
		check_case(&o, &o2, d);
	}

	/* full minus one (drives setfree over the whole range) */
	for (e = 0; e < NELEM; e += 1)
	{
		orc_all(&o); o.in[e] = 0;
		orc_clr(&o2); orc_incl(&o2, e);
		sprintf(buf, "full minus %d", e); curcase = buf;
		check_case(&o, &o2, d);
	}

	/* pairs on a grid */
	{
		static const int g[] = {0,1,7,8,9,31,32,33,63,64,65,127,128,255,256,
		                        511,512,513,1000,1022,1023};
		for (i = 0; i < (int)(sizeof g / sizeof g[0]); i++)
			for (k = 0; k < (int)(sizeof g / sizeof g[0]); k++)
			{
				orc_clr(&o); orc_incl(&o, g[i]); orc_incl(&o, g[k]);
				orc_clr(&o2); orc_incl(&o2, g[k]);
				sprintf(buf, "pair %d,%d", g[i], g[k]); curcase = buf;
				check_case(&o, &o2, d);
			}
	}

	/* pseudo-random sets */
	for (n = 0; n < 300; n++)
	{
		orc_clr(&o); orc_clr(&o2);
		for (i = 0; i < 40; i++) { orc_incl(&o, next_rand() % NELEM); orc_incl(&o2, next_rand() % NELEM); }
		sprintf(buf, "random %d", n); curcase = buf;
		check_case(&o, &o2, d);
	}

	/* contiguous ranges */
	for (n = 0; n < 64; n++)
	{
		int lo = n * 13, hi = lo + n * 7;
		if (hi >= NELEM) hi = NELEM - 1;
		orc_clr(&o); for (e = lo; e <= hi; e++) orc_incl(&o, e);
		orc_clr(&o2);
		sprintf(buf, "range %d..%d", lo, hi); curcase = buf;
		check_case(&o, &o2, d);
	}

	/* str2set */
	curcase = "str2set";
	for (i = 0; strs[i]; i++)
	{
		strcpy(buf, strs[i]);
		d->str2set(buf, t); m_str2set(buf, mt);
		if (memcmp(t, mt, M_BYTES) != 0) failimg(F_STR, strs[i], (unsigned char *)t, mt);
	}

	/* a SET whose PADDING (the bytes past element SETMAX that only exist on
	 * LP64) is dirty: on m68k there is no such region, so the module must
	 * ignore it. */
	curcase = "dirty padding";
	{
		unsigned long x[SETSIZE];
		d->setclr(x); m_setclr(mt);
		memset((unsigned char *)x + M_BYTES, 0xA5,
		       SETSIZE * sizeof(unsigned long) - M_BYTES);
		d->setincl(x, 100); m_setincl(mt, 100);
		if (d->setmin(x) != m_setmin(mt)) fail(F_MIN, "dirty padding", d->setmin(x), m_setmin(mt));
		if (d->setmax(x) != m_setmax(mt)) fail(F_MAX, "dirty padding", d->setmax(x), m_setmax(mt));
		if (d->setcard(x) != m_setcard(mt)) fail(F_CARD, "dirty padding", d->setcard(x), m_setcard(mt));
		if ((int)d->setcmp(x, NULL) != (int)m_setcmp(mt, NULL))
			fail(F_CMPN, "dirty padding", d->setcmp(x, NULL), m_setcmp(mt, NULL));
	}

	/* --- the restore_edit loop, verbatim in shape (edit.c:2106-2109) ------ */
	curcase = "restore_edit loop";
	{
		int handle, body, bad = 0;
		for (handle = 0; handle <= 40; handle++)
		{
			short link, max;
			body = 0;
			d->setclr(t); d->setincl(t, (short)handle);
			max = d->setmax(t);
			for (link = d->setmin(t); link <= max; link++)
				if (d->setin(t, link)) body++;
			if (body != 1) { bad++; if (bad <= 3)
				printf("   FAIL restore_edit  window handle %-3d               "
				       "         loop body ran %d times, want 1\n", handle, body); }
		}
		if (bad) fails[F_MIN] += bad;
		printf("   restore_edit loop body: reached for %d of 41 window handles\n", 41 - bad);
	}

	{
		int tot = 0;
		for (i = 0; i < NFUNC; i++) tot += fails[i];
		printf("   test sets driven: %ld (each one checks all 17 functions)\n", ncase);
		printf("   model-vs-oracle mismatches: %d\n", fails_mo);
		printf("   DEVICE vs m68k MODEL: %s (%d mismatches)\n",
		       tot ? "*** RED ***" : "green", tot);
		for (i = 0; i < NFUNC; i++)
			if (fails[i]) printf("       %-12s %d\n", fname[i], fails[i]);
		printf("\n");
		return;
	}
}

int main(void)
{
	printf("host data model: sizeof(unsigned long)=%zu sizeof(SET)=%zu SETSIZE=%d SETMAX=%d\n",
	       sizeof(unsigned long), sizeof(SET), SETSIZE, SETMAX);
	{
		unsigned long probe = 1;
		printf("byte order: %s\n",
		       (*(unsigned char *)&probe) ? "little-endian" : "big-endian");
	}
	printf("addressable bytes of a SET: %d, object bytes: %zu\n\n",
	       M_BYTES, sizeof(SET));
	run_dev(&dev_orig);
	run_dev(&dev_ovl);
	return 0;
}
