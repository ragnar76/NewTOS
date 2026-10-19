#!/usr/bin/env bash
# (copied from measurement-2026-10-19-proc04-fdelete-test/method; used here: plain, drn, trn, qsv, s2s, tdq)
# build-kernels.sh <side> -- fresh release rpi4 kernels from `git archive $HEADREV` into scratch trees with own target dirs.
#   side=head : the archive as is (red run)
#   side=wt   : the archive + the uncommitted kernel/ tos-api/ files of the working tree (green run)
# Per side (KERNELS, default "plain"): plain (product); dsk (+ desktop=dsk.script, desktop trash/Ctrl+D deletes);
# s2s (+ launch:S2ALLS=regress/space.script, answers the crash prompts of strict S2ALLS); tdq (+ up:DESKTOP=regress/f1.script,
# F1 in TeraDesk starts QED); tdd (+ up:DESKTOP=inter/tdd.script, TeraDesk delete via its Trash); qsv (+ up:QED=inter/qsv.script,
# QED save + save over the existing file). Instrumentation on the scratch trees only.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SIDE="${1:?head|wt}"; export SOURCE_DATE_EPOCH=1792368000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"; FILES=""
[ "$SIDE" = wt ] && FILES="$( (git diff --name-only $HEADREV -- kernel tos-api; git ls-files --others --exclude-standard -- kernel tos-api) | sort -u)"
echo "working-tree files over $HEADREV ($SIDE):"; echo "$FILES" | sed 's/^/  /'
for k in ${KERNELS:-plain}; do
  T="$S/trees/$SIDE-$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
  for f in $FILES; do mkdir -p "$T/$(dirname "$f")"; cp "$WS/$f" "$T/$f"; done
  [ $k = dsk ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj desktop="$HERE/inter/dsk.script"
  [ $k = s2s ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj launch:S2ALLS="$HERE/regress/space.script"
  [ $k = tdq ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:DESKTOP="$HERE/regress/f1.script"
  [ $k = tdd ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:DESKTOP="$HERE/inter/tdd.script"
  [ $k = dcm ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj desktop="$HERE/inter/dcm.script"
  [ $k = tcp ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:DESKTOP="$HERE/inter/tcp.script"
  [ $k = ec ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj launch:P4Z2="$HERE/inter/ec.script"
  [ $k = qsv ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:QED="$HERE/inter/qsv.script"
  [ $k = drn ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj desktop="$HERE/inter/drn.script"
  [ $k = trn ] && python3 "$WS/tools/inject-arm.py" "$T" --tag inj up:DESKTOP="$HERE/inter/trn.script"
  (cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$SIDE-$k.log" 2>&1 || { tail -30 "$S/logs/build-$SIDE-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$SIDE-$k.img"; shasum -a 256 "$S/bin/kernel8-$SIDE-$k.img"
done
