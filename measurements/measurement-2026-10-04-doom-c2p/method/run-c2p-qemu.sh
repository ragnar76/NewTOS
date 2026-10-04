#!/usr/bin/env bash
#
# run-c2p-qemu.sh <scratch-dir> <evidence-dir>
#
# PORT06 step "table c2p" (2026-10-04): NewTOS must be unaffected.
# = measurement-2026-10-04-doom-aspect/method/run-aspect.sh, "play" part,
# reduced to the runs that cover both routes:
#   a-640-default     EdDI truecolour (packed, no c2p involved)
#   e-640-noeddi      vr_trnfm route, 16 planes = 2 slices of the new c2p
#   g-1080-noeddi-x4  vr_trnfm route, 16 planes, scaled x4 on 1920x1080
# DOOM.NAP from the delivered sdk/apps/build-doom.sh, DOOM_STATS=5 (cost
# probe "c2p N us" + fps line), BUILD in the scratch dir. Same kernel8.img,
# same NEWDESK.INF/LAUNCH.NAP mechanism, same dump times as the aspect step,
# so serial lines and screendumps line up with it.
set -euo pipefail
SCR="${1:?usage: run-c2p-qemu.sh <scratch-dir> <evidence-dir>}"; EV="${2:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
KERNEL="${KERNEL:-$WS/kernel8.img}"
WAD="$WS/Applications/doom-wad/doom1.wad"
mkdir -p "$SCR" "$EV/screendumps" "$EV/serial" "$EV/method"
"$WS/sdk/libc/build-libc.sh" "$HERE/launch.c" "$SCR/LAUNCH.NAP" > "$EV/method/build-launch.log" 2>&1
grep -E '^LINK' "$EV/method/build-launch.log"

run() {
    local tag="$1" res="$2" nap="$3" inf="$4" cmd="$5"; shift 5
    if [ "$cmd" = "-" ]; then
        "$HERE/seed-scale.sh" "$SCR/$tag.img" "$nap" "$WAD" "$HERE/$inf" 2>/dev/null
    else
        "$HERE/seed-scale.sh" "$SCR/$tag.img" "$nap" "$WAD" "$HERE/$inf" "$SCR/LAUNCH.NAP" "$cmd" 2>/dev/null
    fi
    "$HERE/run-qemu.sh" "$KERNEL" "$SCR/$tag.img" "$SCR/out" "$tag" "$res" "$@"
    rm -f "$SCR/$tag.img"
    cp "$SCR/out/$tag-serial.log" "$EV/serial/" && rm -f "$SCR/out/$tag-serial.log"
    for f in "$SCR"/out/*.png; do [ -e "$f" ] && cp "$f" "$EV/screendumps/" && rm -f "$f"; done
    tr '\r' '\n' < "$EV/serial/$tag-serial.log" | grep -a -E 'LAUNCH|DOOM|timed|EXC|FAR|exited|PANIC' | grep -v '^$' | head -16 || true
}

BUILD="$SCR/doom-play" DOOM_STATS=5 "$WS/sdk/apps/build-doom.sh" > "$EV/method/build-doom-play.log" 2>&1
grep -E "compiled OK|LINK|DOOM_|platform layer" "$EV/method/build-doom-play.log"
P="$SCR/doom-play/DOOM.NAP"; shasum -a 256 "$P"
L=newdesk-launch.inf
run a-640-default    640x480   "$P" newdesk-doom.inf -                     35:a-640-default-t35.png 60:a-640-default-t60.png
run e-640-noeddi     640x480   "$P" $L 'C:\DOOM\DOOM.NAP -noeddi'          35:e-640-noeddi-t35.png 60:e-640-noeddi-t60.png
run g-1080-noeddi-x4 1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -noeddi -scale 4' 35:g-1080-noeddi-x4-t35.png 60:g-1080-noeddi-x4-t60.png
