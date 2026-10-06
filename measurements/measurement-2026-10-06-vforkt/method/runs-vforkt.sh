#!/usr/bin/env bash
# runs-vforkt.sh <scratch> -- the diagnosis runs (QEMU raspi4b, private cards
# in <scratch>/cards, kernel <scratch>/kernel8-vtc.img = the post-E9 crash
# kernel instrumentation + vt-instrumentation.py). Programs: the post-E9 bin/
# (VFORKT = exit() crt0); "old" cards swap in VFORKT built against the HEAD
# (Pterm) crt0 (build-vforkt.sh, <scratch>/VFORKT-headcrt0.NAP).
set -uo pipefail
S="${1:?scratch}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../../measurement-2026-10-06-post-e9/bin"
K="$S/kernel8-vtc.img"; C="$S/cards"; mkdir -p "$C"
rm -rf "$S/binold"; cp -R "$B" "$S/binold"; cp "$S/VFORKT-headcrt0.NAP" "$S/binold/VFORKT.NAP"
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
sd s2all 'C:\S2ALL.NAP'; sd s2alls 'C:\S2ALLS.NAP'; sd vforkt 'C:\VFORKT.NAP'
[ -f "$C/s2all-old.img" ] || BIN="$S/binold" bash "$HERE/seed-card.sh" "$C/s2all-old.img" 'C:\S2ALL.NAP' > /dev/null
[ -f "$C/vforkt-old.img" ] || BIN="$S/binold" bash "$HERE/seed-card.sh" "$C/vforkt-old.img" 'C:\VFORKT.NAP' > /dev/null
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
E='S2ALL: (PASS|FAIL)'
APPEND="serial=on"                 rd "$K" "$C/s2all.img"      vtc-on-s2all       --end "$E" --end-delay 2 --timeout 300
APPEND="serial=on"                 rd "$K" "$C/s2all-old.img"  vtc-on-s2all-oldcrt0 --end "$E" --end-delay 2 --timeout 300
APPEND="serial=on memprot=off"     rd "$K" "$C/s2all.img"      vtc-off-s2all      --end "$E" --end-delay 2 --timeout 300
APPEND="serial=on memprot=strict"  rd "$K" "$C/s2alls.img"     vtc-strict-s2alls  --end "$E" --end-delay 2 --timeout 300
APPEND="serial=on"                 rd "$K" "$C/vforkt.img"     vtc-on-vforkt-alone --end '\[desk\] desktop up|VFORK: (PASS|FAIL)' --end-delay 6 --timeout 200
APPEND="serial=on"                 rd "$K" "$C/vforkt-old.img" vtc-on-vforkt-alone-oldcrt0 --end '\[desk\] desktop up' --end-delay 2 --timeout 200
