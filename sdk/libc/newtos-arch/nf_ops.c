/*
 * nf_ops.c -- NewTOS/AArch64 replacement for mintlib/nf_ops.c (NatFeats
 * detection), excluded from the mintlib build (build-libc.sh EXCLUDE).
 *
 * NatFeats ("native features", <mint/arch/nf_ops.h>) is the ARAnyM/Hatari
 * interface through which an m68k program talks to its EMULATOR HOST: the
 * program executes the m68k opcodes 0x7300 (NATFEAT_ID) and 0x7301
 * (NATFEAT_CALL), which a real 680x0 traps as illegal instructions and which
 * the emulator intercepts. mintlib's nf_init() detects the host in three
 * ways, all of them m68k: the deprecated __NF cookie (whose value points at
 * an m68k NatFeatCookie with m68k function pointers), Supexec of an
 * illegal-instruction probe through the 0x10 vector, and a SIGILL probe
 * under MiNT. All three variants of the file (__AHCC__/__PUREC__/__GNUC__)
 * are m68k inline asm or opcode injection; the file never compiled for
 * AArch64 (it was recorded in expected-failures.txt), so nf_init was
 * undefined and none of the libc nf_* functions (nf_debug, nf_debugprintf,
 * nf_debugvprintf, nf_exec, nf_execv, nf_get_name, nf_get_fullname,
 * nf_shutdown, nf_version) linked (TASK/SDK01 A,
 * docs/analysis/sdk-inventory-2026-10-20.md W7).
 *
 * NewTOS runs natively on AArch64; there is no emulator host and no m68k
 * NatFeats opcode to execute, so the complete and correct detection result
 * on this platform is "NatFeats not available". That is the documented
 * absent-path of the API, which every mintlib caller already handles:
 *   nf_init()   -> NULL  ("return the pointer to 'struct nf_ops' or NULL
 *                         when not available", nf_ops.h)
 *   nf_get_id() -> 0     (mintlib's own body below: no ops -> id 0)
 * and therefore nf_version() == 0, nf_get_name()/nf_get_fullname() == NULL,
 * nf_debug() == FALSE, nf_debugvprintf()/nf_debugprintf() == -1 with
 * errno = ENOSYS, nf_shutdown() == 0, nf_exec()/nf_execv() == -32
 * (EINVFN) -- each from its unchanged mintlib file (mintlib/nf_*.c), not
 * from here.
 *
 * The __NF cookie is not consulted: its value is an m68k structure with m68k
 * code pointers (mintlib/nf_ops.c NatFeatCookie), which an AArch64 program
 * could never call, and nothing on NewTOS installs it. This is the same
 * result as TeraDesk's app overlay (sdk/overlays/teradesk/library/utility/
 * nf_ops.c), which predates this file.
 *
 * Symbols: exactly those of mintlib/nf_ops.c -- nf_catch_sigsys, nf_init,
 * nf_get_id.
 */
#include <stddef.h>
#include <mint/arch/nf_ops.h>
#include <mint/mintbind.h>   /* Psignal (GEMDOS 0x112) */

#ifndef __MINT_SIGSYS
#define __MINT_SIGSYS		12		/* bad system call */
#endif

#ifndef __mint_sighandler_t_defined
#define __mint_sighandler_t_defined 1
typedef void __CDECL (*__mint_sighandler_t) (long signum);
#endif

static int sigsys;

static void __CDECL sigsys_handler(long sig)
{
	(void) sig;
	sigsys = 1;
}

/* mintlib/nf_ops.c, unchanged: catch (and ignore) SIGSYS on MiNT during
 * detection. A real Psignal trap; on a kernel without it the trap answers
 * EINVFN, which mintlib ignores here as well. */
void nf_catch_sigsys(void)
{
	sigsys = 0;
	(void)Psignal(__MINT_SIGSYS, sigsys_handler);
}

/* No emulator host on NewTOS/AArch64: NatFeats are not available (see the
 * file header for why this is the platform's detection result, not a
 * placeholder). */
struct nf_ops *nf_init(void)
{
	return NULL;
}

/* mintlib/nf_ops.c, unchanged: the id of a feature, or 0 when NatFeats are
 * not available. */
long nf_get_id(const char *feature_name)
{
	struct nf_ops *ops;
	long id = 0;

	if ((ops = nf_init()) != NULL)
	{
		id = NF_GET_ID(ops, feature_name);
	}
	return id;
}
