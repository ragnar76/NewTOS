#!/usr/bin/env bash
# build-all.sh <side> [variant ...] -- fresh release rpi4 kernels from `git archive $HEADREV` (side head) or that + the uncommitted
# kernel/ tos-api/ files (side wt), into scratch trees; instrumented variants clone the plain tree's target dir (APFS cp -c).
# variants: plain inj2(emucon2.script) con01(con01/con01.script) s2s tdq kd kdn ktn kqn  + any extra defined in $HERE/extra-variants.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SIDE="${1:?head|wt}"; shift; export SOURCE_DATE_EPOCH=1792152000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"; FILES=""
[ "$SIDE" = wt ] && FILES="$( (git diff --name-only $HEADREV -- kernel tos-api; git ls-files --others --exclude-standard -- kernel tos-api) | sort -u)"
echo "files over $HEADREV ($SIDE):"; echo "$FILES" | sed 's/^/  /'
SC="$HERE/../scripts"; M="$SC/m113"
for k in "${@:-plain}"; do
  T="$S/trees/$SIDE-$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
  for f in $FILES; do mkdir -p "$T/$(dirname "$f")"; cp "$WS/$f" "$T/$f"; done
  I="python3 $WS/tools/inject-arm.py $T"
  case $k in
    plain) ;;
    inj2) $I --tag inj launch:P4Z2="$HERE/emucon2.script";;
    con01) $I --tag inj launch:P4Z2="$HERE/con01/con01.script";;
    s2s) $I --tag inj launch:S2ALLS="$HERE/regress/space.script";;
    tdq) $I --tag inj up:DESKTOP="$HERE/regress/f1.script";;
    kd) $I --tag kd launch:DESKTOP="$M/td.script" desktop="$M/s1.script" launch:QED="$M/q.script";;
    kdn) $I --tag kn desktop="$SC/n/dn.script";;
    ktn) $I --tag kn up:DESKTOP="$SC/n/tn.script";;
    kqn) $I --tag kn up:QED="$SC/n/qn.script";;
    *) . "$HERE/extra-variants.sh"; extra_variant "$k";;
  esac
  (cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$SIDE-$k.log" 2>&1 || { tail -30 "$S/logs/build-$SIDE-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$SIDE-$k.img"; shasum -a 256 "$S/bin/kernel8-$SIDE-$k.img"
done
