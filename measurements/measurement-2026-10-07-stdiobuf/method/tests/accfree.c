/*
 * accfree.c -- post-E9 point 3: free_accs gives the accessories' memory
 * back when the AES session ends (EmuTOS aes/geminit.c:296-303,741 and the
 * end of the run_accs_and_desktop process, bdos/proc.c:133-139).
 *
 * Runs as the #Z autorun program on a card with C:\ACC\SYSACC.NAC.
 * 1st session: records the TOTAL free ST and TT memory (free_total():
 *   take the largest block with Mxalloc(-1)/Mxalloc(len) until none is
 *   left, add the lengths, give them all back) in C:\ACCFREE.DAT and
 *   requests an AES restart (shel_write(SHW_RESCHNG),
 *   gemshlib.c:187-191), then exits: sh_main ends the session, free_accs
 *   runs, the next session loads the accessory again and runs #Z again.
 * 2nd session: the same two totals must be back -- with the accessory of
 *   session 1 still holding its image, environment and Malloc'ed stack,
 *   that much memory would be missing. (The total, not the largest block:
 *   the second accessory need not land where the first did -- the AES
 *   screen buffer of session 1 sits right behind the first one -- so the
 *   largest block shows that layout shift, the total does not.) Deletes the
 *   file. "ACCFREE: PASS" / "ACCFREE: FAIL".
 */
#include "common.h"
#include <gem.h>

#define DAT "C:\\ACCFREE.DAT"

static unsigned long free_total(short mode, unsigned long *blocks)
{
	static void *got[64];
	unsigned long sum = 0;
	int n = 0, i;
	while (n < 64) {
		long l = (long)Mxalloc(-1L, mode);
		if (l <= 0)
			break;
		got[n] = (void *)Mxalloc(l, mode);
		if (got[n] == 0 || (long)got[n] < 0)
			break;
		sum += (unsigned long)l;
		n++;
	}
	for (i = 0; i < n; i++)
		Mfree(got[i]);
	*blocks = (unsigned long)n;
	return sum;
}

int main(void)
{
	unsigned long now[2], then[2], nb[2];
	long fh;

	appl_init();
	now[0] = free_total(0, &nb[0]);
	now[1] = free_total(1, &nb[1]);
	Cconws("[ACCFREE] free ST ");
	puthex(now[0]);
	Cconws(" in ");
	puthex(nb[0]);
	Cconws(" blocks, TT ");
	puthex(now[1]);
	Cconws(" in ");
	puthex(nb[1]);
	Cconws(" blocks; largest TT ");
	puthex((unsigned long)Mxalloc(-1L, 1));
	Cconws("\r\n");
	fh = Fopen(DAT, 0);
	if (fh < 0) {
		fh = Fcreate(DAT, 0);
		if (fh < 0) {
			Cconws("ACCFREE: FAIL (cannot create " DAT ")\r\n");
			appl_exit();
			return 1;
		}
		Fwrite((short)fh, sizeof now, now);
		Fclose((short)fh);
		Cconws("[ACCFREE] session 1: values saved, requesting an AES restart\r\n");
		shel_write(5, 0, 0, 0L, 0L);
		appl_exit();
		return 0;
	}
	Fread((short)fh, sizeof then, then);
	Fclose((short)fh);
	Fdelete(DAT);
	Cconws("[ACCFREE] session 1 had ST ");
	puthex(then[0]);
	Cconws(" TT ");
	puthex(then[1]);
	Cconws("\r\n");
	checkhex("free ST-RAM as in session 1", now[0], then[0]);
	checkhex("free TT-RAM as in session 1", now[1], then[1]);
	appl_exit();
	Cconws(fails ? "ACCFREE: FAIL\r\n" : "ACCFREE: PASS\r\n");
	return fails;
}
