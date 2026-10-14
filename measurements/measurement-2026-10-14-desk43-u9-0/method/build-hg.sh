#!/usr/bin/env bash
# build-hg.sh -- DESK43 U9-0: the programs and the injector kernels of the HELLOGEM runs.
#  1. HELLOGEM.NAP and its panic-test twin HGPANIC.NAP, both as `cargo xtask build-programs --board rpi4` builds a
#     program (programs/build.sh ... -- --features board-rpi4; HGPANIC adds panic-test) -> ../bin (gitignored, sha256
#     in SHA256SUMS). HELLOGEM.NAP must equal U8b's bin/HELLOGEM-rpi4.NAP (5b4f283c..).
#  2. two scratch kernels, injector applied with tools/inject-arm.py (T10, U9-0d) to a scratch copy only:
#       kernel8-inj-z.img   launch:HELLOGEM=ret.script launch:HGPANIC=ret.script            (the #Z cards)
#       kernel8-inj-f1.img  desktop=f1.script launch:HELLOGEM=ret.script launch:HGPANIC=ret.script (the F1 cards)
#     = U8b's inj-u6a.py arming (A: F1 at desktop-up +200/+210, B: Return at the program start +1600/+1610), but
#     B now names the program and A is a separate kernel instead of inj-u6a.py's "A only if no program ran first".
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; B="$M/bin"; W=/tmp/m5j/u90
export SOURCE_DATE_EPOCH=1791979200
"$WS/programs/build.sh" hello-gem "$B/HELLOGEM.NAP" -- --features board-rpi4 > "$W/build-hellogem.log" 2>&1
"$WS/programs/build.sh" hello-gem "$B/HGPANIC.NAP" -- --features board-rpi4,panic-test > "$W/build-hgpanic.log" 2>&1
[ -n "${HG_ONLY:-}" ] && { (cd "$B" && shasum -a 256 HELLOGEM.NAP HGPANIC.NAP); exit 0; }
for k in z f1; do
  sh "$HERE/make-scratch.sh" "$W/tree-inj-$k"
  if [ $k = z ]; then A=(); else A=(desktop="$HERE/f1.script"); fi
  python3 "$WS/tools/inject-arm.py" "$W/tree-inj-$k" --tag u6a ${A[@]+"${A[@]}"} \
    launch:HELLOGEM="$HERE/ret.script" launch:HGPANIC="$HERE/ret.script"
  (cd "$W/tree-inj-$k" && cargo xtask build --board rpi4 --release) > "$W/build-inj-$k.log" 2>&1 \
    || { tail -30 "$W/build-inj-$k.log"; exit 1; }
  grep -E "generated [0-9]+ warning|^error" "$W/build-inj-$k.log" || true
  cp "$W/tree-inj-$k/kernel8.img" "$B/kernel8-inj-$k.img"
done
(cd "$B" && shasum -a 256 HELLOGEM.NAP HGPANIC.NAP kernel8-inj-z.img kernel8-inj-f1.img)
