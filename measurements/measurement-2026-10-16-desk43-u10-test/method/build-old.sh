#!/usr/bin/env bash
# build-old.sh <id>... -- CONTROL kernels: the tree of the U9 acceptance base (dab1fe62, git archive -> scratch, read-only on the repo), built rpi4 release with
# the same injector arms as the new-tree kernels of the same name (kf2 / kxri3 use the U9 copy inject-arm-u935.py). -> bin/kernel8-<id>old.img
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad
export SOURCE_DATE_EPOCH=1792152000
for id in "$@"; do
  T="$W/tree-${id}old"; rm -rf "$T"; mkdir -p "$T"; (cd "$WS" && git archive dab1fe62) | tar -x -C "$T"
  case $id in
    kf2) python3 "$HERE/inject-arm-u935.py" "$T" --tag u6a desktop="$HERE/f1.script" launch:HELLOGEM="$HERE/ret.script" launch:HGPANIC="$HERE/ret.script" launch:CRCHILD="$HERE/ret.script" launch:CRASHTST="$HERE/crash-keys.script";;
    kxri3) python3 "$HERE/inject-arm-u935.py" "$T" --tag kxri3 desktop="$HERE/ri3.script";;
    *) echo unknown; exit 2;;
  esac
  (cd "$T" && cargo xtask build --board rpi4 --release) > "$W/build-${id}old.log" 2>&1 || { tail -20 "$W/build-${id}old.log"; exit 1; }
  cp "$T/kernel8.img" "$M/bin/kernel8-${id}old.img"; echo "${id}old built"
done
