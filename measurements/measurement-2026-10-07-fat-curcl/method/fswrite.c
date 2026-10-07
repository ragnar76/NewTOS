/*
 * fswrite.c -- FSWRITE.NCP, the headless GEMDOS write-path check of TASK/FS08
 * part 2 (measurement tool, not product code).
 *
 * Runs from C:\AUTO\ before the AES. Through plain GEMDOS calls it writes a
 * fixed set of files on D:\FSW\ (and a small one on C:\FSWC\) with mixed
 * Fwrite sizes, reads every file back with a different Fread size, patches
 * some with Fseek + Fwrite across sector and cluster boundaries, appends,
 * copies, moves (Frename), deletes files and a folder -- and checks every
 * byte against the model below after every step. The final file set stays
 * on the card for the host side (fswrite-check.py, the same model in Python)
 * and fsck_msdos.
 *
 * Content model: byte i of a run written with seed s is
 *     ((i * 7 + i / 251) & 0xFF) ^ s
 * (the FSBENCH/fatclus pattern), i counted from the start of that run.
 *
 * Output: progress lines on the console, the result in C:\FSWRITE.LOG and
 * the last line "FSWRITE: PASS" / "FSWRITE: FAIL (<n> errors)".
 */
#include <mint/osbind.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#define LOG_PATH "C:\\FSWRITE.LOG"

static unsigned char pat(unsigned char seed, unsigned long i)
{
	return (unsigned char)(((i * 7u) + (i / 251u)) & 0xFF) ^ seed;
}

/* ------------------------------------------------------------------ log */

static char logbuf[16384];
static int loglen;
static int errors;

static void out(const char *fmt, ...)
{
	char line[256];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(line, sizeof line, fmt, ap);
	va_end(ap);
	Cconws(line);
	Cconws("\r\n");
	int n = (int)strlen(line);
	if (loglen + n + 2 < (int)sizeof logbuf) {
		memcpy(logbuf + loglen, line, n);
		loglen += n;
		logbuf[loglen++] = '\r';
		logbuf[loglen++] = '\n';
	}
}

static void fail(const char *fmt, ...)
{
	char line[200];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(line, sizeof line, fmt, ap);
	va_end(ap);
	errors++;
	out("ERROR %s", line);
}

/* ------------------------------------------------------------------ model */

/* A patch: `len` bytes of seed `seed` written at `off` (run-relative index). */
struct patch { long off, len; unsigned char seed; };

struct file {
	const char *name;      /* path */
	long size;             /* initial size */
	long wchunk, rchunk;   /* Fwrite / Fread sizes (0 = whole file) */
	unsigned char seed;
	int npatch;
	struct patch p[3];
};

static struct file files[] = {
	{ "D:\\FSW\\A00.DAT",       0,   512,   512, 0x01, 0, {{0}} },
	{ "D:\\FSW\\A01.DAT",       1,     1,   333, 0x02, 0, {{0}} },
	{ "D:\\FSW\\A02.DAT",     511,   100,   512, 0x03, 0, {{0}} },
	{ "D:\\FSW\\A03.DAT",     512,   512,   100, 0x04, 0, {{0}} },
	{ "D:\\FSW\\A04.DAT",     513,  1000,   512, 0x05, 0, {{0}} },
	{ "D:\\FSW\\A05.DAT",    4095,   512,  4096, 0x06, 0, {{0}} },
	{ "D:\\FSW\\A06.DAT",    8192,  4096,   333, 0x07, 0, {{0}} },
	{ "D:\\FSW\\A07.DAT",    8193,  8192, 65536, 0x08, 0, {{0}} },
	{ "D:\\FSW\\A08.DAT",   65539,  1000,   512, 0x09, 0, {{0}} },
	{ "D:\\FSW\\A09.DAT",   65539, 65536,  4096, 0x0A, 0, {{0}} },
	{ "D:\\FSW\\A10.DAT",  200003,   512,     0, 0x0B, 0, {{0}} },
	/* patched: across a cluster boundary (8 KiB clusters on D:), and a
	   20000-byte run starting one byte before a record boundary */
	{ "D:\\FSW\\A11.DAT", 1048653,  4096,  1000, 0x0C, 2,
	  {{ 8192 - 100, 300, 0x40 }, { 3 * 8192 + 511, 20000, 0x41 }} },
	{ "D:\\FSW\\A12.DAT", 1048653,     0,   512, 0x0D, 0, {{0}} },
	/* patched: appended 1000 bytes at the end, then 10 bytes at 0 */
	{ "D:\\FSW\\A13.DAT", 3000000, 65536,     0, 0x0E, 2,
	  {{ 3000000, 1000, 0x42 }, { 0, 10, 0x43 }} },
	{ "C:\\FSWC\\B00.DAT",  100003,   777,  4096, 0x21, 0, {{0}} },
};
#define NFILES ((int)(sizeof files / sizeof files[0]))

/* Size of file `f` after its first `np` patches. */
static long size_after(const struct file *f, int np)
{
	long s = f->size;
	for (int k = 0; k < np; k++)
		if (f->p[k].off + f->p[k].len > s)
			s = f->p[k].off + f->p[k].len;
	return s;
}

/* Byte `i` of file `f` after its first `np` patches. */
static unsigned char model(const struct file *f, int np, long i)
{
	unsigned char b = pat(f->seed, (unsigned long)i);
	for (int k = 0; k < np; k++)
		if (i >= f->p[k].off && i < f->p[k].off + f->p[k].len)
			b = pat(f->p[k].seed, (unsigned long)(i - f->p[k].off));
	return b;
}

/* ------------------------------------------------------------------ I/O */

#define BUFSZ (3L * 1048576L + 65536L)
static unsigned char *rbuf;   /* Malloc'd, BUFSZ: read target and write source */

/* Write `len` bytes of seed `seed` with `chunk`-byte Fwrites (0 = one). */
static int write_run(int h, long len, long chunk, unsigned char seed, const char *what)
{
	long done = 0;
	if (chunk <= 0)
		chunk = len;
	while (done < len) {
		long m = len - done < chunk ? len - done : chunk;
		for (long k = 0; k < m; k++)
			rbuf[k] = pat(seed, (unsigned long)(done + k));
		long r = Fwrite(h, m, rbuf);
		if (r != m) {
			fail("%s: Fwrite(%ld) at %ld -> %ld", what, m, done, r);
			return -1;
		}
		done += m;
	}
	return 0;
}

/* Read the whole file `path` with `chunk`-byte Freads into rbuf; length. */
static long read_file(const char *path, long chunk)
{
	int h = (int)Fopen(path, 0);
	if (h < 0) {
		fail("Fopen(%s) -> %d", path, h);
		return -1;
	}
	long total = 0;
	if (chunk <= 0)
		chunk = BUFSZ;
	for (;;) {
		long want = chunk;
		if (total + want > BUFSZ)
			want = BUFSZ - total;
		if (want <= 0)
			break;
		long r = Fread(h, want, rbuf + total);
		if (r < 0) {
			fail("%s: Fread -> %ld", path, r);
			Fclose(h);
			return -1;
		}
		if (r == 0)
			break;
		total += r;
	}
	long rc = Fclose(h);
	if (rc != 0)
		fail("%s: Fclose -> %ld", path, rc);
	return total;
}

static void verify(const struct file *f, int np, const char *path, long chunk, const char *step)
{
	long want = size_after(f, np);
	long got = read_file(path, chunk);
	if (got < 0)
		return;
	if (got != want) {
		fail("%s %s: length %ld, want %ld", step, path, got, want);
		return;
	}
	for (long i = 0; i < got; i++) {
		if (rbuf[i] != model(f, np, i)) {
			fail("%s %s: byte %ld = %02x, want %02x", step, path, i, rbuf[i], model(f, np, i));
			return;
		}
	}
	out("ok  %-6s %-22s %8ld B, Fread %ld", step, path, got, chunk);
}

static void make_file(const struct file *f)
{
	int h = (int)Fcreate(f->name, 0);
	if (h < 0) {
		fail("Fcreate(%s) -> %d", f->name, h);
		return;
	}
	write_run(h, f->size, f->wchunk, f->seed, f->name);
	long rc = Fclose(h);
	if (rc != 0)
		fail("%s: Fclose -> %ld", f->name, rc);
}

static void apply_patches(const struct file *f)
{
	for (int k = 0; k < f->npatch; k++) {
		int h = (int)Fopen(f->name, 2);
		if (h < 0) {
			fail("Fopen(%s, 2) -> %d", f->name, h);
			return;
		}
		long pos = Fseek(f->p[k].off, h, 0);
		if (pos != f->p[k].off)
			fail("%s: Fseek(%ld) -> %ld", f->name, f->p[k].off, pos);
		write_run(h, f->p[k].len, 4096, f->p[k].seed, f->name);
		long rc = Fclose(h);
		if (rc != 0)
			fail("%s: Fclose -> %ld", f->name, rc);
		verify(f, k + 1, f->name, 4096, "patch");
	}
}

/* Copy `src` to `dst` with `chunk`-byte Fread/Fwrite pairs (a desktop copy). */
static void copy_file(const char *src, const char *dst, long chunk)
{
	static unsigned char cbuf[32768];
	int hs = (int)Fopen(src, 0);
	int hd = (int)Fcreate(dst, 0);
	if (hs < 0 || hd < 0) {
		fail("copy %s -> %s: Fopen %d Fcreate %d", src, dst, hs, hd);
		return;
	}
	for (;;) {
		long r = Fread(hs, chunk, cbuf);
		if (r < 0) {
			fail("copy: Fread -> %ld", r);
			break;
		}
		if (r == 0)
			break;
		long w = Fwrite(hd, r, cbuf);
		if (w != r) {
			fail("copy: Fwrite(%ld) -> %ld", r, w);
			break;
		}
	}
	Fclose(hs);
	long rc = Fclose(hd);
	if (rc != 0)
		fail("copy: Fclose -> %ld", rc);
}

int main(void)
{
	out("==== FSWRITE (TASK/FS08 part 2) -- GEMDOS write path ====");
	rbuf = (unsigned char *)Malloc(BUFSZ);
	if (!rbuf) {
		out("FSWRITE: FAIL (no memory)");
		return 1;
	}
	if (Dcreate("D:\\FSW") != 0 || Dcreate("D:\\FSW\\SUB") != 0 || Dcreate("C:\\FSWC") != 0)
		fail("Dcreate D:\\FSW, D:\\FSW\\SUB, C:\\FSWC (already there?)");

	for (int i = 0; i < NFILES; i++) {
		make_file(&files[i]);
		verify(&files[i], 0, files[i].name, files[i].rchunk, "write");
	}
	for (int i = 0; i < NFILES; i++)
		if (files[i].npatch)
			apply_patches(&files[i]);

	/* copy (32 KiB and 4 KiB chunks), move, delete */
	copy_file("D:\\FSW\\A13.DAT", "D:\\FSW\\SUB\\COPY13.DAT", 32768);
	verify(&files[13], files[13].npatch, "D:\\FSW\\SUB\\COPY13.DAT", 65536, "copy");
	copy_file("C:\\FSWC\\B00.DAT", "C:\\FSWC\\B01.DAT", 4096);
	verify(&files[14], 0, "C:\\FSWC\\B01.DAT", 512, "copy");

	long r = Frename(0, "D:\\FSW\\A05.DAT", "D:\\FSW\\SUB\\MOVED05.DAT");
	if (r != 0)
		fail("Frename A05 -> SUB\\MOVED05 -> %ld", r);
	verify(&files[5], 0, "D:\\FSW\\SUB\\MOVED05.DAT", 100, "move");
	int hm = (int)Fopen("D:\\FSW\\A05.DAT", 0);
	if (hm >= 0) {
		fail("A05.DAT still there after the move");
		Fclose(hm);
	}

	if ((r = Fdelete("D:\\FSW\\A02.DAT")) != 0)
		fail("Fdelete A02 -> %ld", r);
	if ((r = Fdelete("D:\\FSW\\A07.DAT")) != 0)
		fail("Fdelete A07 -> %ld", r);

	/* a folder with files, emptied and removed */
	if ((r = Dcreate("D:\\FSW\\TMP")) != 0)
		fail("Dcreate TMP -> %ld", r);
	static const long tsz[3] = { 5000, 100, 0 };
	char tp[40];
	for (int k = 0; k < 3; k++) {
		sprintf(tp, "D:\\FSW\\TMP\\T%d.DAT", k);
		int h = (int)Fcreate(tp, 0);
		if (h < 0) {
			fail("Fcreate %s -> %d", tp, h);
			continue;
		}
		write_run(h, tsz[k], 333, 0x30, tp);
		Fclose(h);
	}
	for (int k = 0; k < 3; k++) {
		sprintf(tp, "D:\\FSW\\TMP\\T%d.DAT", k);
		if ((r = Fdelete(tp)) != 0)
			fail("Fdelete %s -> %ld", tp, r);
	}
	if ((r = Ddelete("D:\\FSW\\TMP")) != 0)
		fail("Ddelete TMP -> %ld", r);

	/* the final set, once more */
	for (int i = 0; i < NFILES; i++) {
		if (i == 2 || i == 5 || i == 7)
			continue;
		verify(&files[i], files[i].npatch, files[i].name, 65536, "final");
	}

	if (errors)
		out("FSWRITE: FAIL (%d errors)", errors);
	else
		out("FSWRITE: PASS");

	int h = (int)Fcreate(LOG_PATH, 0);
	if (h >= 0) {
		Fwrite(h, loglen, logbuf);
		Fclose(h);
	}
	return errors ? 1 : 0;
}
