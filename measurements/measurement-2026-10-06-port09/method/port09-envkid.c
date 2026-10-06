/*
 * envkid.c -- PORT09 probe, CHILD half.
 *
 * Reports the BasePage's p_env value, then reads the environment block it
 * points to, record by record, the way any TOS startup code does
 * (mintlib crtinit.c parseargs(), TeraDesk environm.c envlen()).  If p_env
 * is not a VA this process can read, the first dereference faults -- and the
 * serial log shows the fault right after the "[EKID] 2" line.
 */
#include <mint/basepage.h>
#include <mint/osbind.h>


static void puthex(unsigned long v)
{
	static const char d[] = "0123456789abcdef";
	char b[19];
	int i;

	b[0] = '0';
	b[1] = 'x';
	for (i = 0; i < 16; i++)
		b[2 + i] = d[(v >> ((15 - i) * 4)) & 0xf];
	b[18] = '\0';
	Cconws(b);
}

int main(void)
{
	const BASEPAGE *bp = _base;
	const char *p = (const char *)(unsigned long)bp->p_env;
	int n;

	Cconws("[EKID] 1 p_env=");
	puthex((unsigned long)bp->p_env);
	Cconws(" p_cmdlin[0]=");
	puthex((unsigned long)(unsigned char)bp->p_cmdlin[0]);
	Cconws("\r\n");
	Cconws("[EKID] 2 reading the environment block at p_env\r\n");
	if (p == 0) {
		Cconws("[EKID] 3 p_env is 0 -- no environment\r\n");
		return 0;
	}
	for (n = 0; *p && n < 40; n++) {
		Cconws("[EKID] 3 rec \"");
		Cconws(p);
		Cconws("\"\r\n");
		while (*p)
			p++;
		p++;
	}
	Cconws("[EKID] 4 end of environment\r\n");
	return 0;
}
