#!/usr/bin/env bash
# build-naps.sh <src-tree> -- DESK43 U9-2: the two scratch NEWDESK variants of this run, built from an rsync copy of
# <src-tree> (/tmp/m5j/u912/tree-napvar), never the shared tree:
#   NDSELFT.NAP  programs/build.sh newdesk -- --features board-rpi4,selftest-desk   (the desktop self-tests in EL0)
#   NDPANIC.NAP  newdesk/src/lib.rs with `panic!(...)` before desktop::run() (marker U92-PANIC-TEMP), board-rpi4
#                -- the product panic handler with panic-wait-key; LTO drops the now dead desktop (27720 B).
# Both go to C:\NEWDESK.NAP of their card (derive-card.sh) and are copied to ../bin.
set -euo pipefail
SRC="${1:?}"; W=/tmp/m5j/u912; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export SOURCE_DATE_EPOCH=1791979200
rsync -a --delete --exclude target "$SRC/" "$W/tree-napvar/"
cd "$W/tree-napvar"
mkdir -p "$W/nap-st" "$W/nap-panic"
bash programs/build.sh newdesk "$W/nap-st/NEWDESK.NAP" -- --features board-rpi4,selftest-desk
python3 - <<'PY'
import pathlib
p = pathlib.Path('programs/newdesk/src/lib.rs'); s = p.read_text()
old = "    desktop::run();\n    0\n"
assert s.count(old) == 1
s = s.replace(old, "    panic!(\"U9-2 scratch: deliberate panic before desktop::run()\"); // U92-PANIC-TEMP\n"
              "    #[allow(unreachable_code)]\n    {\n        desktop::run();\n        0\n    }\n")
p.write_text(s)
PY
bash programs/build.sh newdesk "$W/nap-panic/NEWDESK.NAP" -- --features board-rpi4
cp "$W/nap-st/NEWDESK.NAP" "$HERE/../bin/NDSELFT.NAP"; cp "$W/nap-panic/NEWDESK.NAP" "$HERE/../bin/NDPANIC.NAP"
