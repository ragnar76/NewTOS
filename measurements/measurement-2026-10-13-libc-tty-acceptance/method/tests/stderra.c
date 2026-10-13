/* stderra.c -- FS11-AUX: a NEW-crt0 libc program started WITH an ARGV
 * command line writes to stderr while handle 2 is still the default AUX.
 *
 * mintlib's _main forces handle 2 to CON only when argv[0] is empty
 * (mintlib/main.c:168-169); a program started via the ARGV scheme
 * (argv[0] = program name) keeps the inherited handle 2. EmuCON starts
 * programs with Pexec(0, path, cmdline, NULL) (references/emutos/cli/
 * cmdexec.c:52), i.e. WITHOUT ARGV, so it cannot produce this case; this
 * probe does it itself:
 *   parent (GEM #Z autorun, argv[0] empty -> _main did Fforce(2,-1)):
 *     Fforce(2,-2) puts handle 2 back to AUX (the default of a process that
 *     nobody forced), then spawnl(P_WAIT, "C:\STDERRA.NAP", "STDERRA",
 *     "kid", NULL) -- mintlib spawn passes ARGV= (mintlib/spawn.c), so the
 *     kid sees argv[0] = "STDERRA" and its _main does NOT Fforce.
 *   kid: argc = 2, argv0len = 7, handle 2 inherited = AUX; it writes through
 *     stdio stderr, fprintf, write(2), Fwrite(2) (4 KiB too) and perror.
 * With AUX = "port present, no cable" (3f-fread.md item 1a) every write
 * returns its count and is discarded; with the old absent AUX (Bconout 0)
 * the first stdio stderr write loops forever (FS11 acceptance finding 1).
 * Control output goes through Cconws (always console). */
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <process.h>
#include "common.h"

static char big[0x1000];

int main(int argc, char **argv)
{
	const char *role = (argc > 1 && argv[1][0] == 'k') ? "KID" : "PAR";
	long r;
	int e;

	Cconws("STDERRA["); Cconws(role); Cconws("] argc="); puthex((unsigned long)argc);
	Cconws(" argv0len="); puthex((unsigned long)strlen(argv[0])); Cconws("\r\n");
	if (role[0] == 'P') {
		r = Fforce(2, -2);
		Cconws("STDERRA[PAR] Fforce(2,-2) rc="); puthex((unsigned long)r); Cconws("\r\n");
	}
	errno = 0; e = fputs("STDERRA-MARK stdio-stderr\n", stderr);
	Cconws("STDERRA["); Cconws(role); Cconws("] fputs(stderr) rc="); puthex((unsigned long)e); Cconws("\r\n");
	e = fprintf(stderr, "STDERRA-MARK fprintf-stderr %d\n", 42);
	Cconws("STDERRA["); Cconws(role); Cconws("] fprintf(stderr) rc="); puthex((unsigned long)e); Cconws("\r\n");
	r = write(2, "STDERRA-MARK write2\n", 20);
	Cconws("STDERRA["); Cconws(role); Cconws("] write(2) rc="); puthex((unsigned long)r); Cconws("\r\n");
	r = Fwrite(2, 21, "STDERRA-MARK Fwrite2\r\n");
	Cconws("STDERRA["); Cconws(role); Cconws("] Fwrite(2) rc="); puthex((unsigned long)r); Cconws("\r\n");
	r = Fwrite(2, (long)sizeof big, big);
	Cconws("STDERRA["); Cconws(role); Cconws("] Fwrite(2,0x1000) rc="); puthex((unsigned long)r); Cconws("\r\n");
	perror("STDERRA-MARK perror");
	Cconws("STDERRA["); Cconws(role); Cconws("] perror returned\r\n");
	if (role[0] == 'P') {
		r = spawnl(P_WAIT, "C:\\STDERRA.NAP", "STDERRA", "kid", (char *)NULL);
		Cconws("STDERRA[PAR] spawn rc="); puthex((unsigned long)r); Cconws("\r\n");
	}
	Cconws("STDERRA["); Cconws(role); Cconws("]: DONE\r\n");
	return 0;
}
