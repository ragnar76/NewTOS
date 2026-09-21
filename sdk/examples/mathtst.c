/*
 * mathtst.c -- libm acceptance for TASK/T07, as a NewTOS console program
 * linked against libm.a (OpenLibm, sdk/libm/) and the ported mintlib.
 *
 * Checks a handful of functions against values known in closed form, each
 * with an absolute tolerance; every verdict goes out through GEMDOS Cconws
 * (with a small fixed-point printer of its own, so the result does not hang
 * on stdio's float formatting). Includes the standard <math.h> name, i.e. the
 * adapter sdk/libm/include/math.h over <openlibm_math.h>.
 *
 * Build (links libm.a + libc.a):
 *     sdk/libm/build-libm.sh sdk/examples/mathtst.c sdk/libm/build/mathtst.NCP
 * Run: as C:\AUTO\MATHTST.NCP in QEMU raspi4b; expect
 *     mathtst: PASS ...   (one line per check)
 *     mathtst: ALL PASS (N checks)
 */
#include <math.h>
#include <mint/osbind.h>

static unsigned checks, failed;

static void
put_dec(unsigned long n)
{
	char buf[24];
	int i = sizeof buf - 1;

	buf[i] = '\0';
	do {
		buf[--i] = (char)('0' + n % 10);
		n /= 10;
	} while (n);
	Cconws(&buf[i]);
}

/* prints v with six decimals; NaN/inf spelled out */
static void
put_double(double v)
{
	unsigned long ip, frac;

	if (isnan(v)) { Cconws("nan"); return; }
	if (isinf(v)) { Cconws(v < 0 ? "-inf" : "inf"); return; }
	if (v < 0) { Cconws("-"); v = -v; }
	ip = (unsigned long)v;
	frac = (unsigned long)((v - (double)ip) * 1000000.0 + 0.5);
	if (frac >= 1000000) { ip++; frac -= 1000000; }
	put_dec(ip);
	Cconws(".");
	{
		char f[7];
		int i;
		for (i = 5; i >= 0; i--) { f[i] = (char)('0' + frac % 10); frac /= 10; }
		f[6] = '\0';
		Cconws(f);
	}
}

static void
check(const char *name, double got, double want, double tol)
{
	int ok = fabs(got - want) <= tol;

	checks++;
	if (!ok)
		failed++;
	Cconws("mathtst: ");
	Cconws(ok ? "PASS " : "FAIL ");
	Cconws(name);
	Cconws(" = ");
	put_double(got);
	Cconws(" (want ");
	put_double(want);
	Cconws(")\r\n");
}

static void
check_flag(const char *name, int ok)
{
	checks++;
	if (!ok)
		failed++;
	Cconws("mathtst: ");
	Cconws(ok ? "PASS " : "FAIL ");
	Cconws(name);
	Cconws("\r\n");
}

/* volatile so nothing is folded at compile time -- the library must run. */
static volatile double two = 2.0, half = 0.5, ten = 10.0, m15 = -1.5;
static volatile double eight = 8.0, one = 1.0, seven = 7.0, three = 3.0;
static volatile double minus1 = -1.0;

int
main(void)
{
	int e = 0;
	double ip = 0.0, m;

	check("sqrt(2)",          sqrt(two),                1.41421356237309505, 1e-12);
	check("sin(M_PI/6)",      sin(M_PI / 6.0),          half,                1e-12);
	check("cos(M_PI/3)",      cos(M_PI / 3.0),          half,                1e-12);
	check("pow(2,10)",        pow(two, ten),            1024.0,              0.0);
	check("floor(-1.5)",      floor(m15),               -2.0,                0.0);
	check("ceil(-1.5)",       ceil(m15),                -1.0,                0.0);
	m = frexp(eight, &e);
	check("frexp(8.0) mantissa", m,                     half,                0.0);
	check_flag("frexp(8.0) exponent == 4", e == 4);
	check("ldexp(0.5,4)",     ldexp(half, 4),           eight,               0.0);
	check("atan2(1,1)",       atan2(one, one),          M_PI / 4.0,          1e-12);
	check("exp(1)",           exp(one),                 M_E,                 1e-12);
	check("log(M_E)",         log(M_E),                 one,                 1e-12);
	check("log10(1000)",      log10(ten * ten * ten),   three,               1e-12);
	check("fmod(7,3)",        fmod(seven, three),       one,                 0.0);
	m = modf(m15, &ip);
	check("modf(-1.5) frac",  m,                        -0.5,                0.0);
	check("modf(-1.5) int",   ip,                       -1.0,                0.0);
	check("fabs(-1.5)",       fabs(m15),                1.5,                 0.0);
	check("hypot(3,4)",       hypot(three, 4.0),        5.0,                 1e-12);
	check("tan(M_PI/4)",      tan(M_PI / 4.0),          one,                 1e-12);
	check("cbrt(8)",          cbrt(eight),              two,                 1e-12);
	check("sqrtf(2)",         (double)sqrtf((float)two), 1.41421356f,        1e-6);
	check_flag("sqrt(-1) is NaN", isnan(sqrt(minus1)));
	check_flag("log(0) is -inf", isinf(log(0.0 * one)) && log(0.0 * one) < 0);

	if (failed) {
		Cconws("mathtst: FAIL (");
		put_dec(failed);
		Cconws(" of ");
		put_dec(checks);
		Cconws(" checks failed)\r\n");
	} else {
		Cconws("mathtst: ALL PASS (");
		put_dec(checks);
		Cconws(" checks)\r\n");
	}
	return (int)failed;
}
