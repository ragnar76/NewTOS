/*
 * libtest.c -- exercises the REAL mintlib facilities on top of the retargeted
 * NewTOS trap layer, to MEASURE the remaining ABI-port surface:
 *
 *   1. printf("%d %ld\n", ...)  -- doprnt / stdio cookie streams
 *   2. malloc + use + free       -- mintlib/malloc.c
 *   3. a file round-trip         -- fopen("C:\\LT.TXT","wb")/fwrite/fclose
 *                                   then fopen(...,"rb")/fread/fclose, compare
 *
 * This is deliberately a HIGH-surface client: it drags in stdio, stdlib and
 * the unix syscall glue so the LP64-vs-ILP32 / endian / missing-header issues
 * surface as concrete compiler/linker diagnostics. Making it fully pass is a
 * later step; the categorized error list is the deliverable here.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void)
{
	int i = -42;
	long l = 0x12345678L;

	printf("%d %ld\n", i, l);

	char *p = malloc(64);
	if (!p) {
		printf("malloc failed\n");
		return 1;
	}
	strcpy(p, "heap string");
	printf("heap: %s\n", p);
	free(p);

	static const char *path = "C:\\LT.TXT";
	static const char payload[] = "libtest round-trip payload 0123456789";
	size_t n = strlen(payload);

	FILE *f = fopen(path, "wb");
	if (!f) {
		printf("fopen(wb) failed\n");
		return 2;
	}
	fwrite(payload, 1, n, f);
	fclose(f);

	char buf[128];
	f = fopen(path, "rb");
	if (!f) {
		printf("fopen(rb) failed\n");
		return 3;
	}
	size_t got = fread(buf, 1, sizeof(buf), f);
	fclose(f);

	if (got == n && memcmp(buf, payload, n) == 0)
		printf("roundtrip OK (%lu bytes)\n", (unsigned long)got);
	else
		printf("roundtrip MISMATCH got=%lu want=%lu\n",
		       (unsigned long)got, (unsigned long)n);

	return 0;
}
