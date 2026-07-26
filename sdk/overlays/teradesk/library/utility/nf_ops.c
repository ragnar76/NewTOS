/*
 * nf_ops.c -- NewTOS/AArch64 overlay for TeraDesk's library/utility/nf_ops.c
 * (the m68k-asm retarget exception; the vendored tree stays pristine).
 *
 * The upstream file implements NatFeats detection: it executes the invalid
 * m68k opcodes 0x7300 (NATFEAT_ID) / 0x7301 (NATFEAT_CALL), which only an
 * emulator host (ARAnyM, Hatari) intercepts, with an illegal-instruction
 * vector trampoline as the "not emulated" fallback. All three upstream
 * variants (__AHCC__/__PUREC__/__GNUC__) are m68k inline asm / opcode
 * injection and cannot be retargeted: NewTOS runs bare-metal on AArch64 --
 * there IS no emulator host, so the correct, complete detection result on
 * this platform is "NatFeats not present". That is exactly the documented
 * not-available path of the <mint/arch/nf_ops.h> API: nf_init() == NULL,
 * nf_get_id() == 0, nf_debugvprintf() == -1/ENOSYS. TeraDesk's only real
 * call site, nf_shutdown(0) in main.c (via nf_shutd.c), then correctly
 * degrades to "return 0, nothing shut down".
 *
 * This file defines the same public symbols as the upstream file
 * (nf_init, nf_get_id, nf_debugvprintf, nf_debugprintf) so link behavior
 * against mintlib's nf_debug.o archive member is unchanged.
 */
#include <stdarg.h>
#include <stdio.h>
#include <mint/arch/nf_ops.h>
#include <errno.h>

struct nf_ops *nf_init(void)
{
	/* No emulator host on bare-metal NewTOS: NatFeats are not available. */
	return NULL;
}


long nf_get_id(const char *feature_name)
{
	(void) feature_name;
	return 0;					/* documented "not available" result */
}

/*** ---------------------------------------------------------------------- ***/

int nf_debugvprintf(const char *format, va_list args)
{
	(void) format;
	(void) args;
#ifdef ENOSYS
	errno = ENOSYS;
#else
	errno = EIO;
#endif
	return -1;					/* documented "NatFeats not available" result */
}

/*** ---------------------------------------------------------------------- ***/

int nf_debugprintf(const char *format, ...)
{
	int ret;
	va_list args;

	va_start(args, format);
	ret = nf_debugvprintf(format, args);
	va_end(args);
	return ret;
}
