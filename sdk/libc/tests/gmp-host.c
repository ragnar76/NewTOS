/*
 * gmp-host.c -- host harness for the limb width of the ported mintlib's GMP
 * core (TASK/PORT10: printf("%f") printed wrong digits and could abort in
 * __printf_fp).
 *
 * mintlib's float formatting (stdio/printf_fp.c), its integer formatter
 * (stdio/_itoa.c) and strtod (stdlib/strtod.c) all run on GMP's mpn layer:
 * numbers are arrays of mp_limb_t, and BITS_PER_MP_LIMB (gmp-mparam.h)
 * selects which code and which power-of-ten table (stdlib/fpioconst.c) they
 * use. The pristine header said 32 while mp_limb_t (stdlib/gmp.h, `unsigned
 * long`) is 64 bits on LP64. This harness checks, on an LP64 little-endian
 * host (the target's data model), that
 *
 *   - BITS_PER_MP_LIMB / BYTES_PER_MP_LIMB agree with sizeof(mp_limb_t);
 *   - each 10^(2^i) in __tens, read as __printf_fp reads it, IS 10^(2^i);
 *   - __mpn_extract_double returns a significand/exponent pair that
 *     reconstructs the double exactly (normals and denormals);
 *   - mpn_mul_1 multiplies a two-limb number exactly.
 *
 * The mintlib side is gmp-probe.c (built with the archive's include path);
 * this file uses only the host's headers. Build + run: gmp-host.sh.
 * Exit status: number of failed checks; 77 = host is not LP64 little-endian.
 */
#include <stdio.h>
#include <math.h>
#include <float.h>

typedef unsigned __int128 u128;

extern int probe_limb_bits (void);
extern int probe_limb_bytes (void);
extern int probe_limb_sizeof (void);
extern int probe_pow10 (int i, u128 *value);
extern int probe_extract (double d, u128 *mant, int *expt, int *neg);
extern int probe_mul_1 (u128 a, unsigned long m, u128 *product);

static int failures, checks;

static void
check (int ok, const char *what)
{
  checks++;
  if (!ok)
    failures++;
  printf ("%s  %s\n", ok ? "PASS" : "FAIL", what);
}

static void
hex128 (char *buf, size_t n, u128 v)
{
  snprintf (buf, n, "0x%016llx%016llx", (unsigned long long) (v >> 64),
	    (unsigned long long) v);
}

int
main (void)
{
  char line[200], a[40], b[40];
  int i;

  if (sizeof (long) != 8 || *(const unsigned char *) &(int){ 1 } != 1)
    {
      printf ("gmp-host: skipped -- host is not LP64 little-endian\n");
      return 77;
    }

  snprintf (line, sizeof line,
	    "limb width: BITS_PER_MP_LIMB=%d BYTES_PER_MP_LIMB=%d sizeof(mp_limb_t)=%d",
	    probe_limb_bits (), probe_limb_bytes (), probe_limb_sizeof ());
  check (probe_limb_bits () == 8 * probe_limb_sizeof ()
	 && probe_limb_bytes () == probe_limb_sizeof (), line);

  /* 10^(2^i) for i = 0..5 fits in 128 bits (10^32 < 2^107). */
  {
    u128 want = 10;
    for (i = 0; i <= 5; i++)
      {
	u128 got = 0;
	int r = probe_pow10 (i, &got);
	hex128 (a, sizeof a, got);
	hex128 (b, sizeof b, want);
	snprintf (line, sizeof line, "__tens 10^(2^%d): got %s want %s%s", i,
		  a, b, r < 0 ? " (pad limbs not zero)" : r == 0 ? " (> 128 bits)" : "");
	check (r == 1 && got == want, line);
	want = want * want;
      }
  }

  /* __mpn_extract_double: value == mant * 2^(expt - 52). */
  {
    static const double vals[] = {
      1.5, 0.1, 3.14159, 79.123, 1e10, 1e-5, 123456789.0, 1e100, -2.5,
      DBL_MAX, DBL_MIN, DBL_MIN / 8.0, 4.9406564584124654e-324
    };
    for (i = 0; i < (int) (sizeof vals / sizeof vals[0]); i++)
      {
	u128 mant = 0;
	int expt = 0, neg = 0;
	int n = probe_extract (vals[i], &mant, &expt, &neg);
	double back = ldexp ((double) mant, expt - (DBL_MANT_DIG - 1));
	if (neg)
	  back = -back;
	snprintf (line, sizeof line,
		  "__mpn_extract_double(%.17g): %d limb(s), rebuilt %.17g", vals[i],
		  n, back);
	check (n > 0 && back == vals[i], line);
      }
  }

  /* mpn_mul_1 on a two-limb operand (a value above 2^64). */
  {
    u128 x = ((u128) 0x0123456789abcdefULL << 64) | 0xfedcba9876543210ULL;
    u128 p = 0;
    int r = probe_mul_1 (x, 10, &p);
    hex128 (a, sizeof a, p);
    hex128 (b, sizeof b, x * 10);
    snprintf (line, sizeof line, "mpn_mul_1(x, 10): got %s want %s", a, b);
    check (r == 1 && p == x * 10, line);
  }

  printf ("gmp-host: %d of %d checks failed\n", failures, checks);
  return failures;
}
