/*
 * cwdprobe.c -- Tester scaffolding (PROC04 test run): a program started from a folder (desktop / TeraDesk) prints its
 * current drive, current directory and the first bytes of the relative file MARK.TXT (exists in every folder with
 * folder-specific content), then ends. Serial lines: "[CWD <tag>] drv=<A..> path='<dir>' mark='<text>' pd37=<n>".
 * GEM variant (appl_init/appl_exit) for .NAP; the same source built twice with different tags.
 */
#include "common.h"
#include <gem.h>
#include <string.h>

#ifndef CWD_TAG
#define CWD_TAG "?"
#endif

int main(void)
{
	char path[128], mark[40];
	long fh, n;
	unsigned long pd;
	int id = appl_init();

	memset(path, 0x55, sizeof path);
	Dgetpath(path, 0);
	mark[0] = 0;
	fh = Fopen("MARK.TXT", 0);
	if (fh >= 0) {
		n = Fread((int)fh, 30, mark);
		if (n < 0) n = 0;
		mark[n] = 0;
		Fclose((int)fh);
	} else {
		strcpy(mark, "<no MARK.TXT>");
	}
	pd = os_run_pd();
	Cconws("[CWD " CWD_TAG "] drv="); Cconout('A' + Dgetdrv());
	Cconws(" path='"); Cconws(path); Cconws("' mark='"); Cconws(mark);
	Cconws("' pd37="); puthex(((const unsigned char *)pd)[0x37]); Cconws(" ap_id="); puthex((unsigned long)id);
	Cconws("\r\n");
	appl_exit();
	return 0;
}
