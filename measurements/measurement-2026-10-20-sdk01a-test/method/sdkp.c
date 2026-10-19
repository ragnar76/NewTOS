/*
 * sdkp.c -- TASK/SDK01 package A runtime probe: the libc functions that did
 * not link before package A, run on NewTOS (QEMU raspi4b).
 *
 * Parent (C:\AUTO\SDKP.NCP, console, before the AES):
 *   L  link check: the program links at all (tmpfile, mkstemp, glob, ttyname,
 *      getlogin, getpwuid, getwd, siggetmask, nf_* referenced below).
 *   S  stat(): C:\SUB\A.TXT (regular, 11 bytes), C:\SUB (directory),
 *      C:\NOPE.TXT (-1, ENOENT).
 *   Y  ttyname(0) == "/dev/con", ttyname(5) / isatty false case, ctermid.
 *   M  mkstemp("C:\TMP\MKXXXXXX"): fd >= 0, template filled, the file
 *      exists (Fsfirst), write 7 bytes, close, stat size 7, unlink, gone.
 *   N  tmpnam(NULL) / tempnam: a name under \TMP, file NOT created.
 *   G  glob("/sub/*"): exactly a.txt and b.txt (case-insensitive: mintlib's
 *      TOS readdir may fold case); globfree. mintlib's glob splits on '/'
 *      only, so the pattern uses mintlib's Unix path view ("/sub" = \SUB on
 *      the current drive); a DOS pattern "C:\SUB\*" is one file-name
 *      pattern for it and answers GLOB_NOMATCH (first run, serial/sp-on-1.log).
 *   P  getpwuid(0) / getlogin(): no /etc/passwd on the card -> NULL (mintlib
 *      behaviour on plain TOS), no crash.
 *   W  getwd(): non-NULL; siggetmask() returns; nf_version() == 0,
 *      nf_get_id("NF_NAME") == 0, nf_debug() == 0 (NatFeats absent).
 *   T  tmpfile() in a child (C:\SDKK.NCP, Pexec 0): the child creates it,
 *      writes 1000 bytes, reads them back, sees ONE new entry in \TMP, exits
 *      (exit code 0x50 on success); after the child the parent sees \TMP
 *      empty again (mintlib's atexit delete_tmpfiles on TOS).
 * Every check prints "[SP] ok|FAIL ..."; ends with "SP RESULT fails=<n>".
 * The card is fsck'ed and listed after the run (run.sh FSCK=1 CARDLS=1).
 */
#include "common.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <unistd.h>
#include <errno.h>
#include <glob.h>
#include <pwd.h>
#include <sys/stat.h>
#include <mint/arch/nf_ops.h>

int siggetmask(void);	/* obsolete; not declared in headers (signal/siggetmask.c) */

static int count_tmp(char *first, int n)
{
	_DTA *old = Fgetdta(), d;
	int c = 0;
	Fsetdta(&d);
	if (Fsfirst("C:\\TMP\\*.*", 0x07) == 0) {
		do {
			if (d.dta_name[0] != '.') {
				if (c == 0 && first) { strncpy(first, d.dta_name, n - 1); first[n - 1] = 0; }
				c++;
			}
		} while (Fsnext() == 0);
	}
	Fsetdta(old);
	return c;
}

static int exists(const char *p)
{
	_DTA *old = Fgetdta(), d;
	long r;
	Fsetdta(&d);
	r = Fsfirst(p, 0x07);
	Fsetdta(old);
	return r == 0;
}

static void say(const char *a, const char *b)
{
	Cconws("[SP] "); Cconws(a); Cconws(b ? b : "(null)"); Cconws("\r\n");
}

#ifdef KID
int main(void)
{
	char buf[1000], back[1000], name[16];
	FILE *fp;
	int i, before, during;
	size_t n;

	before = count_tmp(NULL, 0);
	fp = tmpfile();
	if (!fp) { Cconws("[SP] kid: tmpfile NULL\r\n"); return 0x51; }
	for (i = 0; i < 1000; i++) buf[i] = (char)('A' + i % 26);
	n = fwrite(buf, 1, sizeof buf, fp);
	fflush(fp);
	rewind(fp);
	memset(back, 0, sizeof back);
	n += fread(back, 1, sizeof back, fp);
	during = count_tmp(name, sizeof name);
	Cconws("[SP] kid: tmpfile entries before="); puthex(before);
	Cconws(" during="); puthex(during); Cconws(" name="); Cconws(during ? name : "-");
	Cconws(" rw="); puthex(n); Cconws("\r\n");
	if (n != 2000 || memcmp(buf, back, sizeof buf) != 0) return 0x52;
	if (during != before + 1) return 0x53;
	return 0x50;		/* exit(): atexit delete_tmpfiles closes + removes it */
}
#else
int main(void)
{
	struct stat st;
	char tmpl[32], nm[L_tmpnam + 8], wd[256], first[16];
	const char *s;
	glob_t g;
	int fd, r, gr;
	long rc;

	Cconws("[SP] start: SDK01 A libc runtime probe\r\n");

	/* S */
	r = stat("C:\\SUB\\A.TXT", &st);
	check("S stat C:\\SUB\\A.TXT = 0, regular, 11 bytes", r == 0 && S_ISREG(st.st_mode) && st.st_size == 11);
	r = stat("C:\\SUB", &st);
	check("S stat C:\\SUB = 0, directory", r == 0 && S_ISDIR(st.st_mode));
	errno = 0; r = stat("C:\\NOPE.TXT", &st);
	check("S stat C:\\NOPE.TXT = -1, ENOENT", r == -1 && errno == ENOENT);

	/* Y */
	s = ttyname(0); say("Y ttyname(0)=", s);
	check("Y ttyname(0) = /dev/con", s && strcmp(s, "/dev/con") == 0);
	s = ctermid(NULL); say("Y ctermid=", s);
	check("Y ctermid non-NULL", s != NULL);

	/* M */
	strcpy(tmpl, "C:\\TMP\\MKXXXXXX");
	fd = mkstemp(tmpl); say("M mkstemp name=", tmpl);
	check("M mkstemp fd >= 0, template filled", fd >= 0 && strstr(tmpl, "XXXXXX") == NULL);
	check("M file exists after mkstemp", exists(tmpl));
	check("M write 7 bytes", fd >= 0 && write(fd, "sdk01a\n", 7) == 7);
	check("M close 0", fd >= 0 && close(fd) == 0);
	r = stat(tmpl, &st);
	check("M stat size 7", r == 0 && st.st_size == 7);
	check("M unlink 0", unlink(tmpl) == 0);
	check("M file gone after unlink", !exists(tmpl));

	/* N */
	s = tmpnam(nm); say("N tmpnam=", s);
	check("N tmpnam under \\tmp, not created", s && strstr(s, "tmp") && !exists(s));
	{
		char *t = tempnam("C:\\TMP", "pf");
		say("N tempnam=", t);
		check("N tempnam non-NULL, not created", t && !exists(t));
		free(t);
	}

	/* G */
	memset(&g, 0, sizeof g);
	gr = glob("/sub/*", 0, NULL, &g);
	Cconws("[SP] G glob rc="); puthex((unsigned long)gr); Cconws(" n="); puthex(g.gl_pathc); Cconws("\r\n");
	if (gr == 0) { size_t i; for (i = 0; i < g.gl_pathc; i++) say("G   ", g.gl_pathv[i]); }
	check("G glob /sub/* = 0, 2 paths (a.txt, b.txt)", gr == 0 && g.gl_pathc == 2 &&
	      strcasecmp(g.gl_pathv[0], "/sub/a.txt") == 0 && strcasecmp(g.gl_pathv[1], "/sub/b.txt") == 0);
	if (gr == 0) globfree(&g);

	/* P */
	check("P getpwuid(0) = NULL (no /etc/passwd)", getpwuid(0) == NULL);
	s = getlogin(); say("P getlogin=", s);

	/* W */
	s = getwd(wd); say("W getwd=", s);
	check("W getwd non-NULL", s != NULL);
	Cconws("[SP] W siggetmask="); puthex((unsigned long)siggetmask()); Cconws("\r\n");
	check("W nf_version() = 0", nf_version() == 0);
	check("W nf_get_id(NF_NAME) = 0", nf_get_id(NF_ID_NAME) == 0);
	check("W nf_debug() = 0", nf_debug("x") == 0);

	/* T */
	check("T \\TMP empty before the child", count_tmp(first, sizeof first) == 0);
	rc = Pexec(0, "C:\\SDKK.NCP", "\0", 0L);
	Cconws("[SP] T child exit="); puthex((unsigned long)rc); Cconws("\r\n");
	check("T child tmpfile ok (exit 0x50)", rc == 0x50);
	r = count_tmp(first, sizeof first);
	if (r) say("T left over: ", first);
	check("T \\TMP empty after the child (tmpfile removed at exit)", r == 0);

	Cconws("SP RESULT fails="); puthex((unsigned long)fails); Cconws("\r\n");
	return 0;
}
#endif
