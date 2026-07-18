/*
 * traplink.c -- minimal proof that the retargeted mintlib trap layer (osbind.h)
 * COMPILES and LINKS into a real NewTOS `.NCP` via crt0.S + newtos.ld, using
 * only raw GEMDOS bindings (no libc). This is the "trivial Cconws/Fopen path"
 * required by the port step-1 deliverable: it drives Cconws + a file
 * round-trip (Fcreate/Fwrite/Fclose then Fopen/Fread/Fclose) entirely through
 * the register-SVC macros, with no dependency on the generated features.h /
 * ostruct.h chain.
 *
 * We skip features.h / ostruct.h through their include guards so the trap
 * macros alone are exercised; none of the bindings we call need ostruct types.
 */
#define _FEATURES_H 1
#define _MINT_OSTRUCT_H 1
#define __BEGIN_DECLS
#define __END_DECLS
#include <mint/osbind.h>

#define RDONLY 0
#define WRONLY 1

static int streq(const char *a, const char *b, long n)
{
	for (long i = 0; i < n; i++)
		if (a[i] != b[i])
			return 0;
	return 1;
}

int main(void)
{
	static const char *path = "C:\\LT.TXT";
	static const char msg[] = "NewTOS trap layer round-trip OK\r\n";
	char buf[64];
	long h, wr, rd;
	long n = (long)sizeof(msg) - 1; /* exclude NUL */

	Cconws("traplink: start\r\n");

	/* write */
	h = Fcreate(path, 0);
	if (h < 0) {
		Cconws("traplink: Fcreate failed\r\n");
		return 1;
	}
	wr = Fwrite((short)h, n, msg);
	Fclose((short)h);
	if (wr != n) {
		Cconws("traplink: short write\r\n");
		return 2;
	}

	/* read back */
	h = Fopen(path, RDONLY);
	if (h < 0) {
		Cconws("traplink: Fopen failed\r\n");
		return 3;
	}
	rd = Fread((short)h, n, buf);
	Fclose((short)h);
	if (rd != n || !streq(buf, msg, n)) {
		Cconws("traplink: mismatch\r\n");
		return 4;
	}

	Cconws("traplink: PASS -> ");
	Cconws(msg);
	return 0;
}
