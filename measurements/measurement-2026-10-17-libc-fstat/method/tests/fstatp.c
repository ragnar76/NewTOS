/* fstatp.c -- LIBC-FSTAT QEMU probe (docs/analysis/libc-fstat-2026-10-17.md
 * section 5.2): fstat / isatty / stdio set-up of a libc child whose standard
 * handles the PARENT redirected before the spawn (the child's __open_stat[]
 * cache is empty at start-up, mintlib/main.c:161-162; the redirection is
 * inherited through Pexec, tos-api/src/bdos/process/loader.rs init_pd_files).
 *
 * One binary, role by argv[1]:
 *   (none) PARENT (GEM #Z autorun):
 *     P0  own fstat(1) after Fforce(1, file) in the SAME process -- the
 *         documented upstream limit (cache filled by main.c): info line only.
 *     P1  console control: spawn kid "c" with nothing redirected.
 *     P2  Fcreate C:\OUT.TXT, Fforce(1, fh), spawnl kid "o", Fforce(1, -1),
 *         Fclose, re-read: expect 61 0D 0A 62 0D 0A (text mode, UNIXMODE unset).
 *     P3  the same with UNIXMODE=b in the kid's environment (spawnle):
 *         expect 61 0A 62 0A.
 *     P4  C:\IN.TXT = "x\r\ny\r\n", Fopen, Fforce(0, fh), spawnl kid "i",
 *         Fforce(0, -1): the kid's fgets must give "x\n", "y\n", then EOF.
 *   "c" / "o" / "i" KID: reports via Cconws (on NewTOS always the console,
 *     tos-api/src/gemdos/console.rs cconws -> DEV_CON) fstat(0..2) S_IFMT and
 *     st_blksize, isatty(0..2), and the stdio state of stdout ("c"/"o", before
 *     its first output; then printf("a\n") + putchar('b') + putchar('\n'))
 *     or stdin ("i", after the fgets).
 * Verdict lines "FSTATP[..] ok/FAIL"; last line "FSTATP: PASS" or
 * "FSTATP: FAIL n". The kid checks its own values against the LIBC-FSTAT
 * expectation and exits with its failure count; the parent requires rc 0.
 * The red control is the same source linked against the previous libc.a. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <process.h>
#include <sys/stat.h>
#include "common.h"

#define SELF "C:\\FSTATP.NAP"

static void kv(const char *k, unsigned long v)
{
	Cconws(" "); Cconws(k); Cconws("="); puthex(v);
}

/* fd `redir' (or -1) is redirected to a file of `rsize' bytes, the others
 * are at the console. */
static void kid_fds(const char *role, int redir, long rsize)
{
	int fd;
	for (fd = 0; fd <= 2; fd++) {
		struct stat st;
		int r = fstat(fd, &st), t = isatty(fd);
		int ok = (fd == redir)
			? (r == 0 && S_ISREG(st.st_mode) && st.st_size == rsize && st.st_blksize == 32768 && t == 0)
			: (r == 0 && S_ISCHR(st.st_mode) && st.st_blksize == 1024 && t == 1);
		Cconws("FSTATP["); Cconws(role); Cconws("] fd");
		puthex((unsigned long)fd);
		kv("rc", (unsigned long)r);
		Cconws(S_ISCHR(st.st_mode) ? " CHR" : S_ISREG(st.st_mode) ? " REG" : " OTHER");
		kv("size", (unsigned long)st.st_size);
		kv("blksize", (unsigned long)st.st_blksize);
		kv("isatty", (unsigned long)t);
		Cconws(ok ? " ok" : " FAIL");
		Cconws("\r\n");
		if (!ok)
			fails++;
	}
}

static void stream(const char *role, const char *name, FILE *f,
		   unsigned long bufsize, int linebuf, int binary)
{
	int ok = f->__bufsize == bufsize && (f->__linebuf != 0) == linebuf
		 && f->__mode.__binary == (unsigned)binary;
	Cconws("FSTATP["); Cconws(role); Cconws("] "); Cconws(name);
	kv("bufsize", (unsigned long)f->__bufsize);
	kv("linebuf", (unsigned long)f->__linebuf);
	kv("binary", (unsigned long)f->__mode.__binary);
	Cconws(ok ? " ok" : " FAIL");
	Cconws("\r\n");
	if (!ok)
		fails++;
}

/* stdout's stdio state, info only. */
static void trace(const char *role, const char *when)
{
	Cconws("FSTATP["); Cconws(role); Cconws("] stdout "); Cconws(when);
	kv("seen", (unsigned long)stdout->__seen);
	kv("buffer", stdout->__buffer ? 1UL : 0UL);
	kv("bufsize", (unsigned long)stdout->__bufsize);
	kv("userbuf", (unsigned long)stdout->__userbuf);
	Cconws("\r\n");
}

static void hexline(const char *s)
{
	while (*s) { Cconws(" "); puthex((unsigned char)*s++); }
}

static int kid(char role)
{
	char r[2] = { role, 0 };
	const char *um = getenv("UNIXMODE");
	int bin = um && strchr(um, 'b');
	if (um) { Cconws("FSTATP["); Cconws(r); Cconws("] UNIXMODE="); Cconws(um); Cconws("\r\n"); }
	kid_fds(r, role == 'o' ? 1 : role == 'i' ? 0 : -1, role == 'i' ? 6L : 0L);
	if (role == 'i') {
		char a[32], b[32], c[32];
		char *pa = fgets(a, sizeof a, stdin);
		char *pb = fgets(b, sizeof b, stdin);
		char *pc = fgets(c, sizeof c, stdin);
		stream(r, "stdin", stdin, 32768UL, 0, 0);
		Cconws("FSTATP[i] line1"); if (pa) hexline(a); else Cconws(" NULL"); Cconws("\r\n");
		check("i line1 = \"x\\n\"", pa && strcmp(a, "x\n") == 0);
		Cconws("FSTATP[i] line2"); if (pb) hexline(b); else Cconws(" NULL"); Cconws("\r\n");
		check("i line2 = \"y\\n\"", pb && strcmp(b, "y\n") == 0);
		check("i line3 = EOF", pc == NULL);
	} else {
		/* Checked: the state main.c (setvbuf + init_stream_mode) and
		 * __stdio_init_stream (stdio_init.c:72-77, __bufsize = st_blksize)
		 * leave before the first output. Info only: the first printf on a
		 * stream without a buffer runs vfprintf's buffered_vfprintf helper
		 * (stdio/vfprintf.c:1333) and leaves __bufsize 0, so the buffer the
		 * following putchar allocates (internals.c init_stream) is BUFSIZ --
		 * on the console as on a file (upstream stdio path). */
		if (role == 'o')
			stream(r, "stdout", stdout, 32768UL, 0, bin);
		else
			stream(r, "stdout", stdout, 1024UL, 1, 0);
		printf("a\n");
		trace(r, "after printf");
		putchar('b');
		trace(r, "after putchar");
		putchar('\n');
	}
	Cconws("FSTATP["); Cconws(r); Cconws("]: DONE fails="); puthex((unsigned long)fails); Cconws("\r\n");
	return fails;
}

/* Read the whole of `path` (<= 64 bytes) into buf, return its length. */
static long slurp(const char *path, unsigned char *buf)
{
	long h = Fopen(path, 0), n;
	if (h < 0)
		return h;
	n = Fread((short)h, 64, buf);
	Fclose((short)h);
	return n;
}

static void file_check(const char *what, const char *path, const unsigned char *want, long wn)
{
	unsigned char got[64];
	long n = slurp(path, got), i;
	int ok = (n == wn);
	Cconws("FSTATP[PAR] "); Cconws(what); Cconws(" bytes:");
	for (i = 0; i < n && i < 64; i++) { Cconws(" "); puthex(got[i]); if (got[i] != want[i]) ok = 0; }
	Cconws("\r\n");
	check(what, ok);
}

static long redirect_out(char *const *envp)
{
	long fh, r, f;
	Fdelete("C:\\OUT.TXT");
	fh = Fcreate("C:\\OUT.TXT", 0);
	Cconws("FSTATP[PAR] Fcreate OUT.TXT"); kv("fh", (unsigned long)fh);
	f = Fforce(1, (short)fh);
	kv("Fforce(1)", (unsigned long)f);
	Cconws("\r\n");
	if (envp)
		r = spawnle(P_WAIT, SELF, "FSTATP", "o", (char *)NULL, envp);
	else
		r = spawnl(P_WAIT, SELF, "FSTATP", "o", (char *)NULL);
	f = Fforce(1, -1);
	Cconws("FSTATP[PAR] spawn o"); kv("rc", (unsigned long)r); kv("Fforce(1,-1)", (unsigned long)f);
	f = Fclose((short)fh);
	kv("Fclose", (unsigned long)f);
	Cconws("\r\n");
	return r;
}

int main(int argc, char **argv)
{
	static const unsigned char want_text[] = { 0x61, 0x0d, 0x0a, 0x62, 0x0d, 0x0a };
	static const unsigned char want_bin[] = { 0x61, 0x0a, 0x62, 0x0a };
	static char *const env_b[] = { "UNIXMODE=b", (char *)NULL };
	long fh, f, r;
	struct stat st;

	if (argc > 1)
		return kid(argv[1][0]);

	Cconws("FSTATP[PAR] start argc="); puthex((unsigned long)argc); Cconws("\r\n");

	/* P0: same-process Fforce after main.c's isatty(1) -- documented limit */
	Fdelete("C:\\P0.TXT");
	fh = Fcreate("C:\\P0.TXT", 0);
	f = Fforce(1, (short)fh);
	r = fstat(1, &st);
	Fforce(1, -1);
	Fclose((short)fh);
	Cconws("FSTATP[PAR] P0 same-process Fforce(1) fstat(1)"); kv("rc", (unsigned long)r);
	Cconws(S_ISCHR(st.st_mode) ? " CHR" : S_ISREG(st.st_mode) ? " REG" : " OTHER");
	kv("blksize", (unsigned long)st.st_blksize); kv("Fforce", (unsigned long)f);
	Cconws(" (info: cached isatty, upstream limit)\r\n");

	/* P1: console control */
	r = spawnl(P_WAIT, SELF, "FSTATP", "c", (char *)NULL);
	Cconws("FSTATP[PAR] spawn c"); kv("rc", (unsigned long)r); Cconws("\r\n");
	check("P1 kid c spawned (rc 0)", r == 0);

	/* P2: stdout to a file, text default */
	r = redirect_out((char *const *)0);
	check("P2 kid o spawned (rc 0)", r == 0);
	file_check("P2 OUT.TXT = 61 0D 0A 62 0D 0A (text)", "C:\\OUT.TXT", want_text, (long)sizeof want_text);

	/* P3: the same with UNIXMODE=b */
	r = redirect_out(env_b);
	check("P3 kid o (UNIXMODE=b) spawned (rc 0)", r == 0);
	file_check("P3 OUT.TXT = 61 0A 62 0A (UNIXMODE=b)", "C:\\OUT.TXT", want_bin, (long)sizeof want_bin);

	/* P4: stdin from a file */
	Fdelete("C:\\IN.TXT");
	fh = Fcreate("C:\\IN.TXT", 0);
	Fwrite((short)fh, 6, "x\r\ny\r\n");
	Fclose((short)fh);
	fh = Fopen("C:\\IN.TXT", 0);
	f = Fforce(0, (short)fh);
	Cconws("FSTATP[PAR] Fopen IN.TXT"); kv("fh", (unsigned long)fh); kv("Fforce(0)", (unsigned long)f); Cconws("\r\n");
	r = spawnl(P_WAIT, SELF, "FSTATP", "i", (char *)NULL);
	f = Fforce(0, -1);
	Cconws("FSTATP[PAR] spawn i"); kv("rc", (unsigned long)r); kv("Fforce(0,-1)", (unsigned long)f);
	kv("Fclose", (unsigned long)Fclose((short)fh)); Cconws("\r\n");
	check("P4 kid i spawned (rc 0)", r == 0);

	if (fails) {
		Cconws("FSTATP: FAIL "); puthex((unsigned long)fails); Cconws("\r\n");
	} else
		Cconws("FSTATP: PASS\r\n");
	Cconws("FSTATP[PAR]: DONE\r\n");
	return 0;
}
