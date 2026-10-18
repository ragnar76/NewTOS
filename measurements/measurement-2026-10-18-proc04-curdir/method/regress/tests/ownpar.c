/*
 * ownpar.c -- ARCH03 stage 2d: several owners, only an owner frees (E15).
 * FreeMiNT sys/dosmem.c:330-455 (Mfree/Mshrink only the caller's regions,
 * EFAULT = -40), sys/memory.c:512-660 (links), sys/k_exec.c:518-520 (the
 * started child attaches to basepage and environment), plan E6 (a Pexec 4
 * child shares its caller's memory space). Spec docs/api/gemdos/memory/
 * 49-mfree.md "several owners".
 */
#include "common.h"

static volatile long thread_rc;
static volatile unsigned long victim;

static void __attribute__((noreturn)) thread_main(BASEPAGE *bp)
{
	(void)bp;
	/* A Pexec-4 child runs in our memory space: it may free our block. */
	thread_rc = Mfree((void *)victim);
	Pterm(0x47);
	for (;;)
		;
}

int main(void)
{
	char tail[16];
	unsigned long blk, env;
	long rc;
	BASEPAGE *bp;

	/* 1. A child process may not free or shrink our block. */
	blk = (unsigned long)Mxalloc(4096, 1);
	tail_hex(tail, blk);
	rc = Pexec(0, "C:\\OWNKID.NAP", tail, 0L);
	checkhex("Pexec 0 child: its two refusals passed (exit 0x40)", (unsigned long)rc, 0x40);
	checkhex("our block is still ours: Mshrink", (unsigned long)Mshrink((void *)blk, 64), 0);
	checkhex("our block is still ours: Mfree", (unsigned long)Mfree((void *)blk), 0);

	/* 2. Pexec 3 + 4: the child co-owns basepage and environment. */
	tail[0] = 8;
	{ int i; for (i = 1; i <= 8; i++) tail[i] = '0'; tail[9] = 0; }
	rc = Pexec(3, "C:\\OWNKID.NAP", tail, 0L);
	check("Pexec 3", rc > 0);
	if (rc > 0) {
		bp = (BASEPAGE *)rc;
		env = bp->p_env;
		rc = Pexec(4, 0L, (char *)bp, 0L);
		checkhex("Pexec 4 child: its co-owner checks passed (exit 0x40)", (unsigned long)rc, 0x40);
		checkhex("the child freed the env (both links): EIMBA for us", (unsigned long)Mfree((void *)env),
			 (unsigned long)-40L);
		checkhex("the basepage is ours to free", (unsigned long)Mfree(bp), 0);
	}

	/* 3. A Pexec-4 thread frees a block of ours (shared memory space). */
	victim = (unsigned long)Mxalloc(256, 0);
	rc = Pexec(5, 0L, "", 0L);
	check("Pexec 5", rc > 0);
	if (rc > 0) {
		bp = (BASEPAGE *)rc;
		Mshrink(bp, 0x1100);
		bp->p_tbase = (unsigned long)thread_main;
		bp->p_hitpa = (unsigned long)bp + 0x1100;
		rc = Pexec(4, 0L, (char *)bp, 0L);
		checkhex("thread exit code", (unsigned long)rc, 0x47);
		checkhex("thread's Mfree(our block) = 0 (our memory space)", (unsigned long)thread_rc, 0);
		checkhex("and the block is gone", (unsigned long)Mfree((void *)victim), (unsigned long)-40L);
		Mfree((void *)(unsigned long)bp->p_env);
		Mfree(bp);
	}

	/* 4. Not an owner of a PD -> Pexec 4 refused (FreeMiNT EFAULT). */
	rc = Pexec(5, 0L, "", 0L);
	if (rc > 0) {
		unsigned long pd = (unsigned long)rc;
		tail_hex(tail, pd);
		/* OWNKID with our PD in the tail: it cannot Mfree it (checked
		   there); and we still can. */
		checkhex("child cannot touch our Pexec-5 PD", (unsigned long)Pexec(0, "C:\\OWNKID.NAP", tail, 0L), 0x40);
		Mfree((void *)(unsigned long)((BASEPAGE *)pd)->p_env);
		checkhex("Mfree(our unstarted PD)", (unsigned long)Mfree((void *)pd), 0);
		checkhex("a freed PD cannot be started", (unsigned long)Pexec(4, 0L, (char *)pd, 0L),
			 (unsigned long)-40L);
	}
	Cconws(fails ? "OWN: FAIL\r\n" : "OWN: PASS\r\n");
	return fails;
}
