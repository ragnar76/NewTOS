/*
 * envkidj.c -- A13 replacement for envkid.c (MEM03 child) after PORT09
 * (fa99b4d8): the libc start-up code now consumes the environment like
 * mintlib's parseargs -- it joins "PATH=\0C:\" in place and cuts the block at
 * ARGV=, handing the arguments over as argv[]. The old child scanned p_env
 * after crt0 had run and so no longer saw the "ARGV=" record. This child
 * checks the same facts through the libc view: PATH=C:\ in environ and the
 * two arguments of the parent's explicit environment in argv[1], argv[2].
 * Answers 0x42 when everything arrived (as envkid.c).
 */
#include "common.h"
#include <string.h>
#include <stdlib.h>

int main(int argc, char **argv, char **envp)
{
	const char *path = getenv("PATH");

	Cconws("[MEM03J] child: p_env=");
	puthex((unsigned long)_base->p_env);
	Cconws(" argc=");
	puthex((unsigned long)argc);
	Cconws("\r\n");
	check("PATH=C:\\ arrived (getenv)", path != 0 && strcmp(path, "C:\\") == 0);
	check("argc == 3", argc == 3);
	check("argv[1] == alpha-one", argc > 1 && strcmp(argv[1], "alpha-one") == 0);
	check("an argument with blanks arrived (argv[2])",
	      argc > 2 && strcmp(argv[2], "mike thirteen has blanks") == 0);
	(void)envp;
	return fails ? 1 : 0x42;
}
