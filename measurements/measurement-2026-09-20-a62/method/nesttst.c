/*
 * nesttst.c -- A62 probe: nested Pexec chain.  Each instance reads its depth
 * from the basepage command tail (p_cmdlin at 0x80, on-wire layout), calls
 * appl_init(), reports what the AES gave it, and Pexec(0)s itself with
 * depth+1 while depth < MAXDEPTH.  Then appl_exit(), Pterm(0).
 * SCRATCH ONLY -- not part of the project tree.
 */
#include <mint/osbind.h>
#include <gem.h>

#define MAXDEPTH 17

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
	const char *cmdlin = (const char *)0x80;	/* BASEPAGE.p_cmdlin */
	int depth = 1, i;
	short id, h, owner = -2, d, w, find, e;
	short msg[8];
	char tail[8];
	long rc;

	if (cmdlin[0] >= 1) {
		depth = 0;
		for (i = 1; i <= cmdlin[0]; i++)
			if (cmdlin[i] >= '0' && cmdlin[i] <= '9')
				depth = depth * 10 + (cmdlin[i] - '0');
	}

	id = appl_init();
	h = wind_create(0x0001, 0, 0, 640, 480);
	if (h > 0)
		wind_get(h, WF_OWNER, &owner, &d, &d, &d);
	for (i = 0; i < 8; i++) msg[i] = 0;
	msg[0] = 0x4711; msg[1] = id;
	w = appl_write(id, 16, msg);
	find = appl_find("NESTTST ");

	Cconws("NESTTST depth="); putnum(depth);
	Cconws(" appl_init="); putnum(id);
	Cconws(" wind_create="); putnum(h);
	Cconws(" WF_OWNER="); putnum(owner);
	Cconws(" appl_write(self)="); putnum(w);
	Cconws(" appl_find(NESTTST)="); putnum(find);
	Cconws("\r\n");

	if (depth < MAXDEPTH) {
		int dd = depth + 1;
		if (dd >= 10) { tail[0] = 2; tail[1] = (char)('0' + dd / 10); tail[2] = (char)('0' + dd % 10); tail[3] = 0; }
		else          { tail[0] = 1; tail[1] = (char)('0' + dd); tail[2] = 0; }
		rc = Pexec(0, "C:\\NESTTST.NAP", tail, 0L);
		Cconws("NESTTST depth="); putnum(depth);
		Cconws(" Pexec(child) returned "); putnum(rc);
		Cconws("\r\n");
	}
	if (h > 0)
		wind_delete(h);
	e = appl_exit();
	Cconws("NESTTST depth="); putnum(depth); Cconws(" appl_exit="); putnum(e); Cconws(" -> Pterm\r\n");
	Pterm(0);
	return 0;
}
