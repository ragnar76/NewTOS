/*
 * m68k_scrninfo.c -- TOS-only services of the SCRNINFO probe (core/scrninfo.h).
 *
 * Three things, no logic beyond reading and calling:
 *
 *   td_jar_read     the cookie jar, copied in supervisor mode.  _p_cookies
 *                   at 0x5A0 is a system variable, readable only in
 *                   supervisor mode; Supexec (XBIOS 38) runs the copy there.
 *   td_eddi_call0   the EdDI dispatcher, function 0 (version).
 *   td_clock_read   Tgetdate / Tgettime / Getrez, for the report header.
 */
#include <stdint.h>
#include <string.h>

#include <mint/osbind.h>

#include "scrninfo.h"

#if !defined(__mc68000__) && !defined(__M68K__)
# error "m68k_scrninfo.c is the TOS backend; build it with m68k-atari-mint-gcc"
#endif

#define P_COOKIES ((volatile uint32_t *)0x5A0L)

static struct td_jar *jar_dst;

/* Runs in supervisor mode.  Reads only; the only writes are into *jar_dst,
 * which is this program's own memory. */
static long jar_copy(void)
{
	const volatile uint32_t *p;
	uint16_t n = 0;

	jar_dst->jar_addr = *P_COOKIES;
	if (jar_dst->jar_addr == 0)
		return 0;

	p = (const volatile uint32_t *)jar_dst->jar_addr;
	for (;;) {
		uint32_t tag   = p[0];
		uint32_t value = p[1];

		if (tag == 0) {
			jar_dst->end_value = value;
			break;
		}
		if (n == TD_JAR_MAX) {
			jar_dst->cut = 1;
			break;
		}
		jar_dst->e[n].tag   = tag;
		jar_dst->e[n].value = value;
		n++;
		p += 2;
	}
	jar_dst->count = n;
	return 0;
}

void td_jar_read(struct td_jar *jar)
{
	memset(jar, 0, sizeof *jar);
	jar_dst = jar;
	Supexec(jar_copy);
	jar_dst = 0;
}

/*
 * The dispatcher call.  Convention, docs/api/vdi/eddi-vq_scrninfo.md
 * section 2 (TH-CK; NV250 section 3, "Neue_Fkt.txt" lines 346-355): the
 * cookie value is the address of a function dispatcher, called with the
 * function number in d0 under the Pure C convention -- d0-d2/a0-a1 carry
 * parameters and are not preserved across the call, d3-d7/a2-a6 are.
 * Function 0 takes nothing else and returns the version in d0.
 *
 * Called in user mode with jsr, as any application calls it.  moveq clears
 * all of d0, so the opcode reads as 0 whether the dispatcher looks at d0.w
 * (Pure C int) or d0.l.
 */
uint32_t td_eddi_call0(uint32_t fn)
{
	uint32_t ret;

	__asm__ volatile (
		"move.l %1,%%a0\n\t"
		"moveq  #0,%%d0\n\t"      /* function number 0 = version */
		"jsr    (%%a0)\n\t"
		"move.l %%d0,%0"
		: "=d"(ret)
		: "g"(fn)
		: "d0", "d1", "d2", "a0", "a1", "memory", "cc"
	);
	return ret;
}

void td_clock_read(struct td_clock *c)
{
	c->date = (uint16_t)Tgetdate();
	c->time = (uint16_t)Tgettime();
	c->rez  = (int16_t)Getrez();
}
