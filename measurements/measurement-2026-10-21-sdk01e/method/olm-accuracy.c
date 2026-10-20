/* olm-accuracy.c -- how far OpenLibm 0.8.8's complex inverse functions are from the macOS system libm (an
   independent implementation), measured on the HOST build of OpenLibm (rt-host proved it bit-identical to the
   SDK's AArch64 build on these functions). Informational: upstream behaviour, not an SDK01 E defect.
   Build: cc -O2 olm-accuracy.c <host-openlibm.a>   (olm-accuracy.sh) */
#include <complex.h>
#include <dlfcn.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
static uint64_t rs = 0x9e3779b97f4a7c15ULL;
static uint64_t rnd(void) { rs ^= rs << 13; rs ^= rs >> 7; rs ^= rs << 17; return rs; }
typedef double complex (*cd)(double complex); typedef float complex (*cf)(float complex);
int main(void) {
	void *h = dlopen("/usr/lib/libSystem.B.dylib", RTLD_LAZY | RTLD_LOCAL);
	const char *nd[] = { "casin", "cacos", "casinh", "cacosh", "catanh" };
	const char *nf[] = { "casinf", "cacosf", "casinhf", "cacoshf", "catanhf" };
	cd od[] = { casin, cacos, casinh, cacosh, catanh }; cf of[] = { casinf, cacosf, casinhf, cacoshf, catanhf };
	for (int k = 0; k < 10; k++) {
		int isf = k >= 5; const char *n = isf ? nf[k - 5] : nd[k];
		void *sys = dlsym(h, n);
		double mx = 0; long big = 0, nonfin = 0, signim = 0, N = 50000; double sa = 0, sb = 0, srel = 0;
		rs = 0x9e3779b97f4a7c15ULL;
		for (long i = 0; i < N; i++) {
			double a = ldexp((double)(int64_t)(rnd() % 2000001) - 1000000, -(int)(rnd() % 26));
			double b = ldexp((double)(int64_t)(rnd() % 2000001) - 1000000, -(int)(rnd() % 26));
			double complex g, w;
			if (isf) { g = of[k - 5](CMPLXF((float)a, (float)b)); w = ((cf)sys)(CMPLXF((float)a, (float)b)); }
			else { g = od[k](CMPLX(a, b)); w = ((cd)sys)(CMPLX(a, b)); }
			if (!isfinite(creal(g)) || !isfinite(cimag(g))) { nonfin++; continue; }
			if (signbit(cimag(g)) != signbit(cimag(w)) && cimag(w) != 0) signim++;
			double e = cabs(g - w) / cabs(w);
			if (e > (isf ? 16 * FLT_EPSILON : 8 * DBL_EPSILON)) big++;
			if (e > mx && signbit(cimag(g)) == signbit(cimag(w))) { mx = e; sa = a; sb = b; srel = e; }
		}
		printf("%-8s %ld args: %6ld beyond %s eps*|w| (any cause), %6ld non-finite where the system is finite, %6ld with the imaginary sign flipped; "
		       "worst same-sign relative error %.3g at z=(%a,%a)\n", n, N, big, isf ? "16" : "8", nonfin, signim, srel, sa, sb);
		(void)mx;
	}
	return 0;
}
