/*
 * s2all.c -- ARCH03 stage 2: run the probes one after the other with
 * Pexec(0) and report each exit code (0 = all its checks passed). Ends with
 * "S2ALL: PASS" / "S2ALL: FAIL n".
 *
 * Built twice (post-E9): S2ALL.NAP for protection on and off, S2ALLS.NAP
 * (-DS2ALL_STRICT) for memprot=strict, which runs P567S/TSRPARS -- the
 * builds that expect strict's denials instead of reporting them as FAILs.
 */
#include "common.h"

static const char *const progs[] = {
	"C:\\PFLAGS1.NCP",
	"C:\\PFLAGS2.NCP",
	"C:\\P34PAR.NAP",
	"C:\\ENVPAR.NAP",
#ifdef S2ALL_STRICT
	/* memprot=strict: the strict-expectation builds (post-E9) -- the
	 * Pexec-6 child of P567 and TSRPAR's TSRRES are DENIED the parent's
	 * PROT_P memory, which these variants check as the expected result. */
	"C:\\P567S.NAP",
	"C:\\TSRPARS.NAP",
#else
	"C:\\P567.NAP",
	"C:\\TSRPAR.NAP",
#endif
	"C:\\OWNPAR.NAP",
	"C:\\VFORKT.NAP",
	0
};

int main(void)
{
	int i, bad = 0;

	for (i = 0; progs[i]; i++) {
		long rc;
		Cconws("[S2ALL] run ");
		Cconws(progs[i]);
		Cconws("\r\n");
		rc = Pexec(0, progs[i], "\0", 0L);
		Cconws("[S2ALL] ");
		Cconws(progs[i]);
		Cconws(" -> ");
		puthex((unsigned long)rc);
		Cconws(rc == 0 ? "  ok\r\n" : "  FAIL\r\n");
		if (rc != 0)
			bad++;
	}
	Cconws("[S2ALL] Malloc(-1) at the end: ");
	puthex((unsigned long)Malloc(-1L));
	Cconws("\r\n");
	if (bad) {
		Cconws("S2ALL: FAIL ");
		puthex(bad);
		Cconws("\r\n");
	} else
		Cconws("S2ALL: PASS\r\n");
	return bad;
}
