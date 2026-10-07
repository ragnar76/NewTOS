/*
 * freadben.c -- FS08 read-throughput bench (console program, .NCP).
 *
 * Reads one file completely with Fread chunk sizes 512 B, 4 KiB, 64 KiB and
 * "whole file in one call", timing each pass with _hz_200 (5 ms ticks), and
 * prints KB/s.  Default file C:\FS08TEST.BIN; argv[1] overrides.
 * Results go to the screen and are appended to C:\FREADBEN.LOG.
 * Measurement tool for TASK/FS08 -- not product code.
 */
#include <mint/osbind.h>
#include <mint/sysvars.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char line[256];
static long logh = -1;

static void out(const char *s)
{
	Cconws(s);
	if (logh >= 0)
		Fwrite((short)logh, strlen(s), s);
}

static unsigned long hz(void)
{
	return (unsigned long)get_sysvar(_hz_200);
}

static long pass(const char *path, long chunk, char *buf, long bufsz)
{
	long h, n, total = 0;
	unsigned long sum = 0, t0, t1, calls = 0;
	long i;

	h = Fopen(path, 0);
	if (h < 0) {
		sprintf(line, "Fopen(%s) = %ld\r\n", path, h);
		out(line);
		return h;
	}
	if (chunk > bufsz)
		chunk = bufsz;
	t0 = hz();
	for (;;) {
		n = Fread((short)h, chunk, buf);
		calls++;
		if (n <= 0)
			break;
		for (i = 0; i < n; i += 512)
			sum += (unsigned char)buf[i];
		total += n;
		if (n < chunk)
			break;
	}
	t1 = hz();
	Fclose((short)h);
	{
		unsigned long ms = (t1 - t0) * 5;
		unsigned long kbs = ms ? (unsigned long)((total / 1024) * 1000UL / ms) : 0;
		sprintf(line, "%7ld B/Fread: %5lu ms %5lu KB/s %5lu calls %ld B %08lx\r\n",
			chunk, ms, kbs, calls, total, sum);
		out(line);
	}
	return total;
}

int main(int argc, char **argv)
{
	const char *path = argc > 1 ? argv[1] : "C:\\FS08TEST.BIN";
	long sizes[4] = { 512, 4096, 65536, 0 };
	long len, bufsz;
	char *buf;
	int k;

	logh = Fopen("C:\\FREADBEN.LOG", 1);
	if (logh < 0)
		logh = Fcreate("C:\\FREADBEN.LOG", 0);
	else
		Fseek(0, (short)logh, 2);

	out("\r\n==== FREADBEN (FS08) ====\r\n");
	{
		long h = Fopen(path, 0);
		if (h < 0) {
			sprintf(line, "cannot open %s (%ld)\r\n", path, h);
			out(line);
			goto done;
		}
		len = Fseek(0, (short)h, 2);
		Fclose((short)h);
	}
	sprintf(line, "file %s, %ld bytes\r\n", path, len);
	out(line);
	bufsz = len < 65536 ? 65536 : len;
	buf = (char *)Malloc(bufsz);
	if ((long)buf <= 0) {
		out("Malloc failed\r\n");
		goto done;
	}
	sizes[3] = bufsz;
	for (k = 0; k < 4; k++)
		pass(path, sizes[k], buf, bufsz);
	Mfree(buf);
done:
	out("==== end ====\r\n");
	if (logh >= 0)
		Fclose((short)logh);
	logh = -1;
#ifndef NOWAIT
	Cconws("Press a key.\r\n");
	Cconin();
#endif
	return 0;
}
