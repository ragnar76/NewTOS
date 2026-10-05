/*
 * p34par.c -- ARCH03 stage 1: Pexec 3 + 4, the shell idiom.
 *
 *   bp = Pexec(PE_LOAD, "C:\P34KID.NAP", tail, env)   -> the child's PD
 *   read bp->p_lowtpa/p_hitpa/p_tbase/p_tlen/p_env/p_cmdlin  (in OUR address
 *        space: since stage 1 a PD pointer means the same in every process)
 *   Mshrink(0, bp, needed); bp->p_hitpa = bp + needed     (shell idiom)
 *   rc = Pexec(PE_GO, 0, bp, 0)                        -> the child runs
 *   bp->p_parent == our _base;  Mfree(bp->p_env); Mfree(bp)   (we own them)
 *
 * EmuTOS: xexec PE_LOAD returns the PD (bdos/proc.c:360), owner = run for
 * TPA and environment (:302-307), proc_go sets p_parent = run (:541).
 */
#include "common.h"

static const char env[] = "P34=yes\0OTHER=1\0\0";

int main(void)
{
	char tail[128];
	BASEPAGE *bp;
	long rc, needed;
	unsigned long me = (unsigned long)_base;
	int i;
	const char *hex = "0123456789abcdef";

	/* The command tail carries our PD pointer in hex, so the child can
	   compare its p_parent with it. */
	tail[0] = 8;
	for (i = 0; i < 8; i++)
		tail[1 + i] = hex[(me >> ((7 - i) * 4)) & 0xf];
	tail[9] = 0;

	Cconws("[P34] parent: _base=");
	puthex(me);
	Cconws(" os_run PD=");
	puthex(os_run_pd());
	Cconws("\r\n");
	checkhex("parent: *(os_run) == _base", os_run_pd(), me);

	rc = Pexec(3, "C:\\P34KID.NAP", tail, env);
	Cconws("[P34] Pexec(3) returned ");
	puthex((unsigned long)rc);
	Cconws("\r\n");
	if (rc <= 0) {
		Cconws("P34: FAIL (Pexec 3)\r\n");
		return 1;
	}
	bp = (BASEPAGE *)rc;
	checkhex("child p_lowtpa == PD", bp->p_lowtpa, (unsigned long)bp);
	checkhex("child p_tbase == PD + 0x1000", bp->p_tbase, (unsigned long)bp + 0x1000);
	check("child p_tlen > 0", bp->p_tlen > 0);
	check("child p_hitpa > p_lowtpa", bp->p_hitpa > bp->p_lowtpa);
	checkhex("child p_parent before PE_GO == 0", bp->p_parent, 0);
	check("child p_cmdlin = our tail", bp->p_cmdlin[0] == 8 && bp->p_cmdlin[1] == tail[1]);
	check("child p_env readable, P34=yes", bp->p_env != 0
	      && has_record((const char *)(unsigned long)bp->p_env, "P34=yes"));

	/* The shell idiom: keep basepage + image + 256 KiB of heap/stack. */
	needed = (0x1000 + bp->p_tlen + bp->p_dlen + bp->p_blen + 0x40000 + 15) & ~15L;
	rc = Mshrink(bp, needed);
	checkhex("Mshrink(child PD, needed)", (unsigned long)rc, 0);
	bp->p_hitpa = (unsigned long)bp + needed;

	rc = Pexec(4, 0L, (char *)bp, 0L);
	checkhex("Pexec(4) = child's exit code", (unsigned long)rc, 0x34);
	checkhex("child p_parent after PE_GO == our _base", bp->p_parent, me);

	rc = Pexec(4, 0L, (char *)bp, 0L);
	checkhex("Pexec(4) of a finished PD = EIMBA", (unsigned long)rc, (unsigned long)-40L);

	rc = Mfree((void *)(unsigned long)bp->p_env);
	checkhex("Mfree(child env) (we own it)", (unsigned long)rc, 0);
	rc = Mfree(bp);
	checkhex("Mfree(child PD) (we own it)", (unsigned long)rc, 0);

	Cconws(fails ? "P34: FAIL\r\n" : "P34: PASS\r\n");
	return fails;
}
