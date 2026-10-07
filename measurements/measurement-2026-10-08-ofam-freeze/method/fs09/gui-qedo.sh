#!/usr/bin/env bash
set -uo pipefail
S="$1"; L="$2"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TMPDIR="$S" APPEND="serial=on memprot=on"; unset TRACE
mkdir -p "$S/cards"; tag="$L-qedo"
bash "$HERE/prep-card.sh" "$S" "$S/cards/$tag.img" qed > /dev/null
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$S/cards/$tag.img")
mcopy -o -i "$S/cards/$tag.img@@$OFF" "$HERE/desk/NOTE.TXT" ::Apps/QED/NOTE.TXT
export CHECK="python3 $HERE/qedo-check.py"
bash "$HERE/run-gui.sh" "$S" "$S/fx-kinj-qedo.img" "$S/cards/$tag.img" "$tag" --stamp --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 240 2>&1 | grep -E "^run |HOSTCHECK|^==|Warning|differ|orphan|lost|Cross"
cat "$S/out/$tag-hostcheck.txt"
grep -caE "\[EXC\]|PANIC|Memory violation" "$S/serial/$tag-serial.log" | sed 's/^/exceptions in serial: /'
