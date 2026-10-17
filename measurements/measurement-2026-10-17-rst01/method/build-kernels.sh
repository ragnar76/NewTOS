#!/usr/bin/env bash
# build-kernels.sh -- fresh release rpi4 kernels: `git archive HEAD` + the RST01 working-tree files (uncommitted, copied
# over the archive) into scratch trees with their own target dirs:
#   plain : no instrumentation                                           -> $S/bin/kernel8-plain.img
#   kbd   : + tools/inject-arm.py desktop=kbd.script (scancode sequence) -> $S/bin/kernel8-kbd.img
#   kbdw  : + tools/inject-arm.py desktop=kbdw.script (Ctrl+Alt+Del)        -> $S/bin/kernel8-kbdw.img
#   td    : + tools/inject-arm.py launch:DESKTOP=tdr.script (TeraDesk Quit -> Reboot) -> $S/bin/kernel8-td.img
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SC="$HERE/../scripts"
export SOURCE_DATE_EPOCH=1792152000
mkdir -p "$S/bin" "$S/logs" "$S/trees"
cd "$WS"; FILES="$( (git diff --name-only $HEADREV -- kernel tos-api; git ls-files --others --exclude-standard -- kernel tos-api) | sort -u)"
echo "RST01 files over $HEADREV:"; echo "$FILES" | sed 's/^/  /'
for k in ${KERNELS:-plain kbd kbdw td}; do
  T="$S/trees/$k"; mkdir -p "$T"; find "$T" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
  (cd "$WS" && git archive $HEADREV) | tar -x -C "$T"
  for f in $FILES; do mkdir -p "$T/$(dirname "$f")"; cp "$WS/$f" "$T/$f"; done
  [ $k = kbd ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kd desktop="$SC/kbd.script"
  [ $k = kbdw ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kd desktop="$SC/kbdw.script"
  [ $k = td ] && python3 "$WS/tools/inject-arm.py" "$T" --tag kd launch:DESKTOP="$SC/tdr.script"
  (cd "$T" && touch tos-api/src/lib.rs kernel/src/main.rs && cargo xtask build --board rpi4 --release) > "$S/logs/build-$k.log" 2>&1 || { tail -30 "$S/logs/build-$k.log"; exit 1; }
  cp "$T/kernel8.img" "$S/bin/kernel8-$k.img"; shasum -a 256 "$S/bin/kernel8-$k.img"
done
