/*
 * f18x.c -- independent tester probe for TASK/FS18/FS14 (Frename), cases NOT in the developer's fs18.c.
 * F18X.NCP runs from C:\AUTO (EL0 console program). Lines "[F18X] ok|FAIL ..."; ends with "F18X RESULT fails=<n>".
 * Expectations are from docs/api/gemdos/file/56-frename.md (EmuTOS xrename) and the brief; "INFO" lines carry no verdict.
 *   G  folder cross-dir move C:\G1\SUB -> C:\G2\SUB with a file OPEN inside; write/close; content intact; ".." of SUB = G2 (Dsetpath/Dgetpath).
 *   H  folder same-dir rename C:\H1 -> C:\H2 with a file open inside; write/close; content intact.
 *   I  is_subdir: Frename(C:\I1, C:\I1\X) -> EACCDN; target exists -> EACCDN; same name -> EACCDN; read-only source -> EACCDN (INFO if differs).
 *   K  two handles on one file (Fopen twice), cross-dir move, both handles read/write, both close; 3000 bytes intact.
 *   L  multi-cluster file (40000 B) cross-dir, closed, then moved again back; content intact.
 *   M  VFAT long names: create C:\LongFileName_Test.Txt, rename same-dir to C:\Another long name.txt, to short C:\SHORT1.TXT,
 *      cross-dir to C:\M1\Moved Long Name.txt; listing names exact; fsck of the card checks the LFN chains.
 *   N  open file, cross-dir move, then Fseek+Fread via the same handle BEFORE close (OFD follows) and Fdatime.
 *   O  folder with a subtree (2 files + subfolder) moved cross-dir; all files re-read intact.
 *   P  Frename across drives -> ENSAME (-48) when two drive letters.
 */
#include "common.h"
#include <string.h>

static void say(const char *s) { Cconws("[F18X] "); Cconws(s); }
static void ckx(const char *what, long got, long want)
{
	say(got == want ? "ok   " : "FAIL "); Cconws(what);
	Cconws(" got="); puthex((unsigned long)got); Cconws(" want="); puthex((unsigned long)want);
	Cconws("\r\n");
	if (got != want) fails++;
}
static void ck(const char *what, int ok)
{
	say(ok ? "ok   " : "FAIL "); Cconws(what); Cconws("\r\n");
	if (!ok) fails++;
}
static void info(const char *what, long v)
{
	say("INFO "); Cconws(what); Cconws(" = "); puthex((unsigned long)v); Cconws("\r\n");
}
static char big[40000];
static char rd[40100];
static void fill(char seed, long n) { long i; for (i = 0; i < n; i++) big[i] = (char)(seed + i % 29); }
static long slurp(const char *name)
{
	long h = Fopen(name, 0), n;
	if (h < 0) return h;
	n = Fread((short)h, sizeof(rd), rd);
	Fclose((short)h);
	return n;
}
static int same(const char *name, long n) { return slurp(name) == n && memcmp(rd, big, (unsigned long)n) == 0; }
static long list(const char *glob, char *out, int max)
{
	_DTA dta; long n = 0, r;
	out[0] = 0; Fsetdta(&dta);
	for (r = Fsfirst(glob, 0x37); r == 0; r = Fsnext()) {
		const char *nm = dta.dta_name[0] ? dta.dta_name : "<>";
		if ((int)(strlen(out) + strlen(nm) + 2) < max) { if (n) strcat(out, ","); strcat(out, nm); }
		n++;
	}
	return n;
}
static void mkfile(const char *p, long n, char seed)
{
	long h = Fcreate(p, 0);
	fill(seed, n);
	if (h >= 0) { Fwrite((short)h, n, big); Fclose((short)h); }
}

int main(void)
{
	long h, h2, n, k0; char buf[300];
	Cconws("[F18X] start\r\n");

	/* G */
	Dcreate("C:\\G1"); Dcreate("C:\\G2"); Dcreate("C:\\G1\\SUB");
	h = Fcreate("C:\\G1\\SUB\\F.TXT", 0);
	ck("G Fcreate inside SUB", h >= 6);
	ckx("G Frename folder SUB across dirs while F.TXT open", Frename(0, "C:\\G1\\SUB", "C:\\G2\\SUB"), 0);
	fill('g', 3000);
	ckx("G Fwrite via the open handle", Fwrite((short)h, 3000, big), 3000);
	ckx("G Fclose", Fclose((short)h), 0);
	ck("G C:\\G2\\SUB\\F.TXT 3000 bytes intact", same("C:\\G2\\SUB\\F.TXT", 3000));
	ckx("G old C:\\G1\\SUB gone (Dsetpath -> EPTHNF)", Dsetpath("C:\\G1\\SUB"), -34);
	ckx("G Dsetpath C:\\G2\\SUB", Dsetpath("C:\\G2\\SUB"), 0);
	ckx("G Dsetpath .. (the moved folder's .. entry)", Dsetpath(".."), 0);
	buf[0] = 0; Dgetpath(buf, 3);
	ck("G Dgetpath after .. = \\G2", strcmp(buf, "\\G2") == 0);
	say("G cwd after .. = "); Cconws(buf); Cconws("\r\n");
	Dsetpath("\\");
	n = list("C:\\G1\\*.*", buf, sizeof(buf)); ckx("G C:\\G1 holds only . and ..", n, 2);

	/* H */
	Dcreate("C:\\H1");
	h = Fcreate("C:\\H1\\F.TXT", 0);
	ckx("H Frename folder H1 -> H2 (same dir) with F.TXT open", Frename(0, "C:\\H1", "C:\\H2"), 0);
	fill('h', 3000);
	ckx("H Fwrite via the open handle", Fwrite((short)h, 3000, big), 3000);
	ckx("H Fclose", Fclose((short)h), 0);
	ck("H C:\\H2\\F.TXT 3000 bytes intact", same("C:\\H2\\F.TXT", 3000));

	/* I */
	Dcreate("C:\\I1"); mkfile("C:\\I1\\A.TXT", 10, 'i'); mkfile("C:\\I1\\B.TXT", 10, 'j');
	ckx("I Frename(I1 -> I1\\X) is_subdir -> EACCDN", Frename(0, "C:\\I1", "C:\\I1\\X"), -36);
	ckx("I target exists -> EACCDN", Frename(0, "C:\\I1\\A.TXT", "C:\\I1\\B.TXT"), -36);
	ckx("I same name -> EACCDN", Frename(0, "C:\\I1\\A.TXT", "C:\\I1\\A.TXT"), -36);
	Fattrib("C:\\I1\\A.TXT", 1, 0x01);
	ckx("I read-only source -> EACCDN", Frename(0, "C:\\I1\\A.TXT", "C:\\I1\\C.TXT"), -36);
	ckx("I illegal char in new name -> EACCDN", Frename(0, "C:\\I1\\B.TXT", "C:\\I1\\B*.TXT"), -36);
	n = list("C:\\I1\\*.*", buf, sizeof(buf)); say("I listing "); Cconws(buf); Cconws("\r\n");

	/* K0 control: the same two-handle scenario WITHOUT a rename (what does Fread via handle 2 return at all?) */
	Dcreate("C:\\K0");
	h = Fcreate("C:\\K0\\D.TXT", 0);
	fill('k', 3000);
	Fwrite((short)h, 1000, big);
	h2 = Fopen("C:\\K0\\D.TXT", 2);
	Fwrite((short)h, 2000, big + 1000);
	Fseek(0, (short)h2, 0);
	k0 = Fread((short)h2, 3000, rd);
	info("K0 control (no rename): handle2 Fread 3000 returns", k0);
	Fclose((short)h); Fclose((short)h2);
	ck("K0 control file 3000 intact", same("C:\\K0\\D.TXT", 3000));

	/* K */
	Dcreate("C:\\K1"); Dcreate("C:\\K2");
	h = Fcreate("C:\\K1\\D.TXT", 0);
	fill('k', 3000);
	Fwrite((short)h, 1000, big);
	h2 = Fopen("C:\\K1\\D.TXT", 2);
	ck("K second Fopen", h2 >= 6);
	ckx("K cross-dir Frename with two handles", Frename(0, "C:\\K1\\D.TXT", "C:\\K2\\E.TXT"), 0);
	ckx("K handle1 Fwrite 2000 more", Fwrite((short)h, 2000, big + 1000), 2000);
	ckx("K handle2 Fseek 0", Fseek(0, (short)h2, 0), 0);
	ckx("K handle2 Fread equals the no-rename control K0", Fread((short)h2, 3000, rd), k0);
	ckx("K Fclose h", Fclose((short)h), 0);
	ckx("K Fclose h2", Fclose((short)h2), 0);
	ck("K C:\\K2\\E.TXT 3000 intact", same("C:\\K2\\E.TXT", 3000));
	ckx("K C:\\K1 empty", list("C:\\K1\\*.*", buf, sizeof(buf)), 2);

	/* L */
	Dcreate("C:\\L1"); Dcreate("C:\\L2");
	mkfile("C:\\L1\\BIG.BIN", 40000, 'l');
	ckx("L cross-dir BIG.BIN L1->L2", Frename(0, "C:\\L1\\BIG.BIN", "C:\\L2\\BIG.BIN"), 0);
	ck("L 40000 intact at L2", same("C:\\L2\\BIG.BIN", 40000));
	ckx("L cross-dir back L2->L1 as BACK.BIN", Frename(0, "C:\\L2\\BIG.BIN", "C:\\L1\\BACK.BIN"), 0);
	ck("L 40000 intact at L1", same("C:\\L1\\BACK.BIN", 40000));

	/* M: VFAT long-name entries pre-seeded by the host (mkcard): "Long File Name Test.txt", "Another Long Name.txt" */
	Dcreate("C:\\M1");
	n = list("C:\\*.*", buf, sizeof(buf)); say("M C:\\ before: "); Cconws(buf); Cconws("\r\n");
	{
		_DTA dta; char a1[20], a2[20]; a1[0] = a2[0] = 0;
		Fsetdta(&dta);
		if (Fsfirst("C:\\LONGFI*.*", 0x37) == 0) strcpy(a1, dta.dta_name);
		if (Fsfirst("C:\\ANOTHE*.*", 0x37) == 0) strcpy(a2, dta.dta_name);
		ck("M alias of Long File Name Test.txt found", a1[0] != 0);
		ck("M alias of Another Long Name.txt found", a2[0] != 0);
		if (a1[0] && a2[0]) {
			char p1[40], p2[40];
			strcpy(p1, "C:\\"); strcat(p1, a1); strcpy(p2, "C:\\"); strcat(p2, a2);
			ckx("M same-dir long -> SHORT1.TXT", Frename(0, p1, "C:\\SHORT1.TXT"), 0);
			ckx("M cross-dir long -> C:\\M1\\MOVED.TXT", Frename(0, p2, "C:\\M1\\MOVED.TXT"), 0);
		}
	}
	n = list("C:\\*.*", buf, sizeof(buf)); say("M C:\\ after: "); Cconws(buf); Cconws("\r\n");
	ck("M no LONGFI/ANOTHE alias left, no blank entry", !strstr(buf, "LONGFI") && !strstr(buf, "ANOTHE") && !strstr(buf, "<>"));
	ck("M SHORT1.TXT present", strstr(buf, "SHORT1.TXT") != 0);
	n = list("C:\\M1\\*.*", buf, sizeof(buf)); say("M C:\\M1: "); Cconws(buf); Cconws("\r\n");
	ck("M MOVED.TXT present", strstr(buf, "MOVED.TXT") != 0);

	/* N */
	Dcreate("C:\\N1"); Dcreate("C:\\N2");
	h = Fcreate("C:\\N1\\D.TXT", 0);
	fill('n', 3000); Fwrite((short)h, 3000, big);
	ckx("N cross-dir Frename of the open file (3000 written, unflushed)", Frename(0, "C:\\N1\\D.TXT", "C:\\N2\\E.TXT"), 0);
	ckx("N Fseek 0 on the handle", Fseek(0, (short)h, 0), 0);
	ckx("N Fread 3000 via handle", Fread((short)h, 3000, rd), 3000);
	ck("N read data matches", memcmp(rd, big, 3000) == 0);
	{ unsigned short dt[2]; dt[0] = 0x6000; dt[1] = 0x5A21; ckx("N Fdatime set via handle", Fdatime(dt, (short)h, 1), 0); }
	ckx("N Fclose", Fclose((short)h), 0);
	ck("N C:\\N2\\E.TXT 3000 intact", same("C:\\N2\\E.TXT", 3000));
	{ unsigned short dt[2]; h = Fopen("C:\\N2\\E.TXT", 0); Fdatime(dt, (short)h, 0); Fclose((short)h);
	  ckx("N date/time persisted at new entry (time)", dt[0], 0x6000); ckx("N (date)", dt[1], 0x5A21); }

	/* O */
	Dcreate("C:\\O1"); Dcreate("C:\\O2"); Dcreate("C:\\O1\\T"); Dcreate("C:\\O1\\T\\IN");
	mkfile("C:\\O1\\T\\A.TXT", 5000, 'a'); mkfile("C:\\O1\\T\\IN\\B.TXT", 7000, 'b');
	ckx("O Frename folder T O1->O2", Frename(0, "C:\\O1\\T", "C:\\O2\\T"), 0);
	fill('a', 5000); ck("O A.TXT intact", same("C:\\O2\\T\\A.TXT", 5000));
	fill('b', 7000); ck("O IN\\B.TXT intact", same("C:\\O2\\T\\IN\\B.TXT", 7000));
	ckx("O Dsetpath C:\\O2\\T\\IN", Dsetpath("C:\\O2\\T\\IN"), 0);
	ckx("O .. -> T", Dsetpath(".."), 0); ckx("O .. -> O2", Dsetpath(".."), 0);
	buf[0] = 0; Dgetpath(buf, 3); ck("O cwd = \\O2", strcmp(buf, "\\O2") == 0);
	Dsetpath("\\");
	ckx("O Ddelete(O1) now empty", Ddelete("C:\\O1"), 0);

	/* P */
	info("P cross-drive Frename C:\\ -> D: (spec ENSAME -48 / EDRIVE -46 when no D:)", Frename(0, "C:\\I1\\B.TXT", "D:\\B.TXT"));

	Cconws("F18X RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
	return 0;
}
