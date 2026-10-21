#!/usr/bin/env bash
# double-halfway.sh -- is the strtold halfway-case rounding (ldbl-oracle "info strtold halfway inputs") new with binary128, or
# mintlib's strtod for double as well? 40 random doubles in [2^-63, 2^1), the exact decimal expansion of the midpoint to the next
# double (every digit), strtod() of it on the host via ldbl-host + before-elf.c-style table (the LDBL_KEEP program of
# ldbl-host.sh, which has a "strtod" entry); round-half-even wants the even neighbour. Output ../checks/double-halfway.txt.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; K="$S/keep"
python3 - > "$S/dreq.txt" <<'PY'
from fractions import Fraction as F
import random, struct
r = random.Random(5)
def dval(u): return F(struct.unpack('<d', struct.pack('<Q', u))[0])
for i in range(40):
    u = (r.getrandbits(62) | (0x3c0 << 52)) & ~(1 << 63)
    m = (dval(u) + dval(u + 1)) / 2
    k = m.denominator.bit_length() - 1
    s = str(m.numerator * 5 ** k).rjust(k + 1, '0'); s = s[:-k] + '.' + s[-k:]
    print('strtod\t' + s + '\t%016x' % (u if u % 2 == 0 else u + 1))
PY
cut -f1,2 "$S/dreq.txt" | "$K/ldbl-host" "$K/LDBL-x18.elf" > "$S/drep.txt"
L=$(cut -f2 "$S/dreq.txt" | awk '{l=length($0); if (!mn || l<mn) mn=l; if (l>mx) mx=l} END {print mn ".." mx}')
paste <(cut -f3 "$S/dreq.txt") "$S/drep.txt" | awk -v L="$L" '{n++; if ($1 != $2) bad++} END {printf "%d double midpoints (exact decimal strings of %s characters), %d not rounded to the even neighbour\n", n, L, bad+0}' | tee "$HERE/../checks/double-halfway.txt"
