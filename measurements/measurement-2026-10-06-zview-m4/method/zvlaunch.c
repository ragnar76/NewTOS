/*
 * zvlaunch.c -- test helper of measurement-2026-10-06-zview-m4 (TASK/ZVIEW-M4),
 * not product code. Started by the card's NEWDESK.INF #Z line, it starts
 * zView WITH a file argument through the real AES mechanism and ends:
 *
 *   C:\ZVARG.TXT holds one line, the command line for zView (e.g.
 *   "C:\IMAGES\T24.BMP"). zvlaunch reads it, builds an Atari command tail
 *   (length byte first) and calls
 *       shel_write(SHW_EXEC, 1, 0, "C:\ZVIEW\ZVIEW.NAP", tail)
 *   after Dsetpath(C:\ZVIEW), then appl_exit + Pterm. The AES shell loop
 *   then runs ZVIEW.NAP as a NORMAL_APP in that directory with that tail
 *   (sh_ldapp), and zView's
 *   main() opens argv[argc-1] (zview/main.c:164-165).
 *
 * The same mechanism chaintest.c (sdk/apps/chaintest) proves; one launcher
 * binary serves every card, only ZVARG.TXT differs.
 */
#include <mint/osbind.h>
#include <gem.h>

#define SHW_EXEC 1

int main(void)
{
	char tail[128];
	char line[126];
	long fh, n, i;

	(void)appl_init();

	for (i = 0; i < (long)sizeof(line); i++)
		line[i] = 0;
	n = 0;
	fh = Fopen("C:\\ZVARG.TXT", 0);
	if (fh >= 0) {
		n = Fread((short)fh, sizeof(line) - 1, line);
		Fclose((short)fh);
	}
	if (n < 0)
		n = 0;
	/* first line only, no CR/LF */
	for (i = 0; i < n; i++)
		if (line[i] == '\r' || line[i] == '\n')
			break;
	n = i;
	line[n] = 0;

	tail[0] = (char)(n + 1);
	tail[1] = ' ';
	for (i = 0; i < n; i++)
		tail[2 + i] = line[i];
	tail[2 + n] = 0;

	/* EmuTOS' shel_write saves the CALLER's current drive and directory as
	 * the directory the new program starts in (aes/gemshlib.c:173-175,
	 * sh_curdrvdir(sh_apdir); used by sh_chdef :543) -- the desktop sets the
	 * program's own directory before it launches one. zView finds its
	 * codecs, icons and resource relative to that directory
	 * (zview/main.c:123-125, plugins.c:56-60). */
	Dsetdrv(2);
	Dsetpath("\\ZVIEW");
	Cconws("ZVLAUNCH: shel_write(SHW_EXEC, C:\\ZVIEW\\ZVIEW.NAP, \"");
	Cconws(tail + 1);
	Cconws("\")\r\n");
	(void)shel_write(SHW_EXEC, 1, 0, "C:\\ZVIEW\\ZVIEW.NAP", tail);

	appl_exit();
	Pterm(0);
	return 0;
}
