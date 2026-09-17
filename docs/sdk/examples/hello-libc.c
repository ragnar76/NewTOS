/* hello-libc.c -- a mintlib-linked NewTOS console program (.NCP).
 * Build: see docs/sdk/03-first-programs.md, step 2. */
#include <stdio.h>
#include <stdlib.h>
#include <mint/osbind.h>

int main(int argc, char **argv)
{
	char *p = malloc(32);

	printf("Hello from mintlib on NewTOS (argc=%d, argv[0]=\"%s\")\n",
	       argc, argv[0] ? argv[0] : "(null)");
	printf("Sversion() = 0x%04x, Tgettime() = 0x%04x\n",
	       (unsigned)Sversion(), (unsigned)Tgettime());
	free(p);
	return 0;
}
