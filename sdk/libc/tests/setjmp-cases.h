/*
 * setjmp-cases.h -- the setjmp/longjmp checks for sdk/libc/newtos-arch/setjmp.S,
 * shared by the host harness (setjmp-host.c, run by setjmp-host.sh on a
 * macOS/arm64 host) and the NewTOS console program (sdk/examples/setjmptst.c,
 * run from C:\AUTO\ in QEMU raspi4b). TASK/T07.
 *
 * The includer provides, before including this file:
 *
 *   - jmp_buf, sigsetjmp(), setjmp(), siglongjmp(), longjmp() -- from mintlib's
 *     <setjmp.h> on the target, declared by hand on the host;
 *   - extern long _sigmask;  -- the libc's TOS-emulation signal mask
 *     (mintlib signal/initsig.c on the target, a plain global on the host);
 *   - static void sj_report(const char *name, int ok);
 *
 * and then calls sj_run_all(), which returns the number of failed checks.
 *
 * The raw function symbols are exercised as (setjmp)(env) / longjmp() because
 * mintlib's header turns the setjmp() macro into sigsetjmp(env, 1).
 */

#ifndef SETJMP_CASES_H
#define SETJMP_CASES_H

#define SJ_SIGMASK_SLOT 21	/* sizeof(jmp_buf)/sizeof(long) - 1, see setjmp.S */

static int sj_failures;

#define SJ_CHECK(name, cond)					\
	do {							\
		int sj_ok_ = (cond) ? 1 : 0;			\
		sj_report(name, sj_ok_);			\
		if (!sj_ok_)					\
			sj_failures++;				\
	} while (0)

static jmp_buf sj_env;

static void __attribute__((noinline))
sj_throw(int v)
{
	siglongjmp(sj_env, v);
}

static void __attribute__((noinline))
sj_throw_longjmp(int v)
{
	longjmp(sj_env, v);
}

/* 1. return values: 0 on the direct return, val after the jump, 1 for val 0. */
static void
sj_case_return_values(void)
{
	volatile int phase = 0;
	int r;

	r = sigsetjmp(sj_env, 0);
	if (r == 0) {
		phase = 1;
		sj_throw(7);
	}
	SJ_CHECK("sigsetjmp returns 0 first, then 7 after siglongjmp(env, 7)",
		 phase == 1 && r == 7);

	r = sigsetjmp(sj_env, 0);
	if (r == 0)
		sj_throw(0);
	SJ_CHECK("siglongjmp(env, 0) makes sigsetjmp return 1", r == 1);

	phase = 0;
	r = (setjmp)(sj_env);
	if (r == 0) {
		phase = 2;
		sj_throw_longjmp(42);
	}
	SJ_CHECK("raw setjmp/longjmp pair: 0, then 42", phase == 2 && r == 42);
}

/* 2. the signal-mask slot, as mintlib/setjmp.S (m68k) defines it. */
static void
sj_case_sigmask(void)
{
	volatile long slot = -1;

	_sigmask = 0x50;
	if (sigsetjmp(sj_env, 1) == 0) {
		slot = sj_env[SJ_SIGMASK_SLOT];
		_sigmask = 0;
		sj_throw(3);
	}
	SJ_CHECK("sigsetjmp(env, 1) stores _sigmask with bit 0 set (0x51)",
		 slot == 0x51);
	SJ_CHECK("siglongjmp restores _sigmask (0x50) when the mask was saved",
		 _sigmask == 0x50);

	_sigmask = 0x50;
	slot = -1;
	if (sigsetjmp(sj_env, 0) == 0) {
		slot = sj_env[SJ_SIGMASK_SLOT];
		_sigmask = 0;
		sj_throw(3);
	}
	SJ_CHECK("sigsetjmp(env, 0) leaves the mask slot 0", slot == 0);
	SJ_CHECK("siglongjmp leaves _sigmask alone when the mask was not saved",
		 _sigmask == 0);

	_sigmask = 0x50;
	slot = -1;
	if ((setjmp)(sj_env) == 0) {
		slot = sj_env[SJ_SIGMASK_SLOT];
		_sigmask = 0;
		sj_throw_longjmp(3);
	}
	SJ_CHECK("raw setjmp() leaves the mask slot 0", slot == 0);
	SJ_CHECK("raw longjmp() leaves _sigmask alone", _sigmask == 0);

	_sigmask = 0;
}

/* 3. callee-saved registers. The frame that calls sigsetjmp cannot observe
 *    them: clang spills every value that is live across a returns_twice call
 *    to the stack. What CAN observe them is the frame above it: its values
 *    sit in x19-x28 / d8-d15 across an ordinary call to sj_regs_frame(),
 *    which does the sigsetjmp and whose callee sj_clobber_and_throw() writes
 *    garbage into every callee-saved register and then longjmps -- skipping
 *    its own epilogue, so only longjmp's restore brings the registers back.
 *    The garbage is written by inline asm: a C loop would be scheduled into
 *    the caller-saved temporaries (x8-x17, d16-d31) and touch nothing that
 *    matters. Verified by sabotage: dropping the d8-d15 or the x19/x20
 *    restore from setjmp.S makes exactly these checks fail. Values come from
 *    a volatile so nothing folds; sinks are volatile so nothing is dropped. */
static volatile long sj_seed = 3;
static volatile long sj_sink;
static volatile double sj_dsink;

static void __attribute__((noinline))
sj_clobber_and_throw(void)
{
	__asm__ volatile(
		"mov  x19, #0x1919\n\t"  "mov  x20, #0x2020\n\t"
		"mov  x21, #0x2121\n\t"  "mov  x22, #0x2222\n\t"
		"mov  x23, #0x2323\n\t"  "mov  x24, #0x2424\n\t"
		"mov  x25, #0x2525\n\t"  "mov  x26, #0x2626\n\t"
		"mov  x27, #0x2727\n\t"  "mov  x28, #0x2828\n\t"
		"fmov d8,  #-1.0\n\t"    "fmov d9,  #-2.0\n\t"
		"fmov d10, #-3.0\n\t"    "fmov d11, #-4.0\n\t"
		"fmov d12, #-5.0\n\t"    "fmov d13, #-6.0\n\t"
		"fmov d14, #-7.0\n\t"    "fmov d15, #-8.0"
		:
		:
		: "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27", "x28",
		  "d8", "d9", "d10", "d11", "d12", "d13", "d14", "d15");
	siglongjmp(sj_env, 5);
}

static int __attribute__((noinline))
sj_regs_frame(void)
{
	int r = sigsetjmp(sj_env, 0);

	if (r == 0)
		sj_clobber_and_throw();
	return r;
}

static void __attribute__((noinline))
sj_case_callee_saved(void)
{
	long v0 = sj_seed * 1, v1 = sj_seed * 2, v2 = sj_seed * 3, v3 = sj_seed * 4;
	long v4 = sj_seed * 5, v5 = sj_seed * 6, v6 = sj_seed * 7, v7 = sj_seed * 8;
	long v8 = sj_seed * 9, v9 = sj_seed * 10;
	double f0 = sj_seed * 0.5, f1 = sj_seed * 1.5, f2 = sj_seed * 2.5;
	double f3 = sj_seed * 3.5, f4 = sj_seed * 4.5, f5 = sj_seed * 5.5;
	double f6 = sj_seed * 6.5, f7 = sj_seed * 7.5;
	int r;

	r = sj_regs_frame();

	SJ_CHECK("10 long values in x19-x28 survive a longjmp out of a register-clobbering callee",
		 r == 5 && v0 == 3 && v1 == 6 && v2 == 9 && v3 == 12 && v4 == 15 &&
		 v5 == 18 && v6 == 21 && v7 == 24 && v8 == 27 && v9 == 30);
	SJ_CHECK("8 double values in d8-d15 survive the same longjmp",
		 f0 == 1.5 && f1 == 4.5 && f2 == 7.5 && f3 == 10.5 &&
		 f4 == 13.5 && f5 == 16.5 && f6 == 19.5 && f7 == 22.5);
	sj_sink = v0 + v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + v9;
	sj_dsink = f0 + f1 + f2 + f3 + f4 + f5 + f6 + f7;
}

/* 4. nested: an inner setjmp in a deeper frame, a jump to it, then a jump
 *    out to the outer one. */
static jmp_buf sj_outer, sj_inner;
static volatile int sj_trace[8];
static volatile int sj_ti;

static void __attribute__((noinline))
sj_inner_fn(void)
{
	int r = sigsetjmp(sj_inner, 0);

	if (r == 0) {
		sj_trace[sj_ti++] = 1;
		siglongjmp(sj_inner, 2);
	}
	sj_trace[sj_ti++] = 10 + r;		/* 12 */
	siglongjmp(sj_outer, 9);
}

static void
sj_case_nested(void)
{
	int r;

	sj_ti = 0;
	r = sigsetjmp(sj_outer, 0);
	if (r == 0) {
		sj_trace[sj_ti++] = 0;
		sj_inner_fn();
		sj_trace[sj_ti++] = 99;		/* never reached */
	}
	sj_trace[sj_ti++] = 20 + r;		/* 29 */
	SJ_CHECK("nested setjmp: inner jump returns to the inner frame, outer jump leaves it",
		 sj_ti == 4 && sj_trace[0] == 0 && sj_trace[1] == 1 &&
		 sj_trace[2] == 12 && sj_trace[3] == 29);
}

/* 5. the stack pointer is restored: a local's address is the same before
 *    and after a longjmp that unwinds two frames. */
static void __attribute__((noinline))
sj_deeper(int depth)
{
	volatile char pad[64];

	pad[0] = (char)depth;
	if (depth > 0)
		sj_deeper(depth - 1);
	else
		siglongjmp(sj_env, 4);
	sj_sink = pad[0];
}

static void
sj_case_stack(void)
{
	volatile long marker = 0x5a;
	volatile long *before = &marker;
	volatile long *after;
	int r;

	r = sigsetjmp(sj_env, 0);
	if (r == 0)
		sj_deeper(3);
	after = &marker;
	SJ_CHECK("the frame is intact after unwinding four frames",
		 r == 4 && before == after && marker == 0x5a);
}

static int
sj_run_all(void)
{
	sj_failures = 0;
	sj_case_return_values();
	sj_case_sigmask();
	sj_case_callee_saved();
	sj_case_nested();
	sj_case_stack();
	return sj_failures;
}

#endif /* SETJMP_CASES_H */
