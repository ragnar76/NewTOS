/* stderrp.c -- does stderr of a libc program still reach the console?
 * Role by argv[1]: none = parent (launched from GEM #Z autorun, argv[0] empty),
 * "kid" = child spawned by mintlib spawnl (argv[0] = "STDERRP" via ARGV=).
 * Control output goes through Cconws (always console); the probe lines go
 * through stdio stderr, write(2), Fwrite(2) and stdout. The verdict lines
 * report return values; whether the marker text appears on the console is
 * read off the serial log (console output is mirrored there). */
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <process.h>
#include "common.h"
int main(int argc, char **argv)
{
	const char *role = (argc > 1 && argv[1][0] == 'k') ? "KID" : "PAR";
	long r;
	int e;
	Cconws("STDERRP["); Cconws(role); Cconws("] argc="); puthex((unsigned long)argc);
	Cconws(" argv0len="); puthex((unsigned long)strlen(argv[0])); Cconws("\r\n");
	errno = 0; e = fputs("STDERRP-MARK stdio-stderr\n", stderr);
	Cconws("STDERRP["); Cconws(role); Cconws("] fputs(stderr) rc="); puthex((unsigned long)e); Cconws("\r\n");
	e = fprintf(stderr, "STDERRP-MARK fprintf-stderr %d\n", 42);
	Cconws("STDERRP["); Cconws(role); Cconws("] fprintf(stderr) rc="); puthex((unsigned long)e); Cconws("\r\n");
	r = write(2, "STDERRP-MARK write2\n", 20);
	Cconws("STDERRP["); Cconws(role); Cconws("] write(2) rc="); puthex((unsigned long)r); Cconws("\r\n");
	r = Fwrite(2, 21, "STDERRP-MARK Fwrite2\r\n");
	Cconws("STDERRP["); Cconws(role); Cconws("] Fwrite(2) rc="); puthex((unsigned long)r); Cconws("\r\n");
	e = fputs("STDERRP-MARK stdio-stdout\n", stdout); fflush(stdout);
	Cconws("STDERRP["); Cconws(role); Cconws("] fputs(stdout) rc="); puthex((unsigned long)e); Cconws("\r\n");
	perror("STDERRP-MARK perror");
#ifndef STDERRP_NOSPAWN
	if (role[0] == 'P') {
		r = spawnl(P_WAIT, "C:\\STDERRP.NAP", "STDERRP", "kid", (char *)NULL);
		Cconws("STDERRP[PAR] spawn rc="); puthex((unsigned long)r); Cconws("\r\n");
	}
#endif
	Cconws("STDERRP["); Cconws(role); Cconws("]: DONE\r\n");
	return 0;
}
