/*
 * getcookie.c -- NewTOS/AArch64 reimplementation of mintlib's Getcookie().
 *
 * The pristine mintlib ships Getcookie() as hand-written m68k assembly
 * (mintlib/getcookie.S): it either scans the _p_cookies low-memory sysvar
 * (0x5a0) directly, or falls back to Ssystem(S_GETCOOKIE) via `trap #1`.
 * Neither the raw low-memory scan nor the m68k asm ports to AArch64, so the
 * .S file is never compiled into libc.a (build-libc.sh assembles no mintlib
 * .S). This shim provides the symbol the way NewTOS exposes cookies.
 *
 * NewTOS has the cookie jar behind the GEMDOS Ssystem trap only (0x154,
 * S_GETCOOKIE = 8; tos-api/src/gemdos/ssystem.rs). We reimplement the
 * observable Getcookie contract on top of the already-retargeted Ssystem macro
 * (mint/mintbind.h -> svc). This is real work, not a stub.
 *
 * Contract (mint/cookie.h): return C_FOUND (0) and *val = value on a hit,
 * C_NOTFOUND (-1) and *val = 0 on a miss; p_value may be NULL.
 *
 * On NewTOS Ssystem(S_GETCOOKIE, tag, &v) always honours arg2: on a hit it
 * writes v = value and returns E_OK (0 == C_FOUND); on a miss it returns a
 * negative error (ERROR/-1 == C_NOTFOUND). The m68k "value returned in the
 * result register, arg2 ignored" MiNT 1.14.7 quirk is unobservable on NewTOS
 * and is intentionally dropped (reimplement, don't port).
 */

#include <mint/mintbind.h>
#include <mint/cookie.h>

#ifndef S_GETCOOKIE
#define S_GETCOOKIE 8
#endif

int Getcookie(long cookie, long *p_value)
{
	long v = 0;

	if (Ssystem(S_GETCOOKIE, cookie, (long)&v) != C_FOUND)
	{
		if (p_value)
			*p_value = 0;
		return C_NOTFOUND;
	}

	if (p_value)
		*p_value = v;

	return C_FOUND;
}
