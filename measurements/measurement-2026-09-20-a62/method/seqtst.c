/*
 * seqtst.c -- A62 probe: nine DIFFERENT programs one after another.  The one
 * binary is copied as SEQ1.NAP .. SEQ9.NAP; each finds its own number in the
 * launch path (shel_read), reports appl_init / appl_find(self) /
 * appl_find(previous), chains to SEQ<n+1>.NAP via shel_write(SHW_EXEC), then
 * appl_exit(), Pterm(0).  SEQ9 chains to nothing; the desktop follows.
 * SCRATCH ONLY -- not part of the project tree.
 */
#include <mint/osbind.h>
#include <gem.h>

#define SHW_EXEC 1

static void putnum(long v)
{
	char b[20];
	int i = 19, neg = 0;
	b[i] = 0;
	if (v < 0) { neg = 1; v = -v; }
	do { b[--i] = (char)('0' + (v % 10)); v /= 10; } while (v);
	if (neg) b[--i] = '-';
	Cconws(b + i);
}

int main(void)
{
	char cmd[128], tail[128];
	char name[9], prev[9], next[32];
	short i, n = 0, id, fself, fprev = -99, e;

	for (i = 0; i < 128; i++) { cmd[i] = 0; tail[i] = 0; }
	id = appl_init();
	(void)shel_read(cmd, tail);
	/* find "SEQ<d>" in cmd */
	for (i = 0; cmd[i]; i++)
		if (cmd[i] == 'S' && cmd[i+1] == 'E' && cmd[i+2] == 'Q' && cmd[i+3] >= '1' && cmd[i+3] <= '9') { n = cmd[i+3] - '0'; break; }

	for (i = 0; i < 8; i++) { name[i] = ' '; prev[i] = ' '; }
	name[8] = 0; prev[8] = 0;
	name[0] = 'S'; name[1] = 'E'; name[2] = 'Q'; name[3] = (char)('0' + n);
	fself = appl_find(name);
	if (n > 1) {
		prev[0] = 'S'; prev[1] = 'E'; prev[2] = 'Q'; prev[3] = (char)('0' + n - 1);
		fprev = appl_find(prev);
	}
	Cconws("SEQTST n="); putnum(n);
	Cconws(" cmd=["); Cconws(cmd); Cconws("]");
	Cconws(" appl_init="); putnum(id);
	Cconws(" appl_find(self)="); putnum(fself);
	Cconws(" appl_find(prev)="); putnum(fprev);
	Cconws("\r\n");

	if (n >= 1 && n < 9) {
		const char *p = "C:\\SEQ";
		for (i = 0; p[i]; i++) next[i] = p[i];
		next[i++] = (char)('0' + n + 1);
		next[i++] = '.'; next[i++] = 'N'; next[i++] = 'A'; next[i++] = 'P'; next[i] = 0;
		tail[0] = 0; tail[1] = 0;
		Cconws("SEQTST n="); putnum(n); Cconws(" shel_write(SHW_EXEC, "); Cconws(next); Cconws(")\r\n");
		(void)shel_write(SHW_EXEC, 0, 1, next, tail);
	}
	e = appl_exit();
	Cconws("SEQTST n="); putnum(n); Cconws(" appl_exit="); putnum(e); Cconws(" -> Pterm\r\n");
	Pterm(0);
	return 0;
}
