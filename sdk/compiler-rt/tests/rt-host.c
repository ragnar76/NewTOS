/*
 * rt-host.c -- host test of the SDK's compiler-rt builtins and of the libm
 * functions that need them (TASK/SDK01 E).
 *
 *   rt-host <program.elf>
 *
 * <program.elf> is rt-elf.c linked by rt-host.sh against the SDK's builtins
 * and libm, rebuilt from the same sources with the same flags plus
 * -ffixed-x18 (see "x18" below), with newtos.ld's data segment moved to a
 * 16 KiB boundary so text and data can get different host page protections.
 * This program loads its PT_LOAD segments, applies the R_AARCH64_RELATIVE
 * relocations of its .rela.dyn, makes the text R-X, finds `rt_table` in its
 * .symtab and calls the AArch64 machine code directly on the host CPU. Runs
 * only on an AArch64 host (Apple Silicon); elsewhere it exits 77 (skip).
 *
 * The host program itself links the HOST build of the same OpenLibm sources
 * (rt-host.sh compiles them with OpenLibm's own Darwin/Linux flags): that is
 * the same-source oracle for the libm functions. The host system libm is
 * reached through dlopen/dlsym as the independent oracle.
 *
 * ABI bridge: the target's `long double` is IEEE binary128, passed and
 * returned in a Q register (AAPCS64). The host (Apple AArch64) has a 64-bit
 * `long double` and no __float128, so a binary128 is carried here as a
 * 16-byte short vector (q_t), which AAPCS64 and Apple's variant both pass in a
 * Q register. float/double/complex arguments and results use the same
 * registers on both sides (HFAs in s/d registers, integers in x/w).
 *
 * Oracles (no tolerance where the result is exact):
 *   T  binary128 + - * / : trunc(op(ext(a), ext(b))) == a op b of the host,
 *      bit for bit, for 200000 random pairs per operation over the whole
 *      binary64 range (incl. subnormals, zeros, infinities, NaNs). Valid
 *      because 113 >= 2*53+2: rounding first to binary128 and then to binary64
 *      equals one rounding to binary64 for + - * / (double rounding is
 *      innocuous, Figueroa 1995). Plus exact binary128 bit patterns (1/3,
 *      1+2^-112, ties to even) and the rounding mode from FPCR
 *      (aarch64/fp_mode.c): FE_UPWARD / FE_DOWNWARD / FE_TOWARDZERO.
 *   C  comparisons: the sign of __lttf2 & Co. equals the host predicate on
 *      the same doubles; unordered (NaN) returns the documented side.
 *   V  conversions: float/double <-> binary128 round trips bit-exact,
 *      __trunctfsf2(ext(d)) == (float)d, integer conversions == host casts.
 *   X  complex: __muldc3/__divdc3/__mulsc3/__divsc3 and clang's own lowering
 *      of `*`, `/` (rt_cmul ...) == the host's complex `*`/`/` (Annex G
 *      algorithm of compiler-rt in both) bit for bit, incl. inf/NaN operands;
 *      __multc3 / __powitf2 / long double a*b+c on exact values.
 *   M  libm (OpenLibm, the members that did not link before SDK01 E):
 *      remainder, remainderf, fdim, fdimf, nexttowardf bit-exact against the
 *      host system libm (all are exact operations), the first four also
 *      against host OpenLibm (its nexttowardf is wrong on a 64-bit-long-double
 *      host, see test_libm);
 *      fdiml on binary128 values; powf bit-exact against host OpenLibm
 *      (NaN: any NaN -- the NaN path is the one that runs in binary128 on the
 *      target and in binary64 on the host) and within 1 ulp of the host
 *      system libm (both claim < 1 ulp), plus special values; casin, cacos,
 *      casinh, cacosh, catanh and their f forms, cpowf bit-exact against host
 *      OpenLibm (NaN: any NaN) on random and special arguments. They are
 *      NOT compared with the system libm: OpenLibm 0.8.8's casin family is
 *      the old Cephes code and differs from it far beyond rounding (the host
 *      build of the same source gives the same values, i.e. upstream
 *      behaviour; docs/sdk/07-mintlib-limits.md section 4).
 *
 * x18: aarch64-unknown-none-elf does not reserve x18, so the SDK's archives
 * use it as an ordinary register (the NewTOS kernel saves and restores it,
 * kernel/src/arch/aarch64/exception_vectors.s). macOS does not preserve x18
 * for user code across context switches: run against the shipped archives,
 * 3 of 10 identical runs had one wrong __divtf3 quotient each, on different
 * operands every time; the -ffixed-x18 rebuild: 0 of 10
 * (measurements/measurement-2026-10-21-sdk01e/checks/x18-experiment.txt). Hence
 * the -ffixed-x18 rebuild; rt-host.sh first proves that the same rebuild
 * WITHOUT -ffixed-x18 is byte-identical to every member of the shipped
 * archives, so only the register reservation differs. The shipped machine
 * code itself runs on NewTOS in the QEMU probe of that run.
 */
#include <complex.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <fenv.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <libkern/OSCacheControl.h>
#else
static void sys_icache_invalidate(void *p, size_t n) { __builtin___clear_cache((char *)p, (char *)p + n); }
#endif

#if !defined(__aarch64__)
int main(void) { puts("SKIP: not an AArch64 host"); return 77; }
#else

typedef uint64_t q_t __attribute__((vector_size(16)));   /* binary128 carrier */

/* ---------------------------------------------------------------- ELF ---- */
typedef struct { unsigned char ident[16]; uint16_t type, machine; uint32_t version;
	uint64_t entry, phoff, shoff; uint32_t flags; uint16_t ehsize, phentsize, phnum,
	shentsize, shnum, shstrndx; } Ehdr;
typedef struct { uint32_t type, flags; uint64_t offset, vaddr, paddr, filesz, memsz, align; } Phdr;
typedef struct { uint32_t name, type; uint64_t flags, addr, offset, size; uint32_t link, info;
	uint64_t addralign, entsize; } Shdr;
typedef struct { uint32_t name; unsigned char info, other; uint16_t shndx; uint64_t value, size; } Sym;
typedef struct { uint64_t offset, info; int64_t addend; } Rela;

static void *const *load(const char *path)
{
	int fd = open(path, O_RDONLY);
	struct stat st;
	if (fd < 0 || fstat(fd, &st) != 0) { perror(path); exit(2); }
	unsigned char *f = malloc(st.st_size);
	if (read(fd, f, st.st_size) != st.st_size) { perror("read"); exit(2); }
	close(fd);
	const Ehdr *e = (const Ehdr *)f;
	if (memcmp(e->ident, "\177ELF", 4) || e->ident[4] != 2 || e->ident[5] != 1 ||
	    e->machine != 183 || e->type != 3) {
		fprintf(stderr, "%s: not an ELF64 LE AArch64 ET_DYN\n", path); exit(2);
	}
	const Phdr *ph = (const Phdr *)(f + e->phoff);
	uint64_t span = 0;
	for (int i = 0; i < e->phnum; i++)
		if (ph[i].type == 1 && ph[i].vaddr + ph[i].memsz > span)
			span = ph[i].vaddr + ph[i].memsz;
	long pg = sysconf(_SC_PAGESIZE);
	span = (span + pg - 1) & ~(uint64_t)(pg - 1);
	unsigned char *base = mmap(NULL, span, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
	if (base == MAP_FAILED) { perror("mmap"); exit(2); }
	for (int i = 0; i < e->phnum; i++)
		if (ph[i].type == 1)
			memcpy(base + ph[i].vaddr, f + ph[i].offset, ph[i].filesz);
	const Shdr *sh = (const Shdr *)(f + e->shoff);
	const char *shstr = (const char *)f + sh[e->shstrndx].offset;
	uint64_t table = 0; int nrel = 0;
	for (int i = 0; i < e->shnum; i++) {
		if (sh[i].type == 4) {                               /* SHT_RELA */
			const Rela *r = (const Rela *)(f + sh[i].offset);
			for (uint64_t k = 0; k < sh[i].size / sizeof(Rela); k++) {
				if ((uint32_t)r[k].info != 1027) {           /* R_AARCH64_RELATIVE */
					fprintf(stderr, "%s: relocation type %u in %s\n", path,
						(unsigned)(uint32_t)r[k].info, shstr + sh[i].name);
					exit(2);
				}
				*(uint64_t *)(base + r[k].offset) = (uint64_t)base + r[k].addend;
				nrel++;
			}
		} else if (sh[i].type == 2) {                        /* SHT_SYMTAB */
			const Sym *s = (const Sym *)(f + sh[i].offset);
			const char *str = (const char *)f + sh[sh[i].link].offset;
			for (uint64_t k = 0; k < sh[i].size / sizeof(Sym); k++)
				if (!strcmp(str + s[k].name, "rt_table")) table = s[k].value;
		}
	}
	/* text R-X, data stays RW- (OpenLibm's casin keeps static state): the
	   program must be linked with its data segment on a host page boundary
	   (rt-host.sh links it so; newtos.ld packs data right after text). */
	for (int i = 0; i < e->phnum; i++) {
		if (ph[i].type != 1) continue;
		if (ph[i].flags & 1) {                               /* PF_X */
			uint64_t end = (ph[i].vaddr + ph[i].memsz + pg - 1) & ~(uint64_t)(pg - 1);
			if (mprotect(base, end, PROT_READ | PROT_EXEC) != 0) { perror("mprotect R-X"); exit(2); }
		} else if (ph[i].vaddr & (pg - 1)) {
			fprintf(stderr, "%s: writable segment at 0x%llx is not on a %ld-byte page\n",
				path, (unsigned long long)ph[i].vaddr, pg);
			exit(2);
		}
	}
	sys_icache_invalidate(base, span);
	if (!table) { fprintf(stderr, "%s: no rt_table symbol\n", path); exit(2); }
	printf("loaded %s: %llu bytes at %p, %d relative relocations, rt_table at +0x%llx\n",
	       path, (unsigned long long)span, (void *)base, nrel, (unsigned long long)table);
	free(f);
	return (void *const *)(base + table);
}

/* ------------------------------------------------------------ the table ---- */
struct rt_table {
	q_t (*addtf3)(q_t, q_t), (*subtf3)(q_t, q_t), (*multf3)(q_t, q_t), (*divtf3)(q_t, q_t);
	int (*eqtf2)(q_t, q_t), (*netf2)(q_t, q_t), (*lttf2)(q_t, q_t), (*letf2)(q_t, q_t);
	int (*gttf2)(q_t, q_t), (*getf2)(q_t, q_t), (*unordtf2)(q_t, q_t);
	q_t (*extenddftf2)(double); q_t (*extendsftf2)(float);
	double (*trunctfdf2)(q_t); float (*trunctfsf2)(q_t);
	long long (*fixtfdi)(q_t); unsigned long long (*fixunstfdi)(q_t);
	q_t (*floatditf)(long long); q_t (*floatunditf)(unsigned long long);
	int (*fixtfsi)(q_t); q_t (*floatsitf)(int);
	double complex (*muldc3)(double, double, double, double);
	double complex (*divdc3)(double, double, double, double);
	float complex (*mulsc3)(float, float, float, float);
	float complex (*divsc3)(float, float, float, float);
	struct qc { q_t re, im; } (*multc3)(q_t, q_t, q_t, q_t);
	q_t (*powitf2)(q_t, int);
	q_t (*tf_mad)(q_t, q_t, q_t);
	double complex (*cmul)(double complex, double complex);
	double complex (*cdiv)(double complex, double complex);
	float complex (*cmulf)(float complex, float complex);
	float (*powf)(float, float);
	double (*remainder)(double, double); float (*remainderf)(float, float);
	double (*fdim)(double, double); float (*fdimf)(float, float);
	q_t (*fdiml)(q_t, q_t); float (*nexttowardf)(float, q_t);
	float complex (*cpowf)(float complex, float complex);
	double complex (*casin)(double complex); float complex (*casinf)(float complex);
	double complex (*cacos)(double complex); float complex (*cacosf)(float complex);
	double complex (*casinh)(double complex); float complex (*casinhf)(float complex);
	double complex (*cacosh)(double complex); float complex (*cacoshf)(float complex);
	double complex (*catanh)(double complex); float complex (*catanhf)(float complex);
	uint64_t sentinel;
};
static const struct rt_table *T;

/* -------------------------------------------------------------- helpers ---- */
static long checks, fails;
#define FAIL(...) do { if (fails++ < 40) { printf("  FAIL "); printf(__VA_ARGS__); printf("\n"); } } while (0)

static uint64_t rs = 0x9e3779b97f4a7c15ULL;
static uint64_t rnd(void) { rs ^= rs << 13; rs ^= rs >> 7; rs ^= rs << 17; return rs; }
static uint64_t dbits(double d) { uint64_t u; memcpy(&u, &d, 8); return u; }
static uint32_t fbits(float f) { uint32_t u; memcpy(&u, &f, 4); return u; }
static double bitsd(uint64_t u) { double d; memcpy(&d, &u, 8); return d; }

/* a double from the whole range: random bits, with a bias to specials */
static double rdouble(void)
{
	static const double sp[] = { 0.0, -0.0, 1.0, -1.0, INFINITY, -INFINITY, NAN,
		DBL_MIN, -DBL_MIN, DBL_MAX, -DBL_MAX, 4.9406564584124654e-324, 0.5, 3.0 };
	uint64_t r = rnd();
	if ((r & 15) == 0) return sp[(r >> 4) % (sizeof sp / sizeof sp[0])];
	if ((r & 15) == 1) return (double)(int64_t)(rnd() % 2001) - 1000.0;  /* small ints: exact cases */
	if ((r & 15) == 2) return bitsd(rnd() & 0x800fffffffffffffULL);       /* subnormals */
	return bitsd(rnd());
}

static int same_d(double a, double b) { return (isnan(a) && isnan(b)) || dbits(a) == dbits(b); }
static int same_f(float a, float b) { return (isnan(a) && isnan(b)) || fbits(a) == fbits(b); }
static q_t Q(uint64_t hi, uint64_t lo) { q_t q = { lo, hi }; return q; }   /* little-endian: [0] = low */
static int qeq(q_t a, q_t b) { return a[0] == b[0] && a[1] == b[1]; }

/* ------------------------------------------------------------------- T ---- */
static void test_arith(void)
{
	static const char *nm[4] = { "+", "-", "*", "/" };
	q_t (*fn[4])(q_t, q_t) = { T->addtf3, T->subtf3, T->multf3, T->divtf3 };
	for (int op = 0; op < 4; op++) {
		long bad = 0;
		for (int i = 0; i < 200000; i++) {
			double a = rdouble(), b = rdouble(), want;
			switch (op) { case 0: want = a + b; break; case 1: want = a - b; break;
				      case 2: want = a * b; break; default: want = a / b; }
			double got = T->trunctfdf2(fn[op](T->extenddftf2(a), T->extenddftf2(b)));
			checks++;
			if (!same_d(got, want)) {
				if (bad++ < 5) FAIL("tf %a %s %a -> %a, host %a", a, nm[op], b, got, want);
				else fails++;
			}
		}
		printf("T  binary128 %s: 200000 random pairs, %ld mismatches\n", nm[op], bad);
	}
	/* exact binary128 patterns */
	q_t one = T->extenddftf2(1.0), three = T->extenddftf2(3.0);
	q_t third = T->divtf3(one, three);
	checks++; if (!qeq(third, Q(0x3ffd555555555555ULL, 0x5555555555555555ULL)))
		FAIL("1/3 = %016llx%016llx", (unsigned long long)third[1], (unsigned long long)third[0]);
	q_t ulp = T->multf3(T->extenddftf2(0x1p-60), T->extenddftf2(0x1p-52));   /* 2^-112 */
	q_t onep = T->addtf3(one, ulp);
	checks++; if (!qeq(onep, Q(0x3fff000000000000ULL, 1)))
		FAIL("1+2^-112 = %016llx%016llx", (unsigned long long)onep[1], (unsigned long long)onep[0]);
	q_t half_ulp = T->multf3(ulp, T->extenddftf2(0.5));
	checks++; if (!qeq(T->addtf3(one, half_ulp), one)) FAIL("1+2^-113 does not round to even (1)");
	checks++; if (!qeq(T->addtf3(onep, half_ulp), Q(0x3fff000000000000ULL, 2)))
		FAIL("(1+2^-112)+2^-113 does not round to even (1+2^-111)");
	checks++; if (T->trunctfdf2(T->subtf3(onep, one)) != 0x1p-112) FAIL("(1+2^-112)-1 != 2^-112");
	/* rounding mode from FPCR (aarch64/fp_mode.c) */
	q_t tiny = T->extenddftf2(0x1p-200);
	int saved = fegetround();
	fesetround(FE_UPWARD);
	q_t up = T->addtf3(one, tiny);
	fesetround(FE_DOWNWARD);
	q_t dn = T->subtf3(one, tiny);
	fesetround(FE_TOWARDZERO);
	q_t tz = T->subtf3(one, tiny);
	fesetround(FE_TONEAREST);
	q_t ne = T->addtf3(one, tiny);
	fesetround(saved);
	checks += 4;
	if (!qeq(up, Q(0x3fff000000000000ULL, 1))) FAIL("FE_UPWARD 1+2^-200");
	if (!qeq(dn, Q(0x3ffeffffffffffffULL, ~0ULL))) FAIL("FE_DOWNWARD 1-2^-200");
	if (!qeq(tz, Q(0x3ffeffffffffffffULL, ~0ULL))) FAIL("FE_TOWARDZERO 1-2^-200");
	if (!qeq(ne, one)) FAIL("FE_TONEAREST 1+2^-200");
	printf("T  binary128 exact patterns (1/3, 1+2^-112, ties to even) and FPCR rounding modes checked\n");
}

/* ------------------------------------------------------------------- C ---- */
static void test_compare(void)
{
	long bad = 0;
	for (int i = 0; i < 200000; i++) {
		double a = rdouble(), b = (rnd() & 7) == 0 ? a : rdouble();
		q_t qa = T->extenddftf2(a), qb = T->extenddftf2(b);
		int un = isnan(a) || isnan(b);
		int ok = (T->lttf2(qa, qb) < 0) == (a < b) && (T->letf2(qa, qb) <= 0) == (a <= b) &&
			 (T->gttf2(qa, qb) > 0) == (a > b) && (T->getf2(qa, qb) >= 0) == (a >= b) &&
			 (T->eqtf2(qa, qb) == 0) == (a == b) && (T->netf2(qa, qb) != 0) == (a != b) &&
			 (T->unordtf2(qa, qb) != 0) == un;
		checks++;
		if (!ok) { if (bad++ < 5) FAIL("compare %a, %a", a, b); else fails++; }
	}
	printf("C  binary128 comparisons: 200000 pairs (incl. NaN, +-0, equal), %ld mismatches\n", bad);
}

/* ------------------------------------------------------------------- V ---- */
static void test_convert(void)
{
	long bad = 0;
	for (int i = 0; i < 200000; i++) {
		double d = rdouble();
		float f = (float)rdouble();
		if (rnd() & 1) f = ldexpf((float)(rnd() & 0xffffff), (int)(rnd() % 300) - 170);
		int ok = same_d(T->trunctfdf2(T->extenddftf2(d)), d) &&
			 same_f(T->trunctfsf2(T->extendsftf2(f)), f) &&
			 same_f(T->trunctfsf2(T->extenddftf2(d)), (float)d) &&
			 same_d(T->trunctfdf2(T->extendsftf2(f)), (double)f);
		if (!isnan(d) && fabs(d) < 0x1p62) ok = ok && T->fixtfdi(T->extenddftf2(d)) == (long long)d;
		if (!isnan(d) && d >= 0 && d < 0x1p63) ok = ok && T->fixunstfdi(T->extenddftf2(d)) == (unsigned long long)d;
		if (!isnan(d) && fabs(d) < 0x1p30) ok = ok && T->fixtfsi(T->extenddftf2(d)) == (int)d;
		long long ll = (long long)rnd(); unsigned long long ull = rnd(); int ii = (int)rnd();
		ok = ok && T->trunctfdf2(T->floatditf(ll)) == (double)ll &&
		     T->trunctfdf2(T->floatunditf(ull)) == (double)ull &&
		     T->fixtfdi(T->floatditf(ll)) == ll && T->fixunstfdi(T->floatunditf(ull)) == ull &&
		     T->fixtfsi(T->floatsitf(ii)) == ii;
		checks++;
		if (!ok) { if (bad++ < 5) FAIL("conversion d=%a f=%a ll=%lld", d, f, ll); else fails++; }
	}
	printf("V  conversions float/double/int64/uint64/int32 <-> binary128: 200000 rounds, %ld mismatches\n", bad);
}

/* ------------------------------------------------------------------- X ---- */
static int same_c(double complex a, double complex b) { return same_d(creal(a), creal(b)) && same_d(cimag(a), cimag(b)); }
static int same_cf(float complex a, float complex b) { return same_f(crealf(a), crealf(b)) && same_f(cimagf(a), cimagf(b)); }

static void test_complex(void)
{
	long bad = 0;
	for (int i = 0; i < 100000; i++) {
		double a = rdouble(), b = rdouble(), c = rdouble(), d = rdouble();
		if (rnd() & 1) { a = ldexp((double)(int)(rnd() % 2001) - 1000, (int)(rnd() % 40) - 20);
				 b = ldexp((double)(int)(rnd() % 2001) - 1000, (int)(rnd() % 40) - 20); }
		double complex x = CMPLX(a, b), y = CMPLX(c, d);
		volatile double complex hm = x * y, hd = x / y;      /* host: its own __muldc3/__divdc3 */
		float complex xf = CMPLXF((float)a, (float)b), yf = CMPLXF((float)c, (float)d);
		volatile float complex hmf = xf * yf, hdf = xf / yf;
		int ok = same_c(T->muldc3(a, b, c, d), hm) && same_c(T->divdc3(a, b, c, d), hd) &&
			 same_c(T->cmul(x, y), hm) && same_c(T->cdiv(x, y), hd) &&
			 same_cf(T->mulsc3((float)a, (float)b, (float)c, (float)d), hmf) &&
			 same_cf(T->divsc3((float)a, (float)b, (float)c, (float)d), hdf) &&
			 same_cf(T->cmulf(xf, yf), hmf);
		checks++;
		if (!ok) { if (bad++ < 5) FAIL("complex (%a,%a) (%a,%a)", a, b, c, d); else fails++; }
	}
	printf("X  __muldc3/__divdc3/__mulsc3/__divsc3 + clang's lowering vs host complex * /: 100000 operand sets, %ld mismatches\n", bad);
	/* Annex G: (inf + i nan) * (1 + i 0) is an infinity, not NaN+iNaN */
	double complex r = T->muldc3(INFINITY, NAN, 1.0, 0.0);
	checks++; if (!(isinf(creal(r)) || isinf(cimag(r)))) FAIL("Annex G: inf*1 lost its infinity");
	r = T->divdc3(1.0, 1.0, 0.0, 0.0);
	checks++; if (!(isinf(creal(r)) && isinf(cimag(r)))) FAIL("Annex G: (1+i)/0 not infinite");
	/* binary128 complex multiply, powi, a*b+c on exact values */
	struct qc m = T->multc3(T->extenddftf2(1), T->extenddftf2(2), T->extenddftf2(3), T->extenddftf2(4));
	checks++; if (T->trunctfdf2(m.re) != -5.0 || T->trunctfdf2(m.im) != 10.0) FAIL("__multc3 (1+2i)(3+4i)");
	checks++; if (T->trunctfdf2(T->powitf2(T->extenddftf2(1.5), 10)) != 57.6650390625) FAIL("__powitf2 1.5^10");
	checks++; if (T->trunctfdf2(T->powitf2(T->extenddftf2(2.0), -3)) != 0.125) FAIL("__powitf2 2^-3");
	checks++; if (T->trunctfdf2(T->tf_mad(T->extenddftf2(0x1p52 + 1), T->extenddftf2(0x1p52 - 1),
		T->extenddftf2(1))) != 0x1p104) FAIL("long double a*b+c exact");
	printf("X  Annex G infinities, __multc3, __powitf2, long double a*b+c checked\n");
}

/* ------------------------------------------------------------------- M ---- */
static int64_t ulpdiff_f(float a, float b)
{
	int32_t ia = (int32_t)fbits(a), ib = (int32_t)fbits(b);
	if (ia < 0) ia = INT32_MIN - ia;
	if (ib < 0) ib = INT32_MIN - ib;
	return llabs((int64_t)ia - ib);
}

/* the host system libm, reached by dlsym: the program itself links the host
   build of OpenLibm, which shadows these names */
static struct {
	float (*powf)(float, float);
	double (*remainder)(double, double);
	float (*remainderf)(float, float);
	double (*fdim)(double, double);
	float (*fdimf)(float, float);
	float (*nexttowardf)(float, long double);
} H;

static void host_libm(void)
{
#if defined(__APPLE__)
	void *h = dlopen("/usr/lib/libSystem.B.dylib", RTLD_LAZY | RTLD_LOCAL);
#else
	void *h = dlopen("libm.so.6", RTLD_LAZY | RTLD_LOCAL);
#endif
	if (!h) { fprintf(stderr, "cannot open the host libm: %s\n", dlerror()); exit(2); }
	*(void **)&H.powf = dlsym(h, "powf");
	*(void **)&H.remainder = dlsym(h, "remainder");
	*(void **)&H.remainderf = dlsym(h, "remainderf");
	*(void **)&H.fdim = dlsym(h, "fdim");
	*(void **)&H.fdimf = dlsym(h, "fdimf");
	*(void **)&H.nexttowardf = dlsym(h, "nexttowardf");
	if (!H.powf || !H.remainder || !H.remainderf || !H.fdim || !H.fdimf || !H.nexttowardf) {
		fprintf(stderr, "host libm symbol missing\n"); exit(2);
	}
	if ((void *)H.powf == (void *)powf) {
		fprintf(stderr, "dlsym returned the linked OpenLibm, not the system libm\n"); exit(2);
	}
}

static const double dspecial[] = { 0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, -2.0, 1e-300, -1e-300,
	1e300, -1e300, DBL_MIN, 4.9406564584124654e-324, INFINITY, -INFINITY, NAN };
#define NDS (sizeof dspecial / sizeof dspecial[0])

static void test_libm(void)
{
	long bad = 0, n = 0;
	host_libm();
	/* exact functions: bit-exact against the system libm and host OpenLibm.
	   nexttowardf against the system libm only: OpenLibm reads its `long
	   double` argument through the binary128 union of aarch64/_fpmath.h,
	   which on the host (64-bit long double) reads the wrong bits --
	   nexttowardf(-0.0f, -1e153) is +0x1p-149 there, the target and the
	   system libm give -0x1p-149 (measurements/measurement-2026-10-21-sdk01e) */
	for (int i = 0; i < 200000; i++) {
		double a = rdouble(), b = rdouble();
		float af = (float)a, bf = (float)b;
		if (rnd() & 1) { af = ldexpf((float)(rnd() & 0xffffff), (int)(rnd() % 60) - 30);
				 bf = ldexpf((float)(rnd() & 0xffffff), (int)(rnd() % 60) - 30); }
		double r = T->remainder(a, b), d = T->fdim(a, b);
		float rf = T->remainderf(af, bf), df = T->fdimf(af, bf);
		float nt = T->nexttowardf(af, T->extenddftf2(b));
		int ok = same_d(r, H.remainder(a, b)) && same_d(r, remainder(a, b)) &&
			 same_f(rf, H.remainderf(af, bf)) && same_f(rf, remainderf(af, bf)) &&
			 same_d(d, H.fdim(a, b)) && same_d(d, fdim(a, b)) &&
			 same_f(df, H.fdimf(af, bf)) && same_f(df, fdimf(af, bf)) &&
			 same_f(nt, H.nexttowardf(af, (long double)b));
		checks++; n++;
		if (!ok) { if (bad++ < 5) FAIL("remainder/fdim/nexttowardf a=%a b=%a", a, b); else fails++; }
	}
	printf("M  remainder, remainderf, fdim, fdimf bit-exact vs system libm and host OpenLibm, nexttowardf vs system libm: %ld pairs, %ld mismatches\n", n, bad);
	q_t five = T->extenddftf2(5), three = T->extenddftf2(3);
	checks += 3;
	if (T->trunctfdf2(T->fdiml(five, three)) != 2.0) FAIL("fdiml(5,3)");
	if (dbits(T->trunctfdf2(T->fdiml(three, five))) != 0) FAIL("fdiml(3,5) != +0");
	if (!isnan(T->trunctfdf2(T->fdiml(T->extenddftf2(NAN), five)))) FAIL("fdiml(nan,5)");

	/* powf */
	bad = 0; n = 0; int64_t worst = 0; long bad1 = 0;
	for (int i = 0; i < 200000; i++) {
		float x = ldexpf((float)(rnd() & 0xffffff) / 0x1p24f, (int)(rnd() % 40) - 20);
		float y = ((float)(int32_t)(rnd() % 4001) - 2000.0f) / 100.0f;
		if (rnd() & 1) { x = -x; y = (float)(int)y; }        /* negative base: integral exponent */
		if ((rnd() & 15) == 0) x = (float)dspecial[rnd() % NDS];
		if ((rnd() & 15) == 0) y = (float)dspecial[rnd() % NDS];
		float g = T->powf(x, y), o = powf(x, y), w = H.powf(x, y);
		int64_t u = (isnan(g) && isnan(w)) ? 0 : ulpdiff_f(g, w);
		if (u > worst) worst = u;
		checks += 2; n++;
		if (!same_f(g, o)) { if (bad++ < 5) FAIL("powf(%a, %a) = %a, host OpenLibm %a", x, y, g, o); else fails++; }
		if (u > 1) { if (bad1++ < 5) FAIL("powf(%a, %a) = %a, system libm %a (%lld ulp)", x, y, g, w, (long long)u); else fails++; }
	}
	printf("M  powf: %ld pairs, %ld differ from host OpenLibm, worst %lld ulp from the system libm (%ld beyond 1 ulp)\n",
	       n, bad, (long long)worst, bad1);
	static const float ps[][3] = {          /* x, y, expected (NaN = 'any NaN') */
		{ NAN, 2.0f, NAN }, { 2.0f, NAN, NAN }, { NAN, 0.0f, 1.0f }, { 1.0f, NAN, 1.0f },
		{ -8.0f, 1.0f / 3, NAN }, { 0.0f, -1.0f, INFINITY }, { -2.0f, 3.0f, -8.0f },
		{ INFINITY, -1.0f, 0.0f }, { 2.0f, 0.5f, 1.41421353816986083984375f },
	};
	for (unsigned i = 0; i < sizeof ps / sizeof ps[0]; i++) {
		float g = T->powf(ps[i][0], ps[i][1]);
		checks++;
		if (!same_f(g, ps[i][2])) FAIL("powf(%a, %a) = %a, expected %a", ps[i][0], ps[i][1], g, ps[i][2]);
	}

	/* complex functions: bit-exact against host OpenLibm (same source) */
	bad = 0; n = 0;
	for (int i = 0; i < 50000 + (int)(NDS * NDS); i++) {
		double a, b;
		if (i < 50000) {
			a = ldexp((double)(int64_t)(rnd() % 2000001) - 1000000, -(int)(rnd() % 26));
			b = ldexp((double)(int64_t)(rnd() % 2000001) - 1000000, -(int)(rnd() % 26));
			if ((rnd() & 3) == 0) { a = rdouble(); b = rdouble(); }
		} else {
			a = dspecial[(i - 50000) / NDS]; b = dspecial[(i - 50000) % NDS];
		}
		double complex z = CMPLX(a, b);
		float complex zf = CMPLXF((float)a, (float)b);
		float complex wf = CMPLXF((float)(rnd() % 7) / 2.0f - 1.5f, (float)(rnd() % 7) / 2.0f - 1.5f);
		int ok = same_c(T->casin(z), casin(z)) && same_c(T->cacos(z), cacos(z)) &&
			 same_c(T->casinh(z), casinh(z)) && same_c(T->cacosh(z), cacosh(z)) &&
			 same_c(T->catanh(z), catanh(z)) &&
			 same_cf(T->casinf(zf), casinf(zf)) && same_cf(T->cacosf(zf), cacosf(zf)) &&
			 same_cf(T->casinhf(zf), casinhf(zf)) && same_cf(T->cacoshf(zf), cacoshf(zf)) &&
			 same_cf(T->catanhf(zf), catanhf(zf)) && same_cf(T->cpowf(zf, wf), cpowf(zf, wf));
		checks++; n++;
		if (!ok) { if (bad++ < 5) FAIL("complex libm z=(%a,%a) w=(%a,%a)", a, b, crealf(wf), cimagf(wf)); else fails++; }
	}
	printf("M  casin cacos casinh cacosh catanh (+f), cpowf bit-exact vs host OpenLibm: %ld arguments (incl. %d special pairs), %ld mismatches\n",
	       n, (int)(NDS * NDS), bad);
}

int main(int argc, char **argv)
{
	if (argc != 2) { fprintf(stderr, "usage: rt-host <program.elf>\n"); return 2; }
	setvbuf(stdout, NULL, _IOLBF, 0);       /* lines survive a crash of the loaded code */
	T = (const struct rt_table *)load(argv[1]);
	if (T->sentinel != 0x5254454e44ULL) { fprintf(stderr, "rt_table layout mismatch (no sentinel)\n"); return 2; }
	test_arith();
	test_compare();
	test_convert();
	test_complex();
	test_libm();
	printf("rt-host: %ld checks, %ld failed\n", checks, fails);
	return fails ? 1 : 0;
}
#endif
