/*
 * s2all.c -- ARCH03 stage 2: run the probes one after the other with
 * Pexec(0) and report each exit code (0 = all its checks passed). Ends with
 * "S2ALL: PASS" / "S2ALL: FAIL n".
 */
#include "common.h"

static const char *const progs[] = {
	"C:\\PFLAGS1.NCP",
	"C:\\PFLAGS2.NCP",
	"C:\\P34PAR.NAP",
	"C:\\ENVPAR.NAP",
	"C:\\P567.NAP",
	"C:\\TSRPAR.NAP",
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
