#!/usr/bin/env bash
# teradesk-up.sh -- DESK43 U9-0d: the `up:NAME` trigger of tools/inject-arm.py on an EL0 shell child. Scratch kernel =
# shared tree + `up:DESKTOP=nop.script launch:DESKTOP=nop.script` (TeraDesk's #Z entry is C:\Apps\TERADESK\DESKTOP.NAP,
# p_nameit -> DESKTOP; `desktop` is not armed). Then the U8b series' TeraDesk run (plain QEMU raspi4b, memprot on,
# t35 dump) on a private copy of the same card; the t35 dump must equal U8b's u8b-on-rpi4-teradesk-t35.png.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/tmp/m5j/u90
export SOURCE_DATE_EPOCH=1791979200
sh "$HERE/make-scratch.sh" "$W/tree-tera"
python3 "$WS/tools/inject-arm.py" "$W/tree-tera" --tag t10 up:DESKTOP="$HERE/nop.script" launch:DESKTOP="$HERE/nop.script"
(cd "$W/tree-tera" && cargo xtask build --board rpi4 --release) > "$W/build-tera.log" 2>&1 || { tail -30 "$W/build-tera.log"; exit 1; }
grep -E "generated [0-9]+ warning|^error" "$W/build-tera.log" || true
cp "$W/tree-tera/kernel8.img" "$M/bin/kernel8-inj-tera.img"
cd "$W" && APPEND="serial=on" bash "$HERE/run-rpi4.sh" "$M/bin/kernel8-inj-tera.img" /tmp/m5j/u5b/cards/teradesk.img inj-on-rpi4-teradesk 35:t35.png
