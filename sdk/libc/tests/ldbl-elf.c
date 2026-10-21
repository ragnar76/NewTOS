/*
 * ldbl-elf.c -- the AArch64 half of the long double host test (TASK/SDK01 J).
 *
 * Linked by ldbl-host.sh with sdk/bin/newtos-cc -lm into an ordinary NewTOS
 * program (crt0 + libm.a + libc.a + compiler-rt builtins, sdk/newtos.ld). The
 * program is never started: ldbl-host.c loads it into host memory (the loader
 * of sdk/compiler-rt/tests/rt-host.c) and calls the functions of ldbl_table
 * -- the machine code of the SDK's libc/libm members, on the AArch64 host CPU.
 * The host's `long double' is binary64 (Apple AArch64) and it has no
 * __float128; a binary128 crosses the boundary as a 16-byte vector in a Q
 * register, which AAPCS64 and Apple's variant pass alike.
 *
 * Variadic functions are NOT called across the boundary (Apple's AArch64 ABI
 * passes variadic arguments on the stack, AAPCS64 in registers): printf and
 * scanf are reached through the fixed-argument wrappers below, which are
 * compiled for the target and call the target's snprintf/sscanf.
 *
 * Nothing reached here may trap into GEMDOS (there is no kernel on the host):
 * snprintf/sprintf/sscanf/strtold/qecvt/qecvt_r/qfcvt_r/qgcvt and the libm
 * functions use no malloc for the arguments the oracle sends. qfcvt (static
 * buffer + malloc fallback + atexit) is exercised by the QEMU probe instead.
 */
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

typedef long double tf;

/* mintlib's long double conversions (misc/qefgcvt*.c); no public prototype */
extern char *qecvt(tf, int, int *, int *);
extern char *qgcvt(tf, int, char *);
extern int qecvt_r(tf, int, int *, int *, char *, size_t);
extern int qfcvt_r(tf, int, int *, int *, char *, size_t);
/* mintlib's classification functions (mintlib/s_*l.c) */
extern int __isnanl(tf), __isinfl(tf), __finitel(tf), __signbitl(tf);

/* binary128 arithmetic as clang lowers it (compiler-rt __addtf3 & Co.):
   the oracle checks full 113-bit operands -- SDK01 E's host test only had
   binary64-wide ones */
tf ld_add(tf a, tf b) { return a + b; }
tf ld_sub(tf a, tf b) { return a - b; }
tf ld_mul(tf a, tf b) { return a * b; }
tf ld_div(tf a, tf b) { return a / b; }

/* fixed-argument wrappers around the variadic functions */
int ld_snprintf(char *b, size_t n, const char *f, tf v) { return snprintf(b, n, f, v); }
int ld_snprintf_d(char *b, size_t n, const char *f, double v) { return snprintf(b, n, f, v); }
int ld_sscanf(const char *s, const char *f, tf *o) { return sscanf(s, f, o); }
int ld_sscanf_d(const char *s, const char *f, double *o) { return sscanf(s, f, o); }

/* kinds -- must match ldbl-host.c */
enum { K_FMT = 1, K_FMTD, K_SCAN, K_SCAND, K_STRTOLD, K_STRTOD, K_ECVT, K_ECVT_R, K_GCVT,
       K_Q_Q, K_Q_QQ, K_Q_QQQ, K_Q_QI, K_Q_QL, K_D_DL, K_F_FL, K_Q_QPI, K_I_Q, K_Q_QPQ,
       K_Q_QQPI };

struct ent { const char *name; void *fn; long kind; };

const struct ent ldbl_table[] = {
	{ "snprintf",  (void *)ld_snprintf,   K_FMT },
	{ "snprintfd", (void *)ld_snprintf_d, K_FMTD },
	{ "sscanf",    (void *)ld_sscanf,     K_SCAN },
	{ "sscanfd",   (void *)ld_sscanf_d,   K_SCAND },
	{ "strtold",   (void *)strtold,       K_STRTOLD },
	{ "strtod",    (void *)strtod,        K_STRTOD },
	{ "qecvt",     (void *)qecvt,         K_ECVT },
	{ "qecvt_r",   (void *)qecvt_r,       K_ECVT_R },
	{ "qfcvt_r",   (void *)qfcvt_r,       K_ECVT_R },
	{ "qgcvt",     (void *)qgcvt,         K_GCVT },
	{ "isnanl",    (void *)__isnanl,      K_I_Q },
	{ "isinfl",    (void *)__isinfl,      K_I_Q },
	{ "finitel",   (void *)__finitel,     K_I_Q },
	{ "signbitl",  (void *)__signbitl,    K_I_Q },
	{ "add",       (void *)ld_add,        K_Q_QQ },
	{ "sub",       (void *)ld_sub,        K_Q_QQ },
	{ "mul",       (void *)ld_mul,        K_Q_QQ },
	{ "div",       (void *)ld_div,        K_Q_QQ },
	/* libm: the OpenLibm ld128 / long double members (TASK/SDK01 J) */
	{ "scalbnl",   (void *)scalbnl,   K_Q_QI },
	{ "ldexpl",    (void *)ldexpl,    K_Q_QI },
	{ "scalblnl",  (void *)scalblnl,  K_Q_QL },
	{ "scalbln",   (void *)scalbln,   K_D_DL },
	{ "scalblnf",  (void *)scalblnf,  K_F_FL },
	{ "frexpl",    (void *)frexpl,    K_Q_QPI },
	{ "ilogbl",    (void *)ilogbl,    K_I_Q },
	{ "logbl",     (void *)logbl,     K_Q_Q },
	{ "modfl",     (void *)modfl,     K_Q_QPQ },
	{ "remquol",   (void *)remquol,   K_Q_QQPI },
	{ "sqrtl",     (void *)sqrtl,     K_Q_Q },
	{ "fabsl",     (void *)fabsl,     K_Q_Q },
	{ "floorl",    (void *)floorl,    K_Q_Q },
	{ "ceill",     (void *)ceill,     K_Q_Q },
	{ "truncl",    (void *)truncl,    K_Q_Q },
	{ "roundl",    (void *)roundl,    K_Q_Q },
	{ "rintl",     (void *)rintl,     K_Q_Q },
	{ "copysignl", (void *)copysignl, K_Q_QQ },
	{ "nextafterl",(void *)nextafterl,K_Q_QQ },
	{ "fmodl",     (void *)fmodl,     K_Q_QQ },
	{ "remainderl",(void *)remainderl,K_Q_QQ },
	{ "fmal",      (void *)fmal,      K_Q_QQQ },
	{ "expl",      (void *)expl,      K_Q_Q },
	{ "exp2l",     (void *)exp2l,     K_Q_Q },
	{ "expm1l",    (void *)expm1l,    K_Q_Q },
	{ "logl",      (void *)logl,      K_Q_Q },
	{ "log2l",     (void *)log2l,     K_Q_Q },
	{ "log10l",    (void *)log10l,    K_Q_Q },
	{ "log1pl",    (void *)log1pl,    K_Q_Q },
	{ "powl",      (void *)powl,      K_Q_QQ },
	{ "cbrtl",     (void *)cbrtl,     K_Q_Q },
	{ "hypotl",    (void *)hypotl,    K_Q_QQ },
	{ "sinl",      (void *)sinl,      K_Q_Q },
	{ "cosl",      (void *)cosl,      K_Q_Q },
	{ "tanl",      (void *)tanl,      K_Q_Q },
	{ "atanl",     (void *)atanl,     K_Q_Q },
	{ "sinhl",     (void *)sinhl,     K_Q_Q },
	{ "coshl",     (void *)coshl,     K_Q_Q },
	{ "tanhl",     (void *)tanhl,     K_Q_Q },
	{ 0, 0, 0 }
};

int main(void)
{
	/* keep the table; the program is never run */
	return ldbl_table[0].fn == 0;
}
