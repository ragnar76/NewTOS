/*
 * p4redir.c -- TASK/PROC04 point 1 (= TASK/CON02) probe WITH expectations (EmuTOS): the GEMDOS character
 * functions follow the caller's standard handles run->p_uft[]. A standard handle forced to a FILE is answered
 * by osif itself (bdos/bdosmain.c:498-568): raw bytes, writes return 0, one-byte reads, Cconrs byte-wise up to
 * CR (the byte after it eaten), Cconin at EOF = EOF_INDICATOR 0x0000ff1a, Cconis 0 at EOF, Cconos -1; Crawio
 * (stdio_typ 0) is never redirected to the file. Spec: docs/api/gemdos/_dispatcher.md section 5.
 *
 *   -DKID  -> RKID.NCP (C:\RKID.NCP): the child. Everything it does goes through the redirected handles:
 *             Cconws("KID ") / Cconout('A') / Cconout(TAB) / Crawio('Z') / Cconrs / Cconin x2 / Cconis / Cconos,
 *             then reports the return values with Cconws/Cconout -- into the same redirected stdout.
 *             Exit code 0x42.
 *   default -> P4R.NCP (C:\AUTO\P4R.NCP, EL0 console program before the AES): the parent.
 *     A  Fcreate C:\ROUT.TXT, Fopen C:\RIN.TXT ("line one\r\nX"), Fdup(0/1) to keep the console, Fforce(1,out),
 *        Fforce(0,in), Pexec(0, C:\RKID.NCP), Fforce back, close; rc must be 0x42 and C:\ROUT.TXT must be EXP.
 *     B  mintlib system("C:\RKID.NCP <C:\RIN.TXT >C:\RSYS.TXT") (system_hack: no /bin/sh, no $SHELL -> Fforce +
 *        spawnvp); rc 0x42, C:\RSYS.TXT must be EXP.
 *     C  the parent's own Cconws after both: back on the console (this report itself is the evidence).
 * Every check prints "[RD] ok|FAIL ..."; ends with "RD RESULT fails=<n>".
 */
#include "common.h"
#include <stdlib.h>

#ifdef KID

static void hex(long v)
{
	static const char d[] = "0123456789abcdef";
	char b[9];
	int i;
	for (i = 0; i < 8; i++)
		b[i] = d[((unsigned long)v >> ((7 - i) * 4)) & 0xf];
	b[8] = 0;
	trap_1_wl((short)0x09, (long)b);
}

int main(void)
{
	char rec[48];
	long ws, out, cin, eofv, is, os;
	int i;

	ws = trap_1_wl((short)0x09, (long)"KID ");
	out = trap_1_ww((short)0x02, (short)'A');
	(void)trap_1_ww((short)0x02, (short)'\t');
	(void)trap_1_ww((short)0x06, (short)'Z');	/* Crawio: must NOT reach the file */
	rec[0] = 40;
	rec[1] = 0;
	(void)trap_1_wl((short)0x0A, (long)rec);
	cin = trap_1_w((short)0x01);
	eofv = trap_1_w((short)0x01);
	is = trap_1_w((short)0x0B);
	os = trap_1_w((short)0x10);

	trap_1_wl((short)0x09, (long)" ws="); hex(ws);
	trap_1_wl((short)0x09, (long)" out="); hex(out);
	trap_1_wl((short)0x09, (long)" rs="); hex(rec[1]);
	trap_1_wl((short)0x09, (long)" [");
	for (i = 0; i < rec[1] && i < 40; i++)
		trap_1_ww((short)0x02, (short)rec[2 + i]);
	trap_1_wl((short)0x09, (long)"] cin="); hex(cin);
	trap_1_wl((short)0x09, (long)" eof="); hex(eofv);
	trap_1_wl((short)0x09, (long)" is="); hex(is);
	trap_1_wl((short)0x09, (long)" os="); hex(os);
	trap_1_wl((short)0x09, (long)"\r\n");
	return 0x42;
}

#else

static const char EXP[] =
	"KID A\t ws=00000000 out=00000000 rs=00000008 [line one] cin=00000058 eof=0000ff1a is=00000000 os=ffffffff\r\n";

static void say(const char *s) { Cconws("[RD] "); Cconws(s); }

static void ckx(const char *what, long got, long want)
{
	say(got == want ? "ok   " : "FAIL "); Cconws(what);
	Cconws(" got="); puthex((unsigned long)got); Cconws(" want="); puthex((unsigned long)want);
	Cconws("\r\n");
	if (got != want) fails++;
}

/* Compare file `name` with EXP; print what was read on a mismatch (escaped). */
static void ckfile(const char *what, const char *name)
{
	static char buf[256];
	long h, n;
	int i, ok;

	h = Fopen(name, 0);
	if (h < 0) { ckx(what, h, 0); return; }
	n = Fread((short)h, sizeof(buf) - 1, buf);
	Fclose((short)h);
	ok = n == (long)(sizeof(EXP) - 1);
	for (i = 0; ok && i < n; i++)
		if (buf[i] != EXP[i]) ok = 0;
	say(ok ? "ok   " : "FAIL "); Cconws(what); Cconws(" len="); puthex((unsigned long)n);
	Cconws(ok ? " == EXP\r\n" : " content: ");
	if (!ok) {
		for (i = 0; i < n && i < 200; i++) {
			if (buf[i] >= 0x20 && buf[i] < 0x7f) Cconout(buf[i]);
			else { Cconws("\\x"); Cconout("0123456789abcdef"[(buf[i] >> 4) & 15]); Cconout("0123456789abcdef"[buf[i] & 15]); }
		}
		Cconws("\r\n");
		fails++;
	}
	Fdelete(name);
}

int main(void)
{
	long out, in, so, si, rc;

	Cconws("[RD] start: console redirection probe (TASK/PROC04 point 1)\r\n");

	/* A: Fforce + Pexec */
	out = Fcreate("C:\\ROUT.TXT", 0);
	in = Fopen("C:\\RIN.TXT", 0);
	so = Fdup(1);
	si = Fdup(0);
	if (out < 6 || in < 6 || so < 6 || si < 6) {
		ckx("A setup out", out, 6); ckx("A setup in", in, 6); ckx("A setup Fdup(1)", so, 6); ckx("A setup Fdup(0)", si, 6);
	} else {
		long f1 = Fforce(1, (short)out), f0 = Fforce(0, (short)in);
		rc = Pexec(0, "C:\\RKID.NCP", "\0", 0L);
		Fforce(1, (short)so);
		Fforce(0, (short)si);
		Fclose((short)so);
		Fclose((short)si);
		Fclose((short)out);
		Fclose((short)in);
		ckx("A Fforce(1,out)", f1, 0);
		ckx("A Fforce(0,in)", f0, 0);
		ckx("A Pexec rc (kid exit code)", rc, 0x42);
		ckfile("A C:\\ROUT.TXT", "C:\\ROUT.TXT");
	}

	/* B: mintlib system() with redirection */
	rc = system("C:\\RKID.NCP <C:\\RIN.TXT >C:\\RSYS.TXT");
	ckx("B system() rc", rc, 0x42);
	ckfile("B C:\\RSYS.TXT", "C:\\RSYS.TXT");

	/* C: the console is ours again -- this line is the check */
	ckx("C Cconws back on the console (return = count of \"x\")", Cconws("x"), 1);
	Cconws("\r\n");

	Cconws("RD RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
	return 0;
}

#endif
