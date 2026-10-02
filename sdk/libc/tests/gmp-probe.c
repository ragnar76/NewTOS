/*
 * gmp-probe.c -- the mintlib half of sdk/libc/tests/gmp-host.sh (TASK/PORT10).
 *
 * Compiled with the include path and DEFS sdk/libc/build-libc.sh uses for the
 * archive (so <gmp-mparam.h> is the one the archive's stdio/printf_fp.c,
 * stdio/_itoa.c, stdlib/strtod.c and gmp/ see), and linked against the
 * unmodified mintlib members stdlib/fpioconst.c, gmp/dbl2mpn.c and
 * gmp/mpn-mul_1.c. It hands the harness plain numbers -- unsigned __int128
 * values reassembled from limbs in base 2^(8*sizeof(mp_limb_t)) -- so the
 * harness can compare them with exact arithmetic without seeing any mintlib
 * header.
 */
#include <gmp-mparam.h>
#include "gmp.h"
#include "gmp-impl.h"
#include "fpioconst.h"

typedef unsigned __int128 u128;

int
probe_limb_bits (void)
{
  return BITS_PER_MP_LIMB;
}

int
probe_limb_bytes (void)
{
  return BYTES_PER_MP_LIMB;
}

int
probe_limb_sizeof (void)
{
  return (int) sizeof (mp_limb_t);
}

/* Reassemble n limbs (least significant first) the way every mpn routine
   reads them. Returns 0 when the number does not fit in 128 bits. */
static int
limbs_to_u128 (const mp_limb_t *p, mp_size_t n, u128 *out)
{
  u128 v = 0;
  const int w = (int) (8 * sizeof (mp_limb_t));
  mp_size_t i;

  for (i = n; i-- > 0;)
    {
      if (v >> (128 - w))
	return 0;
      v = (v << w) | (u128) p[i];
    }
  *out = v;
  return 1;
}

/* 10^(2^i) as __printf_fp reads it: _fpioconst_pow10[i] names arraysize
   limbs at __tens[arrayoff], the first _FPIO_CONST_OFFSET of which are a
   zero pad (stdio/printf_fp.c: `__tens + powers->arrayoff +
   _FPIO_CONST_OFFSET`, size `powers->arraysize - _FPIO_CONST_OFFSET`). */
int
probe_pow10 (int i, u128 *value)
{
  const struct mp_power *pw = &_fpioconst_pow10[i];
  mp_size_t k;

  for (k = 0; k < _FPIO_CONST_OFFSET; k++)
    if (__tens[pw->arrayoff + k] != 0)
      return -1;
  return limbs_to_u128 (&__tens[pw->arrayoff + _FPIO_CONST_OFFSET],
			pw->arraysize - _FPIO_CONST_OFFSET, value);
}

/* __mpn_extract_double: the significand as an integer and the unbiased
   exponent, value == mant * 2^(expt - (DBL_MANT_DIG - 1)). */
int
probe_extract (double d, u128 *mant, int *expt, int *neg)
{
  mp_limb_t buf[4] = { 0, 0, 0, 0 };
  mp_size_t n = __mpn_extract_double (buf, 4, expt, neg, d);

  return limbs_to_u128 (buf, n, mant) ? (int) n : -1;
}

/* mpn_mul_1 over a two-limb operand, product reassembled. */
int
probe_mul_1 (u128 a, unsigned long m, u128 *product)
{
  const int w = (int) (8 * sizeof (mp_limb_t));
  mp_limb_t src[2], dst[3];
  mp_limb_t carry;

  src[0] = (mp_limb_t) a;
  src[1] = (mp_limb_t) (a >> w);
  if (w < 64 && (a >> (2 * w)) != 0)
    return 0;
  carry = mpn_mul_1 (dst, src, 2, (mp_limb_t) m);
  dst[2] = carry;
  return limbs_to_u128 (dst, 3, product);
}
