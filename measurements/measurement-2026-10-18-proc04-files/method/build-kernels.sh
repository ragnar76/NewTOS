#!/usr/bin/env bash
# build-kernels.sh <side> -- fresh release rpi4 kernels from `git archive $HEADREV` into scratch trees with own target dirs.
#   side=head : the archive as is (red run)
#   side=wt   : the archive + the uncommitted kernel/ tos-api/ files of the working tree (green run)
# Per side (KERNELS, default "plain inj"): plain (product); inj (+ tools/inject-arm.py launch:P4Z2=emucon.script);
# s2s (+ launch:S2ALLS=regress/space.script, answers the crash prompts of strict S2ALLS); tdq (+ up:DESKTOP=regress/f1.script,
# F1 in TeraDesk starts QED). Instrumentation on the scratch trees only.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SIDE="${1:?head|wt}"; export SOURCE_DATE_EPOCH=1792152000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"; FILES=""
[ "$SIDE" = wt ] && FILES="$( (git diff --name-only $HEADREV -- kernel tos-api; git ls-files --others --exclude-standard -- kernel tos-api) | sort -u)"
echo "working-tree files over $HEADREV ($SIDE):"; echo "$FILES" | sed 's/^/  /'
for k in ${KERNELS:-plain inj}; do
  T="$S/trees/$SIDE-$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
  for f in $FILES; do mkdir -p "$T/$(dirname "$f")"; cp "$WS/$f" "$T/$f"; done
  [ $k = inj ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj launch:P4Z2="$HERE/emucon.script"
  [ $k = s2s ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj launch:S2ALLS="$HERE/regress/space.script"
  SC="$HERE/../scripts"; M="$SC/m113"
  [ $k = kd ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kd launch:DESKTOP="$M/td.script" desktop="$M/s1.script" launch:QED="$M/q.script"
  [ $k = kdn ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kn desktop="$SC/n/dn.script"
  [ $k = ktn ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kn up:DESKTOP="$SC/n/tn.script"
  [ $k = kqn ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kn up:QED="$SC/n/qn.script"
  [ $k = tdq ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:DESKTOP="$HERE/regress/f1.script"
  (cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$SIDE-$k.log" 2>&1 || { tail -30 "$S/logs/build-$SIDE-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$SIDE-$k.img"; shasum -a 256 "$S/bin/kernel8-$SIDE-$k.img"
done
