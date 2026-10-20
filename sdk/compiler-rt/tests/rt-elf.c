/*
 * rt-elf.c -- the AArch64 half of the compiler-rt host test (TASK/SDK01 E).
 *
 * Linked by rt-host.sh with sdk/bin/newtos-cc -lm into an ordinary NewTOS
 * program (crt0 + libm.a + libc.a + libclang_rt.builtins.a, sdk/newtos.ld).
 * The program is never started: rt-host.c loads its PT_LOAD segments into
 * host memory, applies the R_AARCH64_RELATIVE relocations and calls the
 * functions this table points to -- the very machine code of the SDK's
 * archives, on the AArch64 host CPU. The host's own `long double` is binary64
 * (Apple AArch64) and it has no __float128, so it cannot compute binary128
 * itself; the table gives it the target's.
 *
 * Every entry is the address of a real archive member: the compiler-rt
 * builtins directly, the libm functions that did not link before SDK01 E
 * (they pull the builtins), and two wrappers that make clang emit the
 * builtins from C (`long double` arithmetic, C99 complex multiply/divide),
 * so the test also proves clang's lowering reaches them.
 */
#include <math.h>
#include <complex.h>

typedef long double tf;

/* compiler-rt builtins (declared here: no public header has them) */
extern tf __addtf3(tf, tf), __subtf3(tf, tf), __multf3(tf, tf), __divtf3(tf, tf);
extern int __eqtf2(tf, tf), __netf2(tf, tf), __lttf2(tf, tf), __letf2(tf, tf);
extern int __gttf2(tf, tf), __getf2(tf, tf), __unordtf2(tf, tf);
extern tf __extenddftf2(double), __extendsftf2(float);
extern double __trunctfdf2(tf);
extern float __trunctfsf2(tf);
extern long long __fixtfdi(tf);
extern unsigned long long __fixunstfdi(tf);
extern tf __floatditf(long long), __floatunditf(unsigned long long);
extern int __fixtfsi(tf);
extern tf __floatsitf(int);
extern double _Complex __muldc3(double, double, double, double);
extern double _Complex __divdc3(double, double, double, double);
extern float _Complex __mulsc3(float, float, float, float);
extern float _Complex __divsc3(float, float, float, float);
extern long double _Complex __multc3(tf, tf, tf, tf);
extern tf __powitf2(tf, int);

/* clang lowers these to the builtins (checked by rt-host.sh with llvm-nm) */
tf rt_tf_mad(tf a, tf b, tf c) { return a * b + c; }
double _Complex rt_cmul(double _Complex a, double _Complex b) { return a * b; }
double _Complex rt_cdiv(double _Complex a, double _Complex b) { return a / b; }
float _Complex rt_cmulf(float _Complex a, float _Complex b) { return a * b; }

/* Order and names must match struct rt_table in rt-host.c. */
void *const rt_table[] = {
	(void *)__addtf3, (void *)__subtf3, (void *)__multf3, (void *)__divtf3,
	(void *)__eqtf2, (void *)__netf2, (void *)__lttf2, (void *)__letf2,
	(void *)__gttf2, (void *)__getf2, (void *)__unordtf2,
	(void *)__extenddftf2, (void *)__extendsftf2,
	(void *)__trunctfdf2, (void *)__trunctfsf2,
	(void *)__fixtfdi, (void *)__fixunstfdi, (void *)__floatditf, (void *)__floatunditf,
	(void *)__fixtfsi, (void *)__floatsitf,
	(void *)__muldc3, (void *)__divdc3, (void *)__mulsc3, (void *)__divsc3,
	(void *)__multc3, (void *)__powitf2,
	(void *)rt_tf_mad, (void *)rt_cmul, (void *)rt_cdiv, (void *)rt_cmulf,
	/* libm members that did not link before SDK01 E */
	(void *)powf, (void *)remainder, (void *)remainderf,
	(void *)fdim, (void *)fdimf, (void *)fdiml, (void *)nexttowardf,
	(void *)cpowf, (void *)casin, (void *)casinf, (void *)cacos, (void *)cacosf,
	(void *)casinh, (void *)casinhf, (void *)cacosh, (void *)cacoshf,
	(void *)catanh, (void *)catanhf,
	(void *)0x5254454e44ULL                       /* "RTEND" sentinel */
};

int main(void)
{
	/* keep the table; the program is never run */
	return rt_table[0] == 0;
}
