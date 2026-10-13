/*
 * mdpar.c -- MEM05 QEMU tests 11/12 (design §7): the per-process limit with
 * direct GEMDOS Malloc (one block per call, no mintlib hunks), two
 * processes. Run with the boot argument mdlimit=64 (L).
 *
 *  1. The parent takes Malloc(16) until 0: n1 = L - blocks it holds
 *     (its TPA and environment). Malloc(-1) still > 0 at the limit (E5).
 *  2. It frees two and loads MDKID with Pexec(3): EmuTOS/NewTOS give the
 *     child's TPA and environment to the CALLER (proc.c:302-307), so the
 *     parent is at its limit again: Malloc(16) == 0.
 *  3. Pexec(4) runs MDKID while the parent sits at its limit; the child
 *     takes Malloc(16) until 0 under its own cookie: L blocks (it holds no
 *     tag), and exits with that count.
 *  4. Mfree of the child's env + PD: the parent may take exactly two more.
 *  5. Everything freed; Pexec(0) MDKID: the loader hands TPA + environment
 *     to the child (set_owner), so it gets L - 2.
 * Verdict line: "MDPAR: PASS" / "MDPAR: FAIL".
 */
#include "common.h"

#define MAXB 70000
static void *blk[MAXB];

static void putdec(unsigned long v)
{
	char b[24];
	int i = 23;
	b[i] = 0;
	do { b[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	Cconws(&b[i]);
}

static long fill(void)
{
	long n = 0;
	while (n < MAXB) {
		long a = Malloc(16L);
		if (a == 0 || a < 0)
			break;
		blk[n++] = (void *)a;
	}
	return n;
}

static void line(const char *what, long v)
{
	Cconws("[MDPAR] ");
	Cconws(what);
	putdec((unsigned long)v);
	Cconws("\r\n");
}

int main(void)
{
	long n1, i, rc, lim, needed;
	BASEPAGE *bp;

	n1 = fill();
	line("parent Malloc(16) successes until 0: ", n1);
	check("parent: Malloc(-1) > 0 at its limit", Malloc(-1L) > 0);
	lim = n1 + 2; /* TPA + environment */
	line("=> limit (successes + TPA + env): ", lim);

	Mfree(blk[--n1]);
	Mfree(blk[--n1]);
	rc = Pexec(3, "C:\\MDKID.NAP", "\0", 0L);
	check("Pexec(3) MDKID with two blocks of room", rc > 0);
	if (rc <= 0) {
		Cconws("MDPAR: FAIL\r\n");
		return 1;
	}
	bp = (BASEPAGE *)rc;
	checkhex("parent at its limit again: Malloc(16)", (unsigned long)Malloc(16L), 0);
	/* The shell idiom (p34par.c): basepage + image + BSS + 256 KiB. */
	needed = (0x1000 + bp->p_tlen + bp->p_dlen + bp->p_blen + 0x40000 + 15) & ~15L;
	rc = Mshrink(bp, needed);
	checkhex("Mshrink(child TPA, image + BSS + 256 KiB)", (unsigned long)rc, 0);
	bp->p_hitpa = (unsigned long)bp + needed;

	rc = Pexec(4, 0L, (char *)bp, 0L);
	line("child (Pexec 4, parent at its limit) successes: ", rc);
	checkhex("child gets the full limit, unaffected by the parent", (unsigned long)rc, (unsigned long)lim);
	checkhex("parent still at its limit after the child", (unsigned long)Malloc(16L), 0);

	checkhex("Mfree(child env)", (unsigned long)Mfree((void *)(unsigned long)bp->p_env), 0);
	checkhex("Mfree(child PD)", (unsigned long)Mfree(bp), 0);
	blk[n1] = (void *)Malloc(16L);
	check("room for one ...", blk[n1] != 0);
	n1++;
	blk[n1] = (void *)Malloc(16L);
	check("... and a second block", blk[n1] != 0);
	n1++;
	checkhex("... and not a third", (unsigned long)Malloc(16L), 0);

	for (i = n1 - 1; i >= 0; i--)
		Mfree(blk[i]);
	rc = Pexec(0, "C:\\MDKID.NAP", "\0", 0L);
	line("child (Pexec 0, holds TPA + env) successes: ", rc);
	checkhex("Pexec 0 child gets limit - 2", (unsigned long)rc, (unsigned long)(lim - 2));
	n1 = fill();
	checkhex("parent again: limit - TPA - env", (unsigned long)n1, (unsigned long)(lim - 2));
	for (i = n1 - 1; i >= 0; i--)
		Mfree(blk[i]);

	Cconws(fails ? "MDPAR: FAIL\r\n" : "MDPAR: PASS\r\n");
	return fails;
}
