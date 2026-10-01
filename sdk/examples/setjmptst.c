/*
 * setjmptst.c -- setjmp/longjmp acceptance for TASK/T07, as a NewTOS console
 * program linked against the ported mintlib.
 *
 * It runs the checks in sdk/libc/tests/setjmp-cases.h (the same ones the
 * macOS/arm64 host harness sdk/libc/tests/setjmp-host.sh runs) against the
 * archive's sigsetjmp/setjmp/siglongjmp/longjmp -- sdk/libc/newtos-arch/
 * setjmp.S -- through mintlib's own <setjmp.h>, whose jmp_buf is long[22] on
 * AArch64 (sdk/libc/overlay/include/setjmp.h). Every verdict goes out through
 * GEMDOS Cconws so the serial log carries it independently of stdio.
 *
 * Build (mintlib-linked, so not sdk/build.sh):
 *     sdk/libc/build-libc.sh sdk/examples/setjmptst.c sdk/libc/build/setjmptst.NCP
 * Run: as C:\AUTO\SETJMPT.NCP in QEMU raspi4b; expect
 *     setjmptst: PASS ...   (13 lines)
 *     setjmptst: ALL PASS (13 checks)
 *
 * Deliberately **no** key pause (TASK/T13): \AUTO\ is this program's route of
 * record -- tools/make-sd-image.sh puts it there on SD_SETJMPTST_AUTO=1 and
 * TASK/T07's acceptance ran it exactly that way
 * (measurements/measurement-2026-09-21-t07/method/run.sh) -- and a key wait in \AUTO\
 * stops the boot. Its verdict is a serial line, not a screen to read.
 */
#include <setjmp.h>
#include <mint/osbind.h>

/* mintlib's TOS-emulation signal mask (signal/initsig.c); declared in the
 * library-internal lib.h, so declared here by hand. */
extern long _sigmask;

_Static_assert(sizeof(jmp_buf) == 22 * sizeof(long),
	       "mintlib jmp_buf must be the 22-slot AArch64 layout of setjmp.S");
_Static_assert(sizeof(jmp_buf) == 176, "jmp_buf is 176 bytes on AArch64");

static void
put_dec(unsigned n)
{
	char buf[12];
	int i = sizeof buf - 1;

	buf[i] = '\0';
	do {
		buf[--i] = (char)('0' + n % 10);
		n /= 10;
	} while (n);
	Cconws(&buf[i]);
}

static unsigned checks;

static void
sj_report(const char *name, int ok)
{
	checks++;
	Cconws("setjmptst: ");
	Cconws(ok ? "PASS " : "FAIL ");
	Cconws(name);
	Cconws("\r\n");
}

#include "../libc/tests/setjmp-cases.h"

int
main(void)
{
	int failed = sj_run_all();

	if (failed) {
		Cconws("setjmptst: FAIL (");
		put_dec((unsigned)failed);
		Cconws(" of ");
		put_dec(checks);
		Cconws(" checks failed)\r\n");
	} else {
		Cconws("setjmptst: ALL PASS (");
		put_dec(checks);
		Cconws(" checks)\r\n");
	}
	return failed;
}
