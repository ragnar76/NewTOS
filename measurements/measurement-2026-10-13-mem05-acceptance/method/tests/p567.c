/*
 * p567.c -- ARCH03 stage 2b (TASK/PROC01): Pexec 5, 6 and 7.
 *
 * EmuTOS bdos/proc.c:210-248 -- 5/7 build a basepage (env + TPA owned by
 * the caller, init_pd_fields, p_flags = 0 / (ULONG)path) and return it;
 * 4 starts it at p_tbase (proc_go, :548) leaving ownership alone; 6 first
 * gives basepage and environment to the child (set_owner, :238-242), whose
 * Pterm then frees them. Spec docs/api/gemdos/process/4b-pexec.md (NewTOS
 * notes, 2b). The started code is a function of THIS image: the shared
 * address space makes it callable from the child (vfork's pattern).
 *
 * Built twice (post-E9): P567.NAP for protection on/off, P567S.NAP with
 * -DP567_STRICT for memprot=strict, where the mode-6 child's access to our
 * text is DENIED -- the expected strict result, checked as such.
 */
#include "common.h"

static volatile unsigned long shared, child_pd, child_parent, child_osrun;
static const char env7[] = "P567=seven\0\0";

static void __attribute__((noreturn)) child_main(BASEPAGE *bp)
{
	child_pd = (unsigned long)bp;
	child_parent = bp->p_parent;
	child_osrun = os_run_pd();
	shared = 0x5670000UL | bp->p_cmdlin[0];
	Cconws("[P567] child runs at p_tbase\r\n");
	Pterm(0x45);
	for (;;)
		;
}

static void __attribute__((noreturn)) child6_main(BASEPAGE *bp)
{
	shared = 0x6660000UL | (unsigned long)(bp->p_hitpa - bp->p_lowtpa);
	Pterm(0x46);
	for (;;)
		;
}

static long st_free(void) { return (long)Mxalloc(-1L, 0); }

int main(void)
{
	unsigned long me = (unsigned long)_base, top, b, b2, b7;
	BASEPAGE *bp;
	long rc, st0, st1, stl;

	top = *(volatile unsigned int *)0x42EUL;	/* phystop: top of ST-RAM */

	/* --- mode 5: a basepage, flags 0, our environment copied --------- */
	stl = st_free();
	rc = Pexec(5, 0L, "\003abc", 0L);
	Cconws("[P567] Pexec(5) = ");
	puthex((unsigned long)rc);
	Cconws("\r\n");
	if (rc <= 0) {
		Cconws("P567: FAIL (Pexec 5)\r\n");
		return 1;
	}
	b = (unsigned long)rc;
	bp = (BASEPAGE *)b;
	checkhex("5: p_lowtpa == PD", bp->p_lowtpa, b);
	/* E9 (2026-10-05): the TPA is the largest free block, EmuTOS alloc_tpa
	 * (bdos/proc.c:460-502, p_hitpa = p + max :374) -- the largest ST block
	 * before the call, less the environment allocated first (:281-287) and
	 * the page slack in front (both at most a page each). */
	Cconws("[P567] ST largest before ");
	puthex((unsigned long)stl);
	Cconws(", TPA ");
	puthex(bp->p_hitpa - b);
	Cconws(", ST largest after ");
	puthex((unsigned long)st_free());
	Cconws("\r\n");
	check("5: p_hitpa - PD == the largest ST block (E9)",
	      bp->p_hitpa - b <= (unsigned long)stl && bp->p_hitpa - b + 0x2000 >= (unsigned long)stl);
	check("5: nothing as large left in ST-RAM", (unsigned long)st_free() < bp->p_hitpa - b);
	checkhex("5: p_flags == 0", bp->p_reserved, 0);
	checkhex("5: p_xdta == &p_cmdlin", bp->p_dta, b + 0x80);
	checkhex("5: p_tbase == 0 (nothing loaded)", bp->p_tbase, 0);
	checkhex("5: p_parent == 0 before go", bp->p_parent, 0);
	check("5: p_cmdlin = our tail", bp->p_cmdlin[0] == 3 && bp->p_cmdlin[1] == 'a'
	      && bp->p_cmdlin[3] == 'c' && bp->p_cmdlin[4] == 0);
	check("5: p_env a copy of ours", bp->p_env != 0 && bp->p_env != _base->p_env
	      && *(const char *)(unsigned long)bp->p_env == *(const char *)(unsigned long)_base->p_env);
	check("5: no PF_TTRAMLOAD -> TPA and env in ST-RAM", b < top && bp->p_env < top);

	/* --- mode 4 on it: run child_main at p_tbase, tfork style ---------- */
	checkhex("Mshrink(PD, 0x1100) (we own it)", (unsigned long)Mshrink(bp, 0x1100), 0);
	bp->p_tbase = (unsigned long)child_main;
	bp->p_hitpa = b + 0x1100;
	rc = Pexec(4, 0L, (char *)bp, 0L);
	checkhex("4: Pexec = child's exit code", (unsigned long)rc, 0x45);
	checkhex("4: child wrote our variable (shared space)", shared, 0x5670003UL);
	checkhex("4: child's x0 == the PD", child_pd, b);
	checkhex("4: child's p_parent == our PD", child_parent, me);
	checkhex("4: child's *(os_run) == its PD", child_osrun, b);
	rc = Pexec(4, 0L, (char *)bp, 0L);
	checkhex("4: a finished PD again = EIMBA", (unsigned long)rc, (unsigned long)-40L);
	checkhex("4: Mfree(env) still ours", (unsigned long)Mfree((void *)(unsigned long)bp->p_env), 0);
	checkhex("4: Mfree(PD) still ours", (unsigned long)Mfree(bp), 0);

	/* --- mode 7: flags from path, explicit env, never read as string ----- */
	rc = Pexec(7, (char *)0x10000007UL, "", env7);
	Cconws("[P567] Pexec(7) = ");
	puthex((unsigned long)rc);
	Cconws("\r\n");
	check("7: returns a PD", rc > 0);
	if (rc > 0) {
		b7 = (unsigned long)rc;
		bp = (BASEPAGE *)b7;
		checkhex("7: p_flags == (ULONG)path", bp->p_reserved, 0x10000007UL);
		check("7: PF_TTRAMLOAD -> TPA in TT-RAM", b7 >= top);
		check("7: explicit env", has_record((const char *)(unsigned long)bp->p_env, "P567=seven"));
		checkhex("7: p_cmdlin empty", bp->p_cmdlin[0], 0);
		checkhex("7: Mfree(env)", (unsigned long)Mfree((void *)(unsigned long)bp->p_env), 0);
		checkhex("7: Mfree(PD)", (unsigned long)Mfree(bp), 0);
	}

	/* --- mode 6: memory goes to the child, its Pterm frees it ---------- */
	st0 = st_free();
	rc = Pexec(5, 0L, "", 0L);
	check("6: Pexec(5) for it", rc > 0);
	if (rc > 0) {
		unsigned long env2;
		b2 = (unsigned long)rc;
		bp = (BASEPAGE *)b2;
		env2 = bp->p_env;
		Mshrink(bp, 0x2000);
		bp->p_tbase = (unsigned long)child6_main;
		bp->p_hitpa = b2 + 0x2000;
		rc = Pexec(6, 0L, (char *)bp, 0L);
#ifdef P567_STRICT
		/* memprot=strict (post-E9 expectation, not a failure): a Pexec-6
		 * child has its own space and keeps no rights to the parent's
		 * PROT_P blocks (FreeMiNT MPF_STRICT, sys/k_exec.c:799-806; mode 6
		 * is no thread there). Its p_tbase lies in OUR text, so its first
		 * instruction faults (execute, private memory) and the crash path
		 * ends it with Pterm(-1); it never writes `shared`. */
		checkhex("6 (strict): child denied our text -> Pterm(-1)", (unsigned long)rc, 0xffffUL);
		checkhex("6 (strict): child did not run (shared untouched)", shared, 0x5670003UL);
#else
		checkhex("6: Pexec = child's exit code", (unsigned long)rc, 0x46);
		checkhex("6: child saw its TPA (p_hitpa - p_lowtpa)", shared, 0x6662000UL);
#endif
		checkhex("6: Mfree(PD) after = EIMBA (the child's Pterm freed it)",
			 (unsigned long)Mfree((void *)b2), (unsigned long)-40L);
		checkhex("6: Mfree(env) after = EIMBA", (unsigned long)Mfree((void *)env2),
			 (unsigned long)-40L);
	}
	st1 = st_free();
	checkhex("6: ST-RAM largest block back where it was", (unsigned long)st1, (unsigned long)st0);

	/* --- refusals ----------------------------------------------------- */
	checkhex("4: not a PD = EIMBA", (unsigned long)Pexec(4, 0L, (char *)me, 0L),
		 (unsigned long)-40L);
	checkhex("6: not a PD = EIMBA", (unsigned long)Pexec(6, 0L, (char *)0x12345678UL, 0L),
		 (unsigned long)-40L);
	checkhex("mode 8 = EINVFN", (unsigned long)Pexec(8, 0L, 0L, 0L), (unsigned long)-32L);

	Cconws(fails ? "P567: FAIL\r\n" : "P567: PASS\r\n");
	return fails;
}
