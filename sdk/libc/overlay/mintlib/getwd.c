/*  getwd.c -- MiNTLib.
    Copyright (C) 2000 Guido Flohr <guido@freemint.de>

    This file is part of the MiNTLib project, and may only be used
    modified and distributed under the terms of the MiNTLib project
    license, COPYMINT.  By continuing to use, modify, or distribute
    this file you indicate that you have read the license and
    understand and accept it fully.
*/

#include <limits.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include "lib.h"

char*
getwd (char *buf)
{
	char mybuf[PATH_MAX];
  
	if (buf == NULL) {
  		__set_errno (EINVAL);
  		return NULL;
  	}
  	
  	if (__getcwd (mybuf, PATH_MAX) == NULL) {
  	  __strerror_r (errno, buf, PATH_MAX);
  	  return NULL;
  	}
  	
  	return strcpy (buf, mybuf);  /* Ouch.  */
}

/* NewTOS/AArch64 (TASK/SDK01 A): the pristine file's link_warning (getwd, ...)
   line is removed. mintlib's link_warning (mintlib/libc-symbols.h:71) emits
   a.out `.stabs' directives, which clang's integrated ELF assembler rejects,
   so the whole file -- a real libc function -- failed to compile and was
   missing from libc.a (docs/analysis/sdk-inventory-2026-10-20.md W7). The
   directive only asked an a.out GNU ld to print a warning when the symbol
   is used (stab type 30, N_WARNING) -- a mechanism with no counterpart in
   NewTOS's ELF link. The function body is unchanged. */
