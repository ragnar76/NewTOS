/*
 * newtos_scrninfo.c -- NewTOS/AArch64 services of the SCRNINFO probe
 * (core/scrninfo.h).  The counterpart of abi/m68k_scrninfo.c, same three
 * functions:
 *
 *   td_jar_read     the cookie jar, copied.  NewTOS materialises the classic
 *                   low-memory area per process at the classic addresses,
 *                   user-readable (tos-api/src/bdos/process/loader.rs
 *                   populate_sysvars / write_cookie_jar), so _p_cookies at
 *                   0x5A0 and the jar it points to are plain loads -- the
 *                   same reads the m68k backend makes, without the Supexec
 *                   that only an m68k user program needs to reach them
 *                   (sdk/libc/newtos-arch/getsysvar.c makes the same read).
 *                   Both cells are 32-bit LONGs on the wire, so they are
 *                   read as uint32_t, never as an LP64 pointer.
 *   td_eddi_call0   refuses.  On NewTOS the EdDI cookie is a non-callable
 *                   marker (docs/api/vdi/eddi-vq_scrninfo.md section 7, E2
 *                   option b: tos-api/src/cookie.rs EDDI_MARKER, non-zero
 *                   and odd), and there is no AArch64 dispatcher convention
 *                   to call one with.  core/scrninfo.c never calls this
 *                   for a zero or odd value; should a future cookie value
 *                   be even, the probe stops here with a message instead of
 *                   jumping to an address that no convention defines.
 *   td_clock_read   Tgetdate / Tgettime / Getrez through mintlib's
 *                   osbind.h, which issues them as svc #1 / svc #14 on
 *                   NewTOS.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <mint/osbind.h>

#include "scrninfo.h"

#ifndef __aarch64__
# error "newtos_scrninfo.c is the NewTOS backend; build it for aarch64"
#endif

#define P_COOKIES ((const volatile uint32_t *)0x5A0UL)

void td_jar_read(struct td_jar *jar)
{
	const volatile uint32_t *p;
	uint16_t n = 0;

	memset(jar, 0, sizeof *jar);
	jar->jar_addr = *P_COOKIES;
	if (jar->jar_addr == 0)
		return;

	p = (const volatile uint32_t *)(uintptr_t)jar->jar_addr;
	for (;;) {
		uint32_t tag   = p[0];
		uint32_t value = p[1];

		if (tag == 0) {
			jar->end_value = value;
			break;
		}
		if (n == TD_JAR_MAX) {
			jar->cut = 1;
			break;
		}
		jar->e[n].tag   = tag;
		jar->e[n].value = value;
		n++;
		p += 2;
	}
	jar->count = n;
}

uint32_t td_eddi_call0(uint32_t fn)
{
	printf("SCRNINFO: EdDI cookie value %08lX is even, but NewTOS defines "
	       "no AArch64 dispatcher convention (spec E2) -- not called\n",
	       (unsigned long)fn);
	exit(2);
}

void td_clock_read(struct td_clock *c)
{
	c->date = (uint16_t)Tgetdate();
	c->time = (uint16_t)Tgettime();
	c->rez  = (int16_t)Getrez();
}
