/* siggetmask -- useless alias for `sigblock (0)' for old Linux compatibility.
   Copyright (C) 1996 Free Software Foundation, Inc.
   This file is part of the GNU C Library.

   The GNU C Library is free software; you can redistribute it and/or
   modify it under the terms of the GNU Library General Public License as
   published by the Free Software Foundation; either version 2 of the
   License, or (at your option) any later version.

   The GNU C Library is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Library General Public License for more details.

   You should have received a copy of the GNU Library General Public
   License along with the GNU C Library; if not, see
   <https://www.gnu.org/licenses/>.  */

#include <signal.h>

int siggetmask (void); /* obsolete; not declared in headers */

int
siggetmask (void)
{
  return __sigblock (0);
}

/* NewTOS/AArch64 (TASK/SDK01 A): the pristine file's link_warning (siggetmask, ...)
   line is removed. mintlib's link_warning (mintlib/libc-symbols.h:71) emits
   a.out `.stabs' directives, which clang's integrated ELF assembler rejects,
   so the whole file -- a real libc function -- failed to compile and was
   missing from libc.a (docs/analysis/sdk-inventory-2026-10-20.md W7). The
   directive only asked an a.out GNU ld to print a warning when the symbol
   is used (stab type 30, N_WARNING) -- a mechanism with no counterpart in
   NewTOS's ELF link. The function body is unchanged. */
