/*
 * setjmp-host.c -- host harness for sdk/libc/newtos-arch/setjmp.S (TASK/T07).
 *
 * The AArch64 setjmp is plain AAPCS64 code, so it can be exercised on a
 * macOS/arm64 host before it goes into a NewTOS program: Apple's ABI differs
 * from AAPCS64 only in the reserved x18 and the stack red zone, neither of
 * which setjmp touches. Built and run by setjmp-host.sh; the same checks run
 * on NewTOS as sdk/examples/setjmptst.c (setjmp-cases.h is shared).
 *
 * jmp_buf and the four entry points are declared here by hand (the host's
 * <setjmp.h> would describe the host libc's buffer). The two libc globals the
 * assembly reads -- mintlib's _sigmask and __mint -- are plain zeroes here,
 * which is their NewTOS value too (see setjmp.S).
 */
#include <stdio.h>

typedef long jmp_buf[22];
_Static_assert(sizeof(jmp_buf) == 176, "jmp_buf must match setjmp.S (22 slots)");

int  sigsetjmp(jmp_buf, int) __attribute__((returns_twice));
int  setjmp(jmp_buf) __attribute__((returns_twice));
void siglongjmp(jmp_buf, int) __attribute__((noreturn));
void longjmp(jmp_buf, int) __attribute__((noreturn));

long _sigmask;
int  __mint;

static void
sj_report(const char *name, int ok)
{
	printf("%s  %s\n", ok ? "PASS" : "FAIL", name);
}

#include "setjmp-cases.h"

int
main(void)
{
	int failed = sj_run_all();

	printf("setjmp-host: %d check(s) failed\n", failed);
	return failed;
}
