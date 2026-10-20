/*
 * hello.c -- NewTOS SDK template: a console program (.NCP / .NTP) with
 * mintlib (stdio, stdlib) and the GEMDOS bindings of <mint/osbind.h>.
 *
 * Build:   make                (Makefile + sdk/mk/newtos.mk)  -> HELLO.NCP
 *     or:  newtos-cc -ncp -o HELLO.NCP hello.c
 * As .NTP (the desktop asks for a command line): KIND = ntp in the Makefile.
 *
 * The program ends by itself, so it may also run from C:\AUTO\.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <mint/osbind.h>

int main(int argc, char **argv)
{
	const char *path = getenv("PATH");
	char *buf;
	int i;

	printf("Hello from a NewTOS console program\n");
	printf("argc = %d\n", argc);
	for (i = 0; i < argc; i++)
		printf("argv[%d] = \"%s\"\n", i, argv[i]);
	printf("PATH = %s\n", path ? path : "(not set)");

	/* Sversion(): GEMDOS version, low byte major, high byte minor. */
	printf("GEMDOS version 0x%04x\n", (unsigned)(Sversion() & 0xffff));

	buf = malloc(64);
	if (buf == NULL) {
		printf("malloc failed\n");
		return 1;
	}
	strcpy(buf, "malloc and free work");
	printf("%s\n", buf);
	free(buf);
	return 0;
}
