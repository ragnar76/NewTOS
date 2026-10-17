#!/usr/bin/env bash
# build-int.sh -- fresh release rpi4 kernels with the scripted IKBD injector (tools/inject-arm.py, tag kd), both sides:
#   head = git archive b8245e5c; wt = the same + the uncommitted RST01 files (as build-kernels.sh).
#   kd: launch:DESKTOP=td desktop=s1 launch:QED=q      kt: launch:QED=qt      kb: launch:KBPROBE=kb desktop=kbn
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SC="$HERE/../scripts"; M="$SC/m113"
export SOURCE_DATE_EPOCH=1792152000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"; FILES="$( (git diff --name-only $HEADREV -- kernel tos-api; git ls-files --others --exclude-standard -- kernel tos-api) | sort -u)"
for side in ${SIDES:-head wt}; do for k in ${KERNELS:-kd kt kb}; do
  T="$S/trees/$k${QTS:+-$QTS}-$side"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
  if [ $side = wt ]; then for f in $FILES; do mkdir -p "$T/$(dirname "$f")"; cp "$WS/$f" "$T/$f"; done; fi
  case $k in
    kd) A=(launch:DESKTOP="$M/td.script" desktop="$M/s1.script" launch:QED="$M/q.script");;
    kt) A=(launch:QED="$SC/${QTS:-qt}.script");;
    kb) A=(launch:KBPROBE="$SC/kb.script" desktop="$SC/kbn.script");;
  esac
  python3 "$WS/tools/inject-arm.py" "$T" --tag kd "${A[@]}"
  (cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$k-$side.log" 2>&1 || { tail -30 "$S/logs/build-$k-$side.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$k${QTS:+-$QTS}-$side.img"; shasum -a 256 "$S/bin/kernel8-$k${QTS:+-$QTS}-$side.img"
done; done
