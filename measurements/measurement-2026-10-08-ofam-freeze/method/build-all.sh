#!/usr/bin/env bash
# COPIED from measurement-2026-10-06-vforkt-fix/method/build-all.sh for TASK/FS08
# part 4 (stdiobuf). Changes: output dir = $PROBE_BIN (the session scratch,
# not this run's bin/), logs + repo hashes = $BUILD_LOGS (default ../checks).
# build-all.sh -- rebuild every program the E9 runs use, from the
# current tree (stage-2 script + the 3a/3b/3c probes: build-probes.sh,
# build-mprot.sh, build-e10.sh, ENVRN via S2_TESTS), and record their hashes (../repo-binaries.sha256,
# ../bin/SHA256SUMS). The probes come from build-s2-tests.sh; HELLO.NCP from
# tools/mk-hello-ncp.py; OLDEXEC.NCP (a deliberate ET_EXEC, refusal test only)
# from build-oldexec.sh (the 0d recipe).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
BIN="${PROBE_BIN:?set PROBE_BIN (scratch output dir)}"; LOGS="${BUILD_LOGS:-$HERE/../checks}"; mkdir -p "$BIN" "$LOGS"
cd "$WS"
ex() { sh sdk/build.sh "sdk/examples/$1" "$BIN/$2" > /dev/null || { echo "FAIL $2"; exit 1; }; echo "built $2"; }
ex a107parent.c A107PAR.NAP; ex a79color.c A79COLOR.NAP; ex a96zombie.c A96ZOMB.NAP
ex big.c BIG.NCP; ex cmdline.c CMDLINE.NCP; cp "$BIN/CMDLINE.NCP" "$BIN/CMDLINE.NTP"
ex crashtst.c CRASHTST.NAP; ex crchild.c CRCHILD.NAP; ex curdir.c CURDIR.NCP
ex dtachild.c DTACHILD.NCP; ex hello.c HELLO2.NCP; ex kbdvec.c KBDVEC.NAP
ex physbase.c PHYSBASE.NCP; ex scrflip.c SCRFLIP.NCP; ex sysacc.c SYSACC.NAC
ex sysinfo.c SYSINFO.NCP; ex wfscreen.c WFSCREEN.NAP
python3 tools/mk-hello-ncp.py "$BIN/HELLO.NCP" > /dev/null && echo "built HELLO.NCP"
sh "$HERE/build-oldexec.sh" "$BIN/OLDEXEC.NCP" > /dev/null && echo "built OLDEXEC.NCP"
bash sdk/libc/build-libc.sh sdk/examples/port10kid.c "$BIN/P10KID.NCP" > "$LOGS/build-p10kid.log" 2>&1 && echo "built P10KID.NCP" || { echo "FAIL P10KID"; exit 1; }
bash sdk/libc/build-libc.sh sdk/examples/port10.c "$BIN/P10TEST.NCP" > "$LOGS/build-p10test.log" 2>&1 && echo "built P10TEST.NCP" || { echo "FAIL P10TEST"; exit 1; }
S2_TESTS="envrn e9mem" bash "$HERE/build-s2-tests.sh" "$BIN"
# E9 probes (not in the E9 copy of this script): E9MEM (libc, above) and
# E9PLAIN (plain sdk/crt0.S).
sh sdk/build.sh "$HERE/tests/e9plain.c" "$BIN/E9PLAIN.NCP" > /dev/null || { echo "FAIL E9PLAIN"; exit 1; }; echo "built E9PLAIN.NCP"
bash "$HERE/build-probes.sh" "$BIN"
bash "$HERE/build-mprot.sh" "$BIN"
bash "$HERE/build-e10.sh" "$BIN"
for s in apps/apitest/build-apitest.sh apps/build-qed.sh apps/build-teradesk.sh apps/build-doom.sh \
         apps/gemtest/build-gemtest.sh apps/chaintest/build-chaintest.sh apps/build-coremark.sh; do
  bash "sdk/$s" > "$LOGS/build-$(basename $s .sh).log" 2>&1 && echo "built via $s" || { echo "FAIL $s"; tail -20 "$LOGS/build-$(basename $s .sh).log"; exit 1; }
done
(cd "$BIN" && shasum -a 256 *.NAP *.NCP *.NTP *.NAC | sort -k2 > SHA256SUMS)
shasum -a 256 sdk/apps/apitest/APITEST.NCP sdk/apps/qed/build/QED.NAP sdk/apps/qed/build/qed.rsc \
  sdk/apps/teradesk/build/dist/DESKTOP.NAP sdk/apps/doom/build/DOOM.NAP > "$LOGS/repo-binaries.sha256"
echo done
