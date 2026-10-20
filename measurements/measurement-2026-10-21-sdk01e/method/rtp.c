/*
 * rtp.c -- TASK/SDK01 package E runtime probe: compiler-rt builtins and the
 * libm/libc functions that did not link before package E, run on NewTOS
 * (QEMU raspi4b, C:\AUTO\RTP.NCP, console, before the AES).
 *
 * Built with `sdk/bin/newtos-cc -lm` (build-probe.sh): the SHIPPED archives
 * (libm.a, libc.a, libclang_rt.builtins.a), i.e. the machine code that also
 * uses x18 -- on NewTOS the kernel preserves it.
 *
 * Expected values: exact results where the operation is exact; for the
 * OpenLibm functions the bit patterns the host build of the same OpenLibm
 * source gives (method/expected.txt) -- rt-host proved that build bit-exact
 * against the target build on 50289 arguments.
 *
 *   B  binary128 arithmetic from C: 1/3, (2^52+1)(2^52-1)+1 = 2^104,
 *      FE_UPWARD/FE_DOWNWARD via fesetround (FPCR at EL0), long double
 *      compare/convert, __fixtfdi/__floatditf via casts
 *   X  complex * and / from C (__muldc3/__divdc3/__mulsc3/__divsc3) and an
 *      Annex G infinity
 *   M  powf, remainder, remainderf, fdim, fdimf, fdiml, nexttowardf, casin,
 *      cacos, casinh, cacosh, catanh (+f), cpowf
 *   Q  qecvt/qfcvt/qgcvt: they link now; their digits are printed as INFO
 *      (mintlib formats them with "%Lf", and libc is built with
 *      -D__NO_LONG_DOUBLE_MATH, so vfprintf reads a double where a binary128
 *      was passed -- the expected wrong result is recorded, not hidden)
 *
 * Every check prints "[RTP] ok|FAIL ..."; ends with "RTP RESULT fails=<n>".
 */
#include <complex.h>
#include <fenv.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <mint/osbind.h>

typedef unsigned long long u64;
static int fails;

static void hex64(u64 v)
{
	static const char d[] = "0123456789abcdef";
	char b[19];
	b[0] = '0'; b[1] = 'x';
	for (int i = 0; i < 16; i++) b[2 + i] = d[(v >> ((15 - i) * 4)) & 0xf];
	b[18] = 0;
	Cconws(b);
}
static void num(long v)
{
	char b[24]; int i = 23; int neg = v < 0; unsigned long u = neg ? -(unsigned long)v : (unsigned long)v;
	b[i] = 0;
	do { b[--i] = (char)('0' + u % 10); u /= 10; } while (u);
	if (neg) b[--i] = '-';
	Cconws(b + i);
}
static void line(const char *tag, const char *what) { Cconws("[RTP] "); Cconws(tag); Cconws(what); }
static void check(const char *what, int ok)
{
	line(ok ? "ok   " : "FAIL ", what); Cconws("\r\n");
	if (!ok) fails++;
}
static u64 db(double d) { u64 u; memcpy(&u, &d, 8); return u; }
static unsigned fb(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static void ldbits(long double x, u64 *hi, u64 *lo) { u64 w[2]; memcpy(w, &x, 16); *lo = w[0]; *hi = w[1]; }
static void checkld(const char *what, long double x, u64 hi, u64 lo)
{
	u64 h, l; ldbits(x, &h, &l);
	line(h == hi && l == lo ? "ok   " : "FAIL ", what);
	Cconws(" = "); hex64(h); hex64(l); Cconws("\r\n");
	if (h != hi || l != lo) fails++;
}
static void checkd(const char *what, double x, u64 want)
{
	int ok = db(x) == want;
	line(ok ? "ok   " : "FAIL ", what); Cconws(" = "); hex64(db(x));
	if (!ok) { Cconws(" want "); hex64(want); }
	Cconws("\r\n");
	if (!ok) fails++;
}
static void checkf(const char *what, float x, unsigned want)
{
	int ok = fb(x) == want;
	line(ok ? "ok   " : "FAIL ", what); Cconws(" = "); hex64(fb(x));
	if (!ok) { Cconws(" want "); hex64(want); }
	Cconws("\r\n");
	if (!ok) fails++;
}
static void checkc(const char *what, double complex z, u64 re, u64 im)
{
	int ok = db(creal(z)) == re && db(cimag(z)) == im;
	line(ok ? "ok   " : "FAIL ", what); Cconws(" = "); hex64(db(creal(z))); Cconws(" "); hex64(db(cimag(z)));
	Cconws("\r\n");
	if (!ok) fails++;
}
static void checkcf(const char *what, float complex z, unsigned re, unsigned im)
{
	int ok = fb(crealf(z)) == re && fb(cimagf(z)) == im;
	line(ok ? "ok   " : "FAIL ", what); Cconws(" = "); hex64(fb(crealf(z))); Cconws(" "); hex64(fb(cimagf(z)));
	Cconws("\r\n");
	if (!ok) fails++;
}

/* volatile operands: the compiler must emit the builtin calls, not fold */
static volatile long double one = 1.0L, three = 3.0L, tiny = 0x1p-200L;
static volatile double d52p = 0x1p52 + 1, d52m = 0x1p52 - 1;
static volatile float f2 = 2.0f, fhalf = 0.5f;

int main(void)
{
	Cconws("[RTP] SDK01 E probe start\r\n");

	/* B */
	checkld("B 1.0L/3", one / three, 0x3ffd555555555555ULL, 0x5555555555555555ULL);
	long double p = (long double)d52p * (long double)d52m + one;
	checkd("B (2^52+1)(2^52-1)+1 as long double -> double", (double)p, 0x4670000000000000ULL);
	fesetround(FE_UPWARD);
	long double up = one + tiny;
	fesetround(FE_DOWNWARD);
	long double dn = one - tiny;
	fesetround(FE_TONEAREST);
	long double ne = one + tiny;
	checkld("B FE_UPWARD 1+2^-200", up, 0x3fff000000000000ULL, 1);
	checkld("B FE_DOWNWARD 1-2^-200", dn, 0x3ffeffffffffffffULL, 0xffffffffffffffffULL);
	checkld("B FE_TONEAREST 1+2^-200", ne, 0x3fff000000000000ULL, 0);
	check("B long double compare 1/3 < 0.3333333333333333334L", one / three < 0.3333333333333333334L);
	check("B long double compare 1+2^-200 > 1 (upward)", up > one && !(ne > one));
	volatile long double big = 0x1p62L + 12345;
	check("B (long long)(2^62+12345)L", (long long)big == 0x4000000000003039LL);
	volatile long long ll = -1234567890123LL;
	check("B (long double)ll round trip", (long long)(long double)ll == ll);
	checkf("B (float)(1.0L/3)", (float)(one / three), 0x3eaaaaabU);

	/* X */
	volatile double complex x = CMPLX(1.0, 2.0), y = CMPLX(3.0, 4.0);
	checkc("X (1+2i)*(3+4i)", x * y, db(-5.0), db(10.0));
	volatile double complex a = CMPLX(1.0, 1.0), b = CMPLX(1.0, -1.0);
	checkc("X (1+i)/(1-i)", a / b, db(0.0), db(1.0));
	volatile float complex xf = CMPLXF(1.0f, 2.0f), yf = CMPLXF(3.0f, 4.0f);
	checkcf("X float (1+2i)*(3+4i)", xf * yf, fb(-5.0f), fb(10.0f));
	checkcf("X float (1+2i)/(3+4i)", xf / yf, fb(0.44f), fb(0.08f));
	volatile double complex inf = CMPLX(INFINITY, NAN), o1 = CMPLX(1.0, 0.0);
	double complex r = inf * o1;
	check("X Annex G (inf+i nan)*(1+0i) keeps an infinity", isinf(creal(r)) || isinf(cimag(r)));

	/* M */
	checkf("M powf(2, 0.5)", powf(f2, fhalf), 0x3fb504f3U);
	checkf("M powf(10, 3.5)", powf(10.0f * fhalf * f2, 3.5f), 0x4545a471U);
	checkf("M powf(-2, 3)", powf(-f2, 3.0f), 0xc1000000U);
	checkf("M powf(0.3, -7.25)", powf(0.3f * fhalf * f2, -7.25f), 0x45c1129aU);
	check("M powf(NaN, 2) is NaN", isnan(powf(NAN, f2)));
	check("M powf(2, NaN) is NaN", isnan(powf(f2, NAN)));
	checkf("M powf(1, NaN)", powf(fhalf * f2, NAN), 0x3f800000U);
	checkd("M remainder(10, 3)", remainder(10.0 * fhalf * f2, 3.0), db(1.0));
	checkd("M remainder(11, 3)", remainder(11.0 * fhalf * f2, 3.0), db(-1.0));
	checkd("M remainder(1e300, 3.7)", remainder(1e300 * fhalf * f2, 3.7), 0x3ffc408f4350ed98ULL);
	checkf("M remainderf(5.5, 2)", remainderf(5.5f * fhalf * f2, 2.0f), fb(-0.5f));
	checkf("M remainderf(1e30, 3.7)", remainderf(1e30f * fhalf * f2, 3.7f), 0xbf309424U);
	checkd("M fdim(5, 3)", fdim(5.0 * fhalf * f2, 3.0), db(2.0));
	checkd("M fdim(3, 5)", fdim(3.0 * fhalf * f2, 5.0), db(0.0));
	checkf("M fdimf(3, 5)", fdimf(3.0f * fhalf * f2, 5.0f), 0);
	checkld("M fdiml(5, 3)", fdiml(5.0L * one, 3.0L), 0x4000000000000000ULL, 0);
	checkf("M nexttowardf(1, 2L)", nexttowardf(fhalf * f2, 2.0L * one), 0x3f800001U);
	checkf("M nexttowardf(-0, -1L)", nexttowardf(-0.0f * f2, -one), 0x80000001U);
	volatile double complex z = CMPLX(0.5, 0.5);
	volatile float complex zf = CMPLXF(0.5f, 0.5f);
	checkc("M casin(0.5+0.5i)", casin(z), 0x3fdcf2214ccccd44ULL, 0x3fe0fafb8f2f147eULL);
	checkc("M cacos(0.5+0.5i)", cacos(z), 0x3ff1e5730110f9c7ULL, 0xbfe0fafb8f2f147eULL);
	checkc("M casinh(0.5+0.5i)", casinh(z), 0x3fe0fafb8f2f147eULL, 0x3fdcf2214ccccd44ULL);
	checkc("M cacosh(0.5+0.5i)", cacosh(z), 0x3fe0fafb8f2f147eULL, 0x3ff1e5730110f9c7ULL);
	checkc("M catanh(0.5+0.5i)", catanh(z), 0x3fd9c041f7ed8d33ULL, 0x3fe1b6e192ebbe44ULL);
	checkcf("M casinf(0.5+0.5i)", casinf(zf), 0x3ee7910aU, 0x3f07d7dcU);
	checkcf("M cacosf(0.5+0.5i)", cacosf(zf), 0x3f8f2b98U, 0xbf07d7dcU);
	checkcf("M casinhf(0.5+0.5i)", casinhf(zf), 0x3f07d7dcU, 0x3ee7910aU);
	checkcf("M cacoshf(0.5+0.5i)", cacoshf(zf), 0x3f07d7dcU, 0x3f8f2b98U);
	checkcf("M catanhf(0.5+0.5i)", catanhf(zf), 0x3ece0210U, 0x3f0db70dU);
	checkcf("M cpowf(1+i, 2)", cpowf(CMPLXF(fhalf * f2, 1.0f), CMPLXF(2.0f, 0.0f)), 0xb3bbbd2dU, 0x3fffffffU);
	checkcf("M cpowf(0.5-1.5i, -0.5+i)", cpowf(CMPLXF(fhalf, -1.5f), CMPLXF(-0.5f, 1.0f)), 0x3fa6776cU, 0x401cc093U);

	/* Q (INFO, see header) */
	int dp = 0, sg = 0;
	char *s = qecvt(one / three, 10, &dp, &sg);
	check("Q qecvt returns a string", s != 0);
	line("INFO ", "qecvt(1.0L/3, 10) = \""); Cconws(s ? s : "(null)"); Cconws("\" decpt="); num(dp);
	Cconws(" sign="); num(sg); Cconws("   (correct: \"3333333333\" decpt=0 sign=0)\r\n");
	s = qfcvt(2.5L * one, 3, &dp, &sg);
	line("INFO ", "qfcvt(2.5L, 3) = \""); Cconws(s ? s : "(null)"); Cconws("\" decpt="); num(dp);
	Cconws(" sign="); num(sg); Cconws("   (correct: \"2500\" decpt=1 sign=0)\r\n");
	char gb[64];
	s = qgcvt(0.125L * one, 6, gb);
	line("INFO ", "qgcvt(0.125L, 6) = \""); Cconws(s ? s : "(null)"); Cconws("\"   (correct: \"0.125\")\r\n");

	Cconws("RTP RESULT fails="); num(fails); Cconws("\r\n");
	return fails ? 1 : 0;
}
