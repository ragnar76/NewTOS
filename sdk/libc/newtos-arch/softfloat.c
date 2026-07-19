/*
 * softfloat.c -- NewTOS soft-float compiler-runtime helper: __extenddftf2.
 *
 * The AArch64 procedure call standard makes C `long double` an IEEE-754
 * binary128 (quad). clang emits a call to the compiler-rt builtin
 * __extenddftf2 whenever it has to widen a `double` result to `long double`.
 * The freestanding NewTOS SDK links no compiler-rt, so this builtin is
 * unresolved.
 *
 * QED pulls it through options.c -> sscanf -> mintlib vfscanf -> strtold: under
 * -D__NO_LONG_DOUBLE_MATH (set by build-libc.sh so the float printf/scanf
 * engine uses hardware double), mintlib's strtold returns strtod()'s `double`
 * as a `long double`, and that return widening is __extenddftf2.
 *
 * This is the real compiler-rt fp_extend algorithm (LLVM
 * compiler-rt/lib/builtins/fp_extend_impl.inc) for binary64 -> binary128,
 * implemented exactly -- a correct conversion for every input class (normal,
 * subnormal, zero, inf, NaN), not a stub. It is the same category of arch/ABI
 * runtime support as newtos-arch/memops.c (which supplies the memcpy/memset the
 * compiler emits).
 */

typedef unsigned long long u64;
typedef unsigned __int128  u128;

/* binary64 (source) */
#define SRC_SIG_BITS   52
#define SRC_EXP_BITS   11
#define SRC_BIAS       1023
#define SRC_INF_EXP    0x7ffULL
#define SRC_SIG_MASK   ((1ULL << SRC_SIG_BITS) - 1)
#define SRC_MIN_NORMAL (1ULL << SRC_SIG_BITS)

/* binary128 (destination) */
#define DST_SIG_BITS   112
#define DST_BIAS       16383
#define DST_MIN_NORMAL ((u128)1 << DST_SIG_BITS)

long double
__extenddftf2(double a)
{
	u64 aRep;
	__builtin_memcpy(&aRep, &a, sizeof aRep);

	const u64 sign = aRep >> (SRC_SIG_BITS + SRC_EXP_BITS);	/* bit 63 */
	const u64 aExp = (aRep >> SRC_SIG_BITS) & SRC_INF_EXP;
	const u64 aSig = aRep & SRC_SIG_MASK;

	u128 absResult;

	if (aExp == SRC_INF_EXP) {
		/* infinity or NaN: exponent all-ones, significand left-aligned. */
		absResult = (u128)0x7fffULL << DST_SIG_BITS;
		absResult |= (u128)aSig << (DST_SIG_BITS - SRC_SIG_BITS);
	} else if (aExp) {
		/* normal number. */
		u64 dstExp = aExp + (DST_BIAS - SRC_BIAS);
		absResult = (u128)dstExp << DST_SIG_BITS;
		absResult |= (u128)aSig << (DST_SIG_BITS - SRC_SIG_BITS);
	} else if (aSig) {
		/* subnormal: renormalize and clear the implicit leading bit. */
		const int scale = __builtin_clzll(aSig) - __builtin_clzll(SRC_MIN_NORMAL);
		absResult = (u128)aSig << (DST_SIG_BITS - SRC_SIG_BITS + scale);
		absResult ^= DST_MIN_NORMAL;
		const int dstExp = (DST_BIAS - SRC_BIAS) - scale + 1;
		absResult |= (u128)(u64)dstExp << DST_SIG_BITS;
	} else {
		/* zero. */
		absResult = 0;
	}

	u128 result = absResult | ((u128)sign << 127);

	long double r;
	__builtin_memcpy(&r, &result, sizeof r);
	return r;
}
