#!/usr/bin/env python3
"""ldbl-oracle.py -- oracle of the long double host test (TASK/SDK01 J).

    ldbl-oracle.py <ldbl-host> <program.elf> [seed]

Generates requests for ldbl-host (which runs the target's libc/libm machine
code from <program.elf> on the host CPU), sends them, and judges every reply
against exact rational arithmetic (fractions.Fraction) -- an implementation
independent of mintlib's GMP code and of OpenLibm:

  P  printf %.<p>Le / %.<p>Lf / %La / %Lg of binary128 values: the exact
     decimal (hex) digits, rounded half-to-even (mintlib's printf_fp and
     printf_fphex round that way), byte for byte.
  R  round trip: strtold(snprintf("%.35Le", x)) == x bit for bit (36
     significant digits identify every binary128).
  S  strtold / sscanf("%Lf"/"%Lg"/"%Le") of decimal strings (random digits,
     exponents over the whole range incl. subnormals): correctly rounded to
     nearest-even, bit for bit, and the consumed length. Exact midpoints
     between neighbours and their 1-unit-in-the-last-digit neighbours are
     INFORMATIONAL: mintlib's strtod (glibc before 2.17, BZ #3479) gets some
     of them wrong for double too; only the consumed length is checked.
  Q  qecvt / qecvt_r / qfcvt_r / qgcvt digits, decpt, sign, against the exact
     value for the arguments where mintlib's algorithm is exact (efgcvt_r.c:
     ecvt scales by an exact power of ten <= 10^48 with one binary128
     rounding, then prints %.*Lf; the oracle uses the same scaled value).
  M  libm, bit-exact where the operation is exact (scalbnl, scalblnl, ldexpl,
     frexpl, ilogbl, logbl, modfl, fabsl, copysignl, floorl, ceill, truncl,
     roundl, rintl, nextafterl, fmodl, remainderl, remquol, fmal; and
     scalbln/scalblnf, which link since this package); sqrtl within 1 ulp
     (correct rounding informational: OpenLibm decides the last bit with
     FE_INEXACT, which compiler-rt's __divtf3 does not raise); within 2 ulp of
     a 130-digit decimal.Decimal reference for the transcendental ones (expl
     exp2l expm1l logl log2l log10l log1pl powl cbrtl hypotl sinl cosl tanl
     atanl sinhl coshl tanhl), max error printed.
  A  binary128 + - * / (compiler-rt) on full 113-bit operands, bit for bit.
  C  isnanl/isinfl/finitel/signbitl on every class.
  D  double printf/strtod/sscanf still exact, inf/nan printed as such (the
     libc now also has a real long double; the double paths must not move).

"info" lines count the informational cases; they never fail the run.
LDBL_DUMP=<file> keeps the request lines, LDBL_FAILS_PER_GROUP=<n> prints
more failures per group (default 3).
Exit 0 = all checks passed, 1 = failures, 2 = harness error.
"""
import decimal
import math
import os
import random
import subprocess
import sys
from fractions import Fraction as F

sys.set_int_max_str_digits(0)          # %Lf of 1e4932 has 4933 digits

BIAS, FB = 16383, 112          # binary128: exponent bias, fraction bits
EMAX = 0x7fff


# ---------------------------------------------------------------- binary128 --
def q_hex(bits):
    return "%016x:%016x" % (bits >> 64, bits & ((1 << 64) - 1))


def q_parse(s):
    hi, lo = s.split(":")
    return (int(hi, 16) << 64) | int(lo, 16)


def q_fields(bits):
    return bits >> 127, (bits >> FB) & EMAX, bits & ((1 << FB) - 1)


def q_isnan(b):
    s, e, f = q_fields(b)
    return e == EMAX and f != 0


def q_isinf(b):
    s, e, f = q_fields(b)
    return e == EMAX and f == 0


def q_val(bits):
    """exact value of a finite binary128 (Fraction), sign separately"""
    s, e, f = q_fields(bits)
    assert e != EMAX
    if e == 0:
        v = F(f, 1 << (FB + BIAS - 1))
    else:
        v = F((1 << FB) | f) * (F(2) ** (e - BIAS - FB))
    return -v if s else v


def rne(r):
    """round a non-negative Fraction to the nearest integer, ties to even"""
    n = r.numerator // r.denominator
    rem = r - n
    if rem > F(1, 2) or (rem == F(1, 2) and n & 1):
        n += 1
    return n


def ilog2(a):
    """floor(log2(a)) for a positive Fraction"""
    e = a.numerator.bit_length() - a.denominator.bit_length()
    if F(2) ** e > a:
        e -= 1
    elif F(2) ** (e + 1) <= a:
        e += 1
    return e


def q_from(x, neg=None):
    """nearest binary128 (ties to even) of a Fraction; overflow -> inf"""
    if neg is None:
        neg = x < 0
    a = abs(x)
    sign = (1 << 127) if neg else 0
    if a == 0:
        return sign
    e = ilog2(a)
    if e < 1 - BIAS:                                   # subnormal range
        m = rne(a * (F(2) ** (BIAS - 1 + FB)))
        return sign | m                                # m == 2^112 -> min normal
    m = rne(a * (F(2) ** (FB - e)))
    if m == 1 << (FB + 1):
        m >>= 1
        e += 1
    if e > BIAS:
        return sign | (EMAX << FB)
    return sign | ((e + BIAS) << FB) | (m - (1 << FB))


def ulp_of(x):
    a = abs(x)
    if a == 0:
        return F(2) ** (1 - BIAS - FB)
    e = max(ilog2(a), 1 - BIAS)
    return F(2) ** (e - FB)


def rand_q(rng, emin=0, emax=EMAX - 1):
    s = rng.getrandbits(1)
    e = rng.randint(emin, emax)
    f = rng.getrandbits(FB)
    return (s << 127) | (e << FB) | f


def dbl_val(u):
    s = u >> 63
    e = (u >> 52) & 0x7ff
    f = u & ((1 << 52) - 1)
    v = F(f, 1 << 1074) if e == 0 else F((1 << 52) | f) * F(2) ** (e - 1075)
    return -v if s else v


def dbl_from(x):
    """nearest binary64 of a Fraction (ties to even), via the exact float() of
    Python, which is correctly rounded for Fraction"""
    import struct
    try:
        d = float(x)
    except OverflowError:
        d = math.inf if x > 0 else -math.inf
    return struct.unpack("<Q", struct.pack("<d", d))[0]


# ------------------------------------------------------------ formatting --
def pow10(k):
    return F(10) ** k


def dec_exp10(a):
    """E with 10^E <= a < 10^(E+1), a > 0"""
    e = int(math.floor((a.numerator.bit_length() - a.denominator.bit_length()) * 0.30102999566398))
    while pow10(e) > a:
        e -= 1
    while pow10(e + 1) <= a:
        e += 1
    return e


def fmt_e(bits, p, upper=False):
    s, e, f = q_fields(bits)
    sg = "-" if s else ""
    if e == EMAX:
        return ("NAN" if upper else "nan") if f else sg + ("INF" if upper else "inf")
    a = abs(q_val(bits))
    if a == 0:
        E, digs = 0, "0" * (p + 1)
    else:
        E = dec_exp10(a)
        n = rne(a * pow10(p - E))
        if n == 10 ** (p + 1):
            n //= 10
            E += 1
        digs = str(n)
    mant = digs[0] + ("." + digs[1:] if p > 0 else "")
    return "%s%s%s%s%02d" % (sg, mant, "E" if upper else "e", "-" if E < 0 else "+", abs(E))


def fmt_f(bits, p):
    s, e, f = q_fields(bits)
    sg = "-" if s else ""
    if e == EMAX:
        return "nan" if f else sg + "inf"
    a = abs(q_val(bits))
    n = rne(a * pow10(p))
    digs = str(n).rjust(p + 1, "0")
    ip, fp = digs[:-p] if p else digs, digs[-p:] if p else ""
    return sg + ip + ("." + fp if p else "")


def fmt_a(bits):
    """%La: binary128, shortest exact hex significand (mintlib printf_fphex
    binary128 branch: implicit digit in front, 28 hex digits, trailing zeros
    dropped, denormals 0x0.<frac>p-16382)"""
    s, e, f = q_fields(bits)
    sg = "-" if s else ""
    if e == EMAX:
        return "nan" if f else sg + "inf"
    if e == 0 and f == 0:
        return sg + "0x0p+0"
    h = ("%028x" % f).rstrip("0")
    lead = "0" if e == 0 else "1"
    ex = 1 - BIAS if e == 0 else e - BIAS
    return "%s0x%s%sp%s%d" % (sg, lead, ("." + h) if h else "", "-" if ex < 0 else "+", abs(ex))


def fmt_g(bits, p):
    """%.<p>Lg (C99 7.19.6.1): style e if X < -4 or X >= P, else f with
    P-1-X digits; trailing zeros and a trailing point removed"""
    if p == 0:
        p = 1
    s, e, f = q_fields(bits)
    if e == EMAX:
        return fmt_e(bits, 0)
    es = fmt_e(bits, p - 1)
    mant, ex = es.split("e")
    X = int(ex)
    if X < -4 or X >= p:
        m = mant.rstrip("0").rstrip(".") if "." in mant else mant
        return m + "e" + ex
    r = fmt_f(bits, p - 1 - X)
    if "." in r:
        r = r.rstrip("0").rstrip(".")
    return r


# ----------------------------------------------------------- the session --
class Session:
    def __init__(self, host, elf):
        self.host, self.elf = host, elf
        self.reqs, self.judges = [], []

    def ask(self, line, judge):
        assert "\n" not in line
        self.reqs.append(line)
        self.judges.append(judge)

    def run(self):
        data = "".join(r + "\n" for r in self.reqs)
        dump = os.environ.get("LDBL_DUMP")
        if dump:                                   # debugging: keep the requests
            with open(dump, "w") as fh:
                fh.write(data)
        p = subprocess.run([self.host, self.elf], input=data, capture_output=True, text=True, encoding="latin-1")
        if p.returncode == 77:
            print("SKIP:", p.stdout.strip())
            sys.exit(77)
        out = p.stdout.split("\n")
        if p.returncode != 0 or len(out) < len(self.reqs):
            print("harness error: ldbl-host rc=%d, %d of %d replies; stderr: %s"
                  % (p.returncode, len(out) - 1, len(self.reqs), p.stderr.strip()[:400]))
            sys.exit(2)
        for req, judge, rep in zip(self.reqs, self.judges, out):
            judge(req, rep)


stats = {}          # group -> [checks, fails]
info = {}           # informational group -> [checks, mismatches]: known upstream limits, never a failure
fail_lines = []


def note(group, ok):
    c = info.setdefault(group, [0, 0])
    c[0] += 1
    c[1] += 0 if ok else 1


def check(group, ok, msg):
    c = stats.setdefault(group, [0, 0])
    c[0] += 1
    if not ok:
        c[1] += 1
        if c[1] <= int(os.environ.get("LDBL_FAILS_PER_GROUP", "3")):
            fail_lines.append("  FAIL %s: %s" % (group, msg))


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    host, elf = sys.argv[1], sys.argv[2]
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 20261021
    rng = random.Random(seed)
    ss = Session(host, elf)

    specials = [0, 1 << 127, (BIAS << FB), (1 << 127) | (BIAS << FB),     # +-0 +-1
                EMAX << FB, (1 << 127) | (EMAX << FB),                   # +-inf
                (EMAX << FB) | (1 << 111), (EMAX << FB) | 1,             # qNaN, sNaN
                1, (1 << FB) - 1, 1 << FB,                               # min sub, max sub, min normal
                ((EMAX - 1) << FB) | ((1 << FB) - 1),                    # max finite
                q_from(F(1, 3)), q_from(F(2, 3)), q_from(F(1, 10)), q_from(F(5, 2)),
                q_from(F(1, 8)), q_from(F(123456789, 1000)), q_from(F(10) ** 4932),
                q_from(F(1, 10) ** 4931), q_from(F(1, 10) ** 4960)]
    vals = specials + [rand_q(rng) for _ in range(1500)] \
        + [rand_q(rng, BIAS - 200, BIAS + 200) for _ in range(1500)] \
        + [rng.getrandbits(FB) | (rng.getrandbits(1) << 127) for _ in range(200)]   # subnormals

    # ---- P: printf -------------------------------------------------------
    def jp(expect, group):
        def j(req, rep):
            rc, _, got = rep.partition("\t")
            check(group, got == expect and rc == str(len(expect)),
                  "%s -> %r (rc %s), want %r" % (req, got, rc, expect))
        return j
    for b in vals:
        ss.ask("snprintf\t%%.35Le\t%s" % q_hex(b), jp(fmt_e(b, 35), "P %.35Le"))
        ss.ask("snprintf\t%%La\t%s" % q_hex(b), jp(fmt_a(b), "P %La"))
    for b in vals[:400]:
        p = rng.choice([0, 1, 3, 6, 10, 20, 40, 60])
        ss.ask("snprintf\t%%.%dLE\t%s" % (p, q_hex(b)), jp(fmt_e(b, p, True), "P %.pLE"))
        g = rng.choice([1, 6, 17, 30, 36])
        ss.ask("snprintf\t%%.%dLg\t%s" % (g, q_hex(b)), jp(fmt_g(b, g), "P %.pLg"))
    for b in specials + [rand_q(rng, BIAS - 120, BIAS + 120) for _ in range(600)]:
        p = rng.choice([0, 1, 2, 6, 15, 30, 50])
        ss.ask("snprintf\t%%.%dLf\t%s" % (p, q_hex(b)), jp(fmt_f(b, p), "P %.pLf"))
    ss.ask("snprintf\t%%Lf\t%s" % q_hex(q_from(F(1, 3))), jp("0.333333", "P %Lf"))
    ss.ask("snprintf\t%%Lf\t%s" % q_hex(q_from(F(10) ** 4932)), jp(fmt_f(q_from(F(10) ** 4932), 6), "P %Lf"))
    ss.ask("snprintf\t%%.0Lf\t%s" % q_hex(q_from(F(5, 2))), jp("2", "P %Lf"))      # tie -> even
    ss.ask("snprintf\t%%.0Lf\t%s" % q_hex(q_from(F(7, 2))), jp("4", "P %Lf"))

    # ---- R: round trip -----------------------------------------------------
    def jr(b):
        def j(req, rep):
            got, _, n = rep.partition("\t")
            s = req.split("\t")[1]
            check("R strtold(%.35Le)", q_parse(got) == b and n == str(len(s)),
                  "%s -> %s (%s), want %s" % (s, got, n, q_hex(b)))
        return j
    for b in vals:
        if q_isnan(b):
            continue
        ss.ask("strtold\t%s" % fmt_e(b, 35), jr(b))

    # ---- S: strtold / sscanf of decimal strings -----------------------------
    def js(s, consumed, group="S strtold", halfway=False):
        want = q_hex(q_from(parse_dec(s[:consumed]))) if consumed else q_hex(0)
        def j(req, rep):
            got, _, n = rep.partition("\t")
            if halfway:
                # mintlib's strtod (glibc before 2.17, BZ #3479) converts only
                # (MANT_DIG - bits + 2) / 3 + 1 fraction digits exactly and
                # keeps the rest as a sticky bit (stdlib/strtod.c "For the
                # fractional part we need not process too many digits"), so
                # an input at or within one last-digit unit of a halfway
                # point between two neighbours can round to the wrong one --
                # for double as well (measured). Informational.
                check(group + " consumed", n == str(consumed), "%r -> %s" % (s, n))
                note("strtold halfway inputs, exact rounding (upstream BZ#3479-class)", got == want)
                return
            check(group, got == want and n == str(consumed), "%r -> %s (%s), want %s (%d)"
                  % (s, got, n, want, consumed))
        return j
    strs, halfway = [], set()
    for _ in range(1500):
        nd = rng.randint(1, 40)
        digs = "".join(rng.choice("0123456789") for _ in range(nd)).lstrip("0") or "0"
        ex = rng.randint(-4970, 4935)
        strs.append("%s%s.%se%d" % (rng.choice(["", "-", "+"]), digs[0], digs[1:], ex))
    for _ in range(300):                                    # exact midpoints and neighbours
        b = rand_q(rng, BIAS - 300, BIAS + 300) & ~(1 << 127)   # strings <= ~420 digits
        lo, hi = q_val(b), q_val(b + 1)
        mid = (lo + hi) / 2
        ms = exact_dec(mid)
        for t in (ms, bump_dec(ms, +1), bump_dec(ms, -1)):
            strs.append(t)
            halfway.add(t)
    for b in [1, 2, (1 << FB) - 1, 1 << FB]:                 # subnormal / boundary midpoints
        mid = (q_val(b) + q_val(b + 1)) / 2
        strs.append(exact_dec(mid))
        halfway.add(strs[-1])
    strs += ["0", "-0", "1e4933", "1.18973149535723176508575932662800702e4932",
             "1.18973149535723176508575932662800703e4932", "3.3621031431120935062626778173217526e-4932",
             "6.4751751194380251109244389582276466e-4966", "3.2e-4966", "3.3e-4966", "1e-4970",
             "0.1", "123.456", "  42.5x", "1e", "-.5", "1.e2", "infinity", "-INF", "nan"]
    for s in strs:
        consumed = dec_prefix_len(s)
        if s.lstrip().lower().lstrip("+-").startswith(("inf", "nan")):
            t = s.lstrip()
            neg = t.startswith("-")
            core = t.lstrip("+-").lower()
            if core.startswith("nan"):
                def jn(req, rep, s=s):
                    got, _, n = rep.partition("\t")
                    check("S strtold", q_isnan(q_parse(got)) and n == str(len(s)), "%r -> %s %s" % (s, got, n))
                ss.ask("strtold\t%s" % s, jn)
            else:
                ln = len(s) - len(t) + (len(t) - len(core)) + (8 if core.startswith("infinity") else 3)
                want = q_hex((EMAX << FB) | ((1 << 127) if neg else 0))
                def ji(req, rep, s=s, want=want, ln=ln, core=core):
                    got, _, n = rep.partition("\t")
                    if core.startswith("infinity"):
                        # strtod.c tests "inf" before "infinity" (|| order),
                        # so the 8-character form is never matched -- double
                        # too. Value checked, end pointer informational.
                        check("S strtold", got == want, "%r -> %s want %s" % (s, got, want))
                        note('strtold consumes all of "infinity" (upstream: "inf" matched first)', n == str(ln))
                        return
                    check("S strtold", got == want and n == str(ln), "%r -> %s %s want %s %d" % (s, got, n, want, ln))
                ss.ask("strtold\t%s" % s, ji)
            continue
        ss.ask("strtold\t%s" % s, js(s, consumed, halfway=s in halfway))
    for s in strs[:600]:
        if "x" in s or s.strip() in ("1e",) or "n" in s.lower():
            continue
        c = dec_prefix_len(s)
        want = q_hex(q_from(parse_dec(s[:c])))
        fm = rng.choice(["%Lf", "%Lg", "%Le", "%LG"])
        def jsc(req, rep, want=want, s=s):
            rc, _, got = rep.partition("\t")
            check("S sscanf %L", rc == "1" and got == want, "%r -> %s %s want %s" % (s, rc, got, want))
        ss.ask("sscanf\t%s\t%s" % (fm, s), jsc)

    # ---- Q: q*cvt ------------------------------------------------------------
    qv = specials[:4] + specials[12:18] + [rand_q(rng, BIAS - 100, BIAS + 100) for _ in range(400)]
    for b in qv:
        nd = rng.randint(1, 36)
        ss.ask("qecvt\t%s\t%d" % (q_hex(b), nd), jq_ecvt(b, nd, False))
        ss.ask("qecvt_r\t%s\t%d" % (q_hex(b), nd), jq_ecvt(b, nd, True))
        fd = rng.randint(0, 30)
        ss.ask("qfcvt_r\t%s\t%d" % (q_hex(b), fd), jq_fcvt(b, fd))
        gd = rng.randint(1, 36)
        want = fmt_g(b, gd)
        def jg(req, rep, want=want):
            check("Q qgcvt", rep == want, "%s -> %r, want %r" % (req, rep, want))
        ss.ask("qgcvt\t%s\t%d" % (q_hex(b), gd), jg)

    # ---- A: binary128 arithmetic (compiler-rt), full-width operands ----------
    for _ in range(3000):
        a_, c_ = rand_q(rng, BIAS - 200, BIAS + 200), rand_q(rng, BIAS - 200, BIAS + 200)
        if rng.random() < .3:
            c_ = rand_q(rng, ((a_ >> FB) & EMAX) - 3, ((a_ >> FB) & EMAX) + 3)
        x, y = q_val(a_), q_val(c_)
        for op, v in (("add", x + y), ("sub", x - y), ("mul", x * y), ("div", x / y)):
            want = q_from(v) if v != 0 else 0
            def ja(req, rep, want=want, op=op):
                check("A " + op, q_parse(rep) == want, "%s -> %s want %s" % (req, rep, q_hex(want)))
            ss.ask("%s\t%s\t%s" % (op, q_hex(a_), q_hex(c_)), ja)

    # ---- C: classification --------------------------------------------------
    for b in specials + vals[:300]:
        nan, inf = q_isnan(b), q_isinf(b)
        sg = b >> 127
        # isinfl: any non-zero for an infinity -- with -lm the __isinfl of
        # OpenLibm's s_isinf.c (1) comes first, libc's mintlib one returns -1
        # for -inf; C99 asks only for non-zero.
        exp = {"isnanl": str(int(nan)), "isinfl": ("nonzero" if inf else "0"),
               "finitel": str(int(not nan and not inf)), "signbitl": str(int(sg))}
        for fn, w in exp.items():
            def jc(req, rep, w=w):
                ok = (rep not in ("0", "")) if w == "nonzero" else rep == w
                check("C classify", ok, "%s -> %s, want %s" % (req, rep, w))
            ss.ask("%s\t%s" % (fn, q_hex(b)), jc)

    # ---- M: libm ------------------------------------------------------------
    libm_tests(ss, rng, vals, specials)

    # ---- D: double paths unchanged -------------------------------------------
    for u, want in ((0x7ff0000000000000, "inf"), (0xfff0000000000000, "-inf"), (0x7ff8000000000000, "nan"),
                    (0xfff8000000000001, "nan"), (0x000000007ff00000, fmt_e(q_from(dbl_val(0x7ff00000)), 16))):
        for fm in ("%.16e", "%f"):
            w = want if not want[0].isdigit() or fm == "%.16e" else "0.000000"
            def jdi(req, rep, w=w):
                rc, _, got = rep.partition("\t")
                check("D inf/nan", got == w, "%s -> %r want %r" % (req, got, w))
            ss.ask("snprintfd\t%s\t%016x" % (fm, u), jdi)
    for _ in range(800):
        u = rng.getrandbits(64)
        if ((u >> 52) & 0x7ff) == 0x7ff:
            continue
        v = dbl_val(u)
        b = q_from(v)   # exact (binary64 is a subset)
        def jd(req, rep, want=fmt_e(b, 16)):
            rc, _, got = rep.partition("\t")
            check("D %.16e", got == want, "%s -> %r want %r" % (req, got, want))
        ss.ask("snprintfd\t%%.16e\t%016x" % u, jd)
        s = fmt_e(b, 16)
        def jdd(req, rep, u=u):
            got, _, n = rep.partition("\t")
            check("D strtod", int(got, 16) == u, "%s -> %s want %016x" % (req, got, u))
        ss.ask("strtod\t%s" % s, jdd)
        def jds(req, rep, u=u):
            rc, _, got = rep.partition("\t")
            check("D sscanf %lf", rc == "1" and int(got, 16) == u, "%s -> %s" % (req, rep))
        ss.ask("sscanfd\t%%lf\t%s" % s, jds)

    ss.run()

    total = sum(c[0] for c in stats.values())
    bad = sum(c[1] for c in stats.values())
    for g in sorted(stats):
        print("%-24s %7d checks  %5d failed" % (g, stats[g][0], stats[g][1]))
    for m in sorted(ulp_max):
        print("  ulp  %-10s max %.3f ulp over %d arguments (bound %.1f)" % (m, ulp_max[m][0], ulp_max[m][1], ULP_BOUND[m]))
    for g in sorted(info):
        print("info %-70s %6d cases, %5d differ" % (g, info[g][0], info[g][1]))
    for l in fail_lines:
        print(l)
    print("ldbl-oracle: %d checks, %d failed (seed %d)" % (total, bad, seed))
    return 1 if bad else 0


# ------------------------------------------------------------- decimal input --
def dec_prefix_len(s):
    """length of the longest prefix of s that strtod accepts as a decimal
    number (leading white space, sign, digits, '.', digits, exponent)"""
    i = 0
    while i < len(s) and s[i] in " \t":
        i += 1
    j = i
    if j < len(s) and s[j] in "+-":
        j += 1
    d0 = j
    while j < len(s) and s[j].isdigit():
        j += 1
    nd = j - d0
    if j < len(s) and s[j] == ".":
        k = j + 1
        while k < len(s) and s[k].isdigit():
            k += 1
        nd += k - j - 1
        if k - j - 1 > 0 or nd > 0:
            j = k
    if nd == 0:
        return 0
    if j < len(s) and s[j] in "eE":
        k = j + 1
        if k < len(s) and s[k] in "+-":
            k += 1
        if k < len(s) and s[k].isdigit():
            while k < len(s) and s[k].isdigit():
                k += 1
            j = k
    return j


def parse_dec(s):
    s = s.strip()
    neg = s.startswith("-")
    s = s.lstrip("+-")
    if "e" in s.lower():
        m, e = s.lower().split("e")
        e = int(e)
    else:
        m, e = s, 0
    if "." in m:
        ip, fp = m.split(".")
    else:
        ip, fp = m, ""
    n = int((ip + fp) or "0")
    v = F(n) * F(10) ** (e - len(fp))
    if neg:
        v = -v if v != 0 else v
    return v if not (neg and v == 0) else NegZero()


class NegZero(F):
    """-0 as a Fraction that q_from() turns into the negative zero"""
    def __new__(cls):
        return super().__new__(cls, 0)


_q_from = q_from


def q_from(x, neg=None):   # noqa: F811 -- wraps the one above for -0
    if isinstance(x, NegZero):
        return 1 << 127
    return _q_from(x, neg)


def exact_dec(x):
    """the exact decimal expansion of a positive dyadic Fraction"""
    d = x.denominator
    k = d.bit_length() - 1
    assert d == 1 << k
    n = x.numerator * 5 ** k                      # x = n / 10^k
    s = str(n).rjust(k + 1, "0")
    return (s[:-k] + "." + s[-k:]) if k else s + ".0"


def bump_dec(s, delta):
    """add delta units in the last place of a decimal string (no exponent)"""
    ip, fp = s.split(".")
    n = int(ip + fp) + delta
    t = str(n).rjust(len(fp) + 1, "0")
    return t[:-len(fp)] + "." + t[-len(fp):]


# ---------------------------------------------------------------- q*cvt --
def jq_fcvt(b, nd):
    """qfcvt_r(value, ndigit): %.*Lf with MIN(ndigit, 36) fraction digits,
    leading zeros stripped (decpt adjusted), the digits without the point
    (efgcvt_r.c fcvt_r)."""
    def j(req, rep):
        rc, digs, dp, sg = rep.split("\t")
        if q_isnan(b) or q_isinf(b):
            check("Q qfcvt_r", rc == "0" and dp == "0", "%s -> %r" % (req, rep))
            return
        v = q_val(b)
        a = abs(v)
        p = min(nd, 36)
        s = fmt_f(b & ~(1 << 127), p)
        ip, _, fp = s.partition(".")
        allds = ip + fp
        decpt = len(ip)
        if decpt == 1 and ip == "0" and a != 0 and p > 0:   # fcvt_r strips only after a point
            t = fp
            decpt = 0
            while t.startswith("0"):
                t = t[1:]
                decpt -= 1
            allds = t
        want = (allds, str(decpt), str(b >> 127))
        check("Q qfcvt_r", rc == "0" and (digs, dp, sg) == want, "%s -> %r, want %r" % (req, rep, want))
    return j


def jq_ecvt(b, nd, reentrant):
    """qecvt(value, ndigit): efgcvt_r.c ecvt_r -- scale into [1, 10) by an
    exact power of ten f (one binary128 rounding of value*f or value/f),
    then fcvt_r(scaled, MIN(ndigit, 36) - 1)."""
    group = "Q qecvt_r" if reentrant else "Q qecvt"
    def j(req, rep):
        parts = rep.split("\t")
        if reentrant:
            rc, parts = parts[0], parts[1:]
            if rc != "0":
                check(group, False, "%s -> %r" % (req, rep))
                return
        digs, dp, sg = parts
        if q_isnan(b) or q_isinf(b):
            check(group, True, "")
            return
        v = q_val(b)
        a = abs(v)
        expo = 0
        sc = b & ~(1 << 127)
        if a != 0:
            if a < 1:
                f = F(1)
                while True:
                    f *= 10
                    expo -= 1
                    if q_val(q_from(a * f)) >= 1:     # do-while (d * f < 1.0), binary128 product
                        break
                sc = q_from(a * f)
            elif a >= 10:
                f = F(1)
                while True:
                    f *= 10
                    expo += 1
                    if not (a >= q_val(q_from(f * 10))):
                        break
                sc = q_from(a / f)
            if (a < 1 or a >= 10) and f > F(10) ** 48:
                check(group, True, "")              # f not exact in binary128: not judged
                return
        n = min(nd, 36) - 1
        s = fmt_f(sc, n)
        ip, _, fp = s.partition(".")
        allds = ip + fp
        decpt = len(ip)
        av = q_val(sc)
        if decpt == 1 and ip == "0" and av != 0:
            t = fp
            decpt = 0
            while t.startswith("0"):
                t = t[1:]
                decpt -= 1
            allds = t
        want = (allds, str(decpt + expo), str(b >> 127))
        check(group, (digs, dp, sg) == want, "%s -> %r, want %r" % (req, rep, want))
    return j


# ------------------------------------------------------------------ libm --
# Not correctly rounded functions: the bound is 2 ulp for all of them (the
# measured maxima are printed; OpenLibm's ld128 code is glibc's/FreeBSD's
# ldbl-128 code, which documents no tighter bound).
ULP_BOUND = dict.fromkeys(("expl", "exp2l", "expm1l", "logl", "log2l", "log10l", "log1pl",
                           "powl", "cbrtl", "hypotl", "sinl", "cosl", "tanl", "atanl",
                           "sinhl", "coshl", "tanhl"), 2.0)
ulp_max = {}

D = decimal.Context(prec=130, Emax=999999, Emin=-999999)
decimal.setcontext(D)      # unary -, abs() and comparisons round with the CURRENT context


def dq(x):
    return D.divide(decimal.Decimal(x.numerator), decimal.Decimal(x.denominator))


def fd(d):
    return F(d)


_PI = None


def dpi():
    global _PI
    if _PI is None:
        # Machin: pi = 16 atan(1/5) - 4 atan(1/239)
        def at_inv(n):
            x = D.divide(1, n)
            x2 = D.multiply(x, x)
            s, t, k = x, x, 1
            while True:
                t = D.multiply(t, x2)
                k += 2
                term = D.divide(t, k)
                if term == 0 or abs(term) < decimal.Decimal(10) ** -140:
                    break
                s = D.subtract(s, term) if (k // 2) % 2 else D.add(s, term)
            return s
        _PI = D.subtract(D.multiply(16, at_inv(5)), D.multiply(4, at_inv(239)))
    return _PI


def dsin_cos(x):
    """sin, cos of a Decimal by reduction mod pi/2 and Taylor series"""
    pi = dpi()
    h = D.divide(pi, 2)
    k = int(D.to_integral_value(D.divide(x, h)))
    r = D.subtract(x, D.multiply(k, h))
    r2 = D.multiply(r, r)
    s, t, n = r, r, 1
    while True:
        t = D.divide(D.multiply(t, -r2), (n + 1) * (n + 2))
        n += 2
        if t == 0 or abs(t) < decimal.Decimal(10) ** -135:
            break
        s = D.add(s, t)
    c, t, n = decimal.Decimal(1), decimal.Decimal(1), 0
    while True:
        t = D.divide(D.multiply(t, -r2), (n + 1) * (n + 2))
        n += 2
        if t == 0 or abs(t) < decimal.Decimal(10) ** -135:
            break
        c = D.add(c, t)
    k &= 3
    return [(s, c), (c, -s), (-s, -c), (-c, s)][k]


def datan(x):
    neg = x < 0
    x = abs(x)
    inv = x > 1
    if inv:
        x = D.divide(1, x)
    n2 = 0
    while x > decimal.Decimal("0.1"):        # atan(x) = 2 atan(x / (1 + sqrt(1 + x^2)))
        x = D.divide(x, D.add(1, D.sqrt(D.add(1, D.multiply(x, x)))))
        n2 += 1
    x2 = D.multiply(x, x)
    s, t, k = x, x, 1
    while True:
        t = D.multiply(t, -x2)
        k += 2
        term = D.divide(t, k)
        if term == 0 or abs(term) < decimal.Decimal(10) ** -135:
            break
        s = D.add(s, term)
    s = D.multiply(s, 2 ** n2)
    if inv:
        s = D.subtract(D.divide(dpi(), 2), s)
    return -s if neg else s


def ref(fn, args):
    """120+-digit reference of a transcendental function (Decimal)"""
    x = dq(args[0])
    if fn == "expl":
        return D.exp(x)
    if fn == "exp2l":
        return D.exp(D.multiply(x, D.ln(decimal.Decimal(2))))
    if fn == "expm1l":
        return D.subtract(D.exp(x), 1)
    if fn == "logl":
        return D.ln(x)
    if fn == "log2l":
        return D.divide(D.ln(x), D.ln(decimal.Decimal(2)))
    if fn == "log10l":
        return D.log10(x)
    if fn == "log1pl":
        return D.ln(D.add(1, x))
    if fn == "powl":
        return D.exp(D.multiply(dq(args[1]), D.ln(x)))
    if fn == "cbrtl":
        neg = x < 0
        a = abs(x)
        y = D.exp(D.divide(D.ln(a), 3)) if a else decimal.Decimal(0)
        if a:
            for _ in range(3):
                y = D.divide(D.add(D.multiply(2, y), D.divide(a, D.multiply(y, y))), 3)
        return -y if neg else y
    if fn == "hypotl":
        return D.sqrt(dq(args[0] * args[0] + args[1] * args[1]))
    if fn in ("sinl", "cosl", "tanl"):
        s, c = dsin_cos(x)
        return {"sinl": s, "cosl": c, "tanl": D.divide(s, c)}[fn]
    if fn == "atanl":
        return datan(x)
    if fn in ("sinhl", "coshl", "tanhl"):
        e, ei = D.exp(x), D.exp(-x)
        # |x| >= 2^-60 here: the cancellation in e - ei costs <= 19 of 130 digits
        sh, ch = D.divide(D.subtract(e, ei), 2), D.divide(D.add(e, ei), 2)
        return {"sinhl": sh, "coshl": ch, "tanhl": D.divide(sh, ch)}[fn]
    raise KeyError(fn)


def libm_tests(ss, rng, vals, specials):
    one = BIAS << FB

    def jexact(name, want_bits, extra=None):
        def j(req, rep):
            fs = rep.split("\t")
            got = q_parse(fs[0])
            ok = got == want_bits or (q_isnan(got) and q_isnan(want_bits))
            if extra is not None:
                ok = ok and fs[1:] == extra
            check("M " + name, ok, "%s -> %s, want %s %s" % (req, rep, q_hex(want_bits), extra or ""))
        return j

    finite = [b for b in vals if not q_isnan(b) and not q_isinf(b)]
    # scalbnl / ldexpl / scalblnl: x * 2^n, exactly rounded (subnormal results round)
    for b in finite[:1200]:
        n = rng.choice([0, 1, -1, 10, -10, 100, -100, 5000, -5000, 16383, -16383, 20000, -20000, 30000, -30000])
        want = q_from(q_val(b) * F(2) ** n, b >> 127)
        ss.ask("scalbnl\t%s\t%d" % (q_hex(b), n), jexact("scalbnl", want))
        ss.ask("ldexpl\t%s\t%d" % (q_hex(b), n), jexact("ldexpl", want))
        ss.ask("scalblnl\t%s\t%d" % (q_hex(b), n), jexact("scalblnl", want))
    for big in (1 << 40, -(1 << 40)):
        want = q_from(F(2) ** (99999 if big > 0 else -99999))
        ss.ask("scalblnl\t%s\t%d" % (q_hex(one), big), jexact("scalblnl", want))
    # scalbln / scalblnf (double, float): link since SDK01 J; exact against Python
    import struct
    for _ in range(400):
        u = rng.getrandbits(64)
        if ((u >> 52) & 0x7ff) == 0x7ff:
            continue
        n = rng.choice([0, 3, -3, 600, -600, 1100, -1100, 1 << 33, -(1 << 33)])
        x = dbl_val(u) * F(2) ** max(min(n, 5000), -5000)   # |n| > 5000: inf or 0 either way
        want = dbl_from(x) if x != 0 else (u & (1 << 63))
        def jd(req, rep, want=want):
            check("M scalbln", int(rep, 16) == want, "%s -> %s want %016x" % (req, rep, want))
        ss.ask("scalbln\t%016x\t%d" % (u, n), jd)
        f32 = struct.unpack("<f", struct.pack("<I", rng.getrandbits(32) & 0xbf7fffff))[0]
        fu = struct.unpack("<I", struct.pack("<f", f32))[0]
        n2 = rng.choice([0, 3, -3, 60, -60, 200, -200, 1 << 33])
        xv = F(f32) * F(2) ** max(min(n2, 500), -500)
        w32 = f32_from(xv, fu >> 31)
        def jf(req, rep, w=w32):
            check("M scalblnf", int(rep, 16) == w, "%s -> %s want %08x" % (req, rep, w))
        ss.ask("scalblnf\t%08x\t%d" % (fu, n2), jf)

    for b in finite[:800] + [rand_q(rng, BIAS - 130, BIAS + 130) for _ in range(800)]:
        x = q_val(b)
        sg = b >> 127
        a = abs(x)
        # frexpl / ilogbl / logbl
        if a == 0:
            ss.ask("frexpl\t%s" % q_hex(b), jexact("frexpl", b, ["0"]))
        else:
            e = ilog2(a) + 1
            ss.ask("frexpl\t%s" % q_hex(b), jexact("frexpl", q_from(x / F(2) ** e), [str(e)]))
            def ji(req, rep, e=e):
                check("M ilogbl", rep == str(e - 1), "%s -> %s want %d" % (req, rep, e - 1))
            ss.ask("ilogbl\t%s" % q_hex(b), ji)
            ss.ask("logbl\t%s" % q_hex(b), jexact("logbl", q_from(F(e - 1))))
        # modfl
        ip = F(int(x))   # truncation toward zero
        fr = x - ip
        def jm(req, rep, ip=ip, fr=fr, sg=sg):
            g1, g2 = rep.split("\t")
            ok = q_parse(g1) == q_from(fr, sg) and q_parse(g2) == q_from(ip, sg)
            check("M modfl", ok, "%s -> %s" % (req, rep))
        ss.ask("modfl\t%s" % q_hex(b), jm)
        ss.ask("fabsl\t%s" % q_hex(b), jexact("fabsl", b & ~(1 << 127)))
        ss.ask("floorl\t%s" % q_hex(b), jexact("floorl", q_from(F(math.floor(x)), sg)))
        ss.ask("ceill\t%s" % q_hex(b), jexact("ceill", q_from(F(math.ceil(x)), sg)))
        ss.ask("truncl\t%s" % q_hex(b), jexact("truncl", q_from(ip, sg)))
        rnd = F(math.floor(a + F(1, 2)))                     # roundl: half away from zero
        ss.ask("roundl\t%s" % q_hex(b), jexact("roundl", q_from(-rnd if sg else rnd, sg)))
        r2 = F(rne(a))
        ss.ask("rintl\t%s" % q_hex(b), jexact("rintl", q_from(-r2 if sg else r2, sg)))
        if a != 0 and not sg:
            # OpenLibm's sqrtl (FreeBSD e_sqrtl.c) decides the last bit with
            # fetestexcept(FE_INEXACT) after a division; compiler-rt's soft-
            # float raises FE_INEXACT only in add/sub (fp_add_impl.inc), not
            # in __divtf3, so the result is faithful but not always correctly
            # rounded. Checked: within 1 ulp; correct rounding informational.
            want = q_from_sqrt(a)
            def jsq(req, rep, want=want):
                got = q_parse(rep)
                check("M sqrtl (<= 1 ulp)", abs(got - want) <= 1, "%s -> %s want %s" % (req, rep, q_hex(want)))
                note("sqrtl correctly rounded (upstream: no FE_INEXACT from __divtf3)", got == want)
            ss.ask("sqrtl\t%s" % q_hex(b), jsq)
        o = finite[rng.randrange(len(finite))]
        ss.ask("copysignl\t%s\t%s" % (q_hex(b), q_hex(o)), jexact("copysignl", (b & ~(1 << 127)) | (o & (1 << 127))))
        # nextafterl toward +inf / -inf
        ss.ask("nextafterl\t%s\t%s" % (q_hex(b), q_hex(EMAX << FB)), jexact("nextafterl", next_up(b)))
        ss.ask("nextafterl\t%s\t%s" % (q_hex(b), q_hex((1 << 127) | (EMAX << FB))), jexact("nextafterl", next_up(b ^ (1 << 127)) ^ (1 << 127)))
        # fmodl / remainderl / remquol with a second operand of similar scale
        c = rand_q(rng, max(1, ((b >> FB) & EMAX) - 40), min(EMAX - 1, ((b >> FB) & EMAX) + 3))
        y = q_val(c)
        if y != 0:
            qt = x / y
            n_tr = int(qt)
            fm = x - n_tr * y
            ss.ask("fmodl\t%s\t%s" % (q_hex(b), q_hex(c)), jexact("fmodl", q_from(fm, sg)))
            n_ne = rne(abs(qt)) * (1 if qt >= 0 else -1)
            rm = x - n_ne * y
            ss.ask("remainderl\t%s\t%s" % (q_hex(b), q_hex(c)), jexact("remainderl", q_from(rm, sg if rm == 0 else None)))
            def jrq(req, rep, rm=rm, n_ne=n_ne, sg=sg):
                g1, g2 = rep.split("\t")
                qv = int(g2)
                ok = q_parse(g1) == q_from(rm, sg if rm == 0 else None) and \
                    (abs(qv) & 7) == (abs(n_ne) & 7) and (n_ne == 0 or (qv < 0) == (n_ne < 0))
                check("M remquol", ok, "%s -> %s want %s quo %d" % (req, rep, q_hex(q_from(rm)), n_ne))
            ss.ask("remquol\t%s\t%s" % (q_hex(b), q_hex(c)), jrq)
        # fmal
        c2 = rand_q(rng, BIAS - 130, BIAS + 130)
        d2 = rand_q(rng, BIAS - 130, BIAS + 130)
        prod = q_val(c2) * q_val(d2) + x
        ss.ask("fmal\t%s\t%s\t%s" % (q_hex(c2), q_hex(d2), q_hex(b)), jexact("fmal", q_from(prod)))

    # transcendental: ulp error against the Decimal reference
    def jt(fn, args):
        r = fd(ref(fn, args))
        want = q_from(r)
        def j(req, rep):
            got = q_parse(rep)
            if q_isnan(got) or q_isinf(got) or q_isinf(want):
                check("M " + fn, got == want, "%s -> %s want %s" % (req, rep, q_hex(want)))
                return
            err = float(abs(q_val(got) - r) / ulp_of(r))
            m = ulp_max.setdefault(fn, [0.0, 0])
            m[0] = max(m[0], err)
            m[1] += 1
            check("M " + fn, err <= ULP_BOUND[fn], "%s -> %s (%.3f ulp) want %s" % (req, rep, err, q_hex(want)))
        return j

    def rq(lo, hi, pos=False):
        x = F(rng.uniform(lo, hi)) + F(rng.getrandbits(60), 1 << 120) * (1 if rng.random() < .5 else -1)
        if pos:
            x = abs(x)
        return q_from(x)
    N = 150
    plan = {"expl": (-11000, 11000), "exp2l": (-16000, 16000), "expm1l": (-80, 80),
            "logl": None, "log2l": None, "log10l": None, "log1pl": (-0.99, 1e6),
            "cbrtl": None, "sinl": (-1e4, 1e4), "cosl": (-1e4, 1e4), "tanl": (-1e4, 1e4),
            "atanl": (-1e6, 1e6), "sinhl": (-100, 100), "coshl": (-100, 100), "tanhl": (-30, 30)}
    for fn, rngv in plan.items():
        for i in range(N):
            if rngv is None:                     # positive, whole range
                b = rand_q(rng, 1, EMAX - 1) & ~(1 << 127)
                if fn == "cbrtl" and rng.random() < .5:
                    b |= 1 << 127
            else:
                lo, hi = rngv
                if i % 3 == 0:
                    b = rq(lo / 1000.0, hi / 1000.0)
                elif i % 3 == 1:
                    b = rq(lo, hi)
                else:
                    b = rand_q(rng, BIAS - 60, BIAS - 1) | (rng.getrandbits(1) << 127)  # small |x|
                    if fn == "log1pl" and q_val(b) <= -1:
                        continue
            if fn in ("expl", "exp2l") and q_val(b) == 0:
                continue
            ss.ask("%s\t%s" % (fn, q_hex(b)), jt(fn, [q_val(b)]))
    for i in range(N):
        x = rand_q(rng, BIAS - 20, BIAS + 20) & ~(1 << 127)
        y = rq(-200, 200)
        if abs(q_val(y) * math.log(max(float(q_val(x)), 1e-300))) > 11000:
            continue
        ss.ask("powl\t%s\t%s" % (q_hex(x), q_hex(y)), jt("powl", [q_val(x), q_val(y)]))
        a, c = rand_q(rng), rand_q(rng)
        if q_isnan(a) or q_isinf(a) or q_isnan(c) or q_isinf(c) or abs(((a >> FB) & EMAX) - ((c >> FB) & EMAX)) > 200:
            a, c = rand_q(rng, BIAS - 100, BIAS + 100), rand_q(rng, BIAS - 100, BIAS + 100)
        ss.ask("hypotl\t%s\t%s" % (q_hex(a), q_hex(c)), jt("hypotl", [q_val(a), q_val(c)]))
    # exact special values
    for fn, arg, want in [("expl", 0, one), ("logl", one, 0), ("log2l", q_from(F(2) ** 1000), q_from(F(1000))),
                          ("log10l", q_from(F(10) ** 30), q_from(F(30))), ("exp2l", q_from(F(-16494)), 1),
                          ("cbrtl", q_from(F(27)), q_from(F(3))), ("sqrtl", q_from(F(2) ** 200), q_from(F(2) ** 100)),
                          ("expl", q_from(F(12000)), EMAX << FB), ("logl", 0, (1 << 127) | (EMAX << FB))]:
        ss.ask("%s\t%s" % (fn, q_hex(arg)), jexact(fn + " special", want))


def f32_from(x, neg):
    """nearest binary32 of a Fraction, ties to even, with overflow/underflow"""
    a = abs(x)
    s = (1 << 31) if neg else 0
    if a == 0:
        return s
    e = ilog2(a)
    if e < -126:
        m = rne(a * F(2) ** 149)
        return s | m
    m = rne(a * F(2) ** (23 - e))
    if m == 1 << 24:
        m >>= 1
        e += 1
    if e > 127:
        return s | 0x7f800000
    return s | ((e + 127) << 23) | (m - (1 << 23))


def q_from_sqrt(a):
    """correctly rounded sqrt of a positive dyadic Fraction"""
    # scale to an integer with plenty of bits, integer sqrt, then decide
    e = ilog2(a)
    shift = 2 * (FB + 8) - e
    shift += shift & 1
    n = a * F(2) ** shift
    num = n.numerator // n.denominator
    exact = (n.denominator == 1)
    r = math.isqrt(num)
    # r <= sqrt(n) < r+1; value = (r + frac) / 2^(shift/2)
    rem_nonzero = (r * r != num) or not exact
    base = F(r, 1) / F(2) ** (shift // 2)
    if not rem_nonzero:
        return q_from(base)
    # r has >= FB+8 bits: the tie decision needs only a sticky bit
    return q_from(base + F(1, 4) / F(2) ** (shift // 2) / (1 << 20))


def next_up(b):
    """nextafterl(x, +inf) by bit manipulation of the pattern"""
    if b == 1 << 127:          # -0 -> +min subnormal
        return 1
    if b >> 127:
        return b - 1
    if b == EMAX << FB:
        return b
    return b + 1


if __name__ == "__main__":
    sys.exit(main())
