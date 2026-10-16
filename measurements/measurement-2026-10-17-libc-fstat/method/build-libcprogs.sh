#!/usr/bin/env bash
# build-libcprogs.sh <outdir> -- LIBC-FSTAT: relink EVERY libc program of the series cards
# (/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards/*.img: the 55 card files holding mintlib's _main, grep "UNIXMODE",
# + STDERRO = old crt0) against the CURRENT sdk/libc/libc.a (LIBC-FSTAT do_fstat.c), with the recipes of
# measurement-2026-10-13-mem05-acceptance/method (build-s2-tests.sh, build-fs11.sh, build-tty.sh, build-stderrq.sh,
# build-mem05.sh, build-mem05b.sh, build-extra.sh; copied here with tests/). Into <outdir> (scratch):
#   probes, P10KID/P10TEST (sdk/libc/build-libc.sh), STDOUTDB = copy of STDOUTDBG, oldcrt0/STDERRP.NAP + STDERRO.NAP (old crt0),
#   APITEST.NCP (build-apitest.sh <outdir>/APITEST.NCP -- the shared tree's sdk/apps/apitest/APITEST.NCP is left alone),
#   apps/ QED.NAP, DESKTOP.NAP (TeraDesk), DOOM.NAP: sdk/apps/build-{qed,teradesk,doom}.sh build into their gitignored
#   sdk/apps/*/build/ dirs and are copied; gemtest/chaintest/coremark relinked too (not on the cards).
# Not relinked: HELLOOLD.NAP (deliberately linked against the pre-MEM05 libc.a, a control) -- the card copy stays.
# Non-libc programs (sdk/build.sh, sdk/crt0.S) are untouched by a libc change and stay as on the cards.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"
B="${1:?outdir}"; mkdir -p "$B/apps" "$B/oldcrt0" "$B/logs"; B="$(cd "$B" && pwd)"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1791979200}"
cd "$WS"
bash sdk/libc/build-libc.sh sdk/examples/port10kid.c "$B/P10KID.NCP" > "$B/logs/p10kid.log" 2>&1 && echo "built P10KID.NCP" || { echo "FAIL P10KID"; exit 1; }
bash sdk/libc/build-libc.sh sdk/examples/port10.c "$B/P10TEST.NCP" > "$B/logs/p10test.log" 2>&1 && echo "built P10TEST.NCP" || { echo "FAIL P10TEST"; exit 1; }
S2_TESTS="envrn e9mem fseekcon stdoutdbg stderrp" bash "$HERE/build-s2-tests.sh" "$B" || { echo "FAIL s2"; exit 1; }
bash "$HERE/build-fs11.sh" "$B" || { echo "FAIL fs11"; exit 1; }
cp "$B/STDOUTDBG.NAP" "$B/STDOUTDB.NAP" && echo "copied STDOUTDBG.NAP -> STDOUTDB.NAP"
bash "$HERE/build-tty.sh" "$B" || { echo "FAIL tty"; exit 1; }
bash "$HERE/build-stderrq.sh" "$B" || { echo "FAIL stderrq"; exit 1; }
CRT0="$HERE/crt0-before-libc-main.S" bash "$HERE/build-stderrq.sh" "$B/oldcrt0" || { echo "FAIL stderro"; exit 1; }
mv "$B/oldcrt0/STDERRQ.NAP" "$B/STDERRO.NAP"
# old-crt0 STDERRP (with spawn): build-stderrq.sh with the other build line (as build-extra.sh does)
sed -e 's/^build stderrp STDERRQ.NAP -DSTDERRP_NOSPAWN$/build stderrp STDERRP.NAP/' "$HERE/build-stderrq.sh" > "$HERE/.build-stderrp-old.sh"
CRT0="$HERE/crt0-before-libc-main.S" bash "$HERE/.build-stderrp-old.sh" "$B/oldcrt0" || { echo "FAIL oldcrt0 stderrp"; exit 1; }
rm -f "$HERE/.build-stderrp-old.sh"
bash "$HERE/build-mem05.sh" "$B" || { echo "FAIL mem05"; exit 1; }
bash "$HERE/build-mem05b.sh" "$B" || { echo "FAIL mem05b"; exit 1; }
bash "$HERE/build-fstatp.sh" "$B" || { echo "FAIL fstatp"; exit 1; }
bash sdk/apps/apitest/build-apitest.sh "$B/APITEST.NCP" > "$B/logs/apitest.log" 2>&1 && echo "built APITEST.NCP" || { echo "FAIL apitest"; tail -20 "$B/logs/apitest.log"; exit 1; }
for s in apps/build-qed.sh apps/build-teradesk.sh apps/build-doom.sh apps/gemtest/build-gemtest.sh apps/chaintest/build-chaintest.sh apps/build-coremark.sh; do
  bash "sdk/$s" > "$B/logs/$(basename $s .sh).log" 2>&1 && echo "built via $s" || { echo "FAIL $s"; tail -20 "$B/logs/$(basename $s .sh).log"; exit 1; }
done
cp sdk/apps/qed/build/QED.NAP sdk/apps/teradesk/build/dist/DESKTOP.NAP sdk/apps/doom/build/DOOM.NAP "$B/apps/"
for l in "$B"/logs/*.log; do echo "$(basename "$l"): $(grep -ciE '(^|[^-])warning[: ]' "$l") warning lines, $(grep -ciE '(^|[ :])error[: ]' "$l") error lines"; done
(cd "$B" && shasum -a 256 *.NAP *.NCP *.NAC oldcrt0/*.NAP apps/*.NAP | sort -k2 > SHA256SUMS)
shasum -a 256 sdk/libc/libc.a sdk/gemlib/gemlib.a | sed "s|$WS/||"
echo done
