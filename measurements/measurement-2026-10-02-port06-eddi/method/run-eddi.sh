#!/usr/bin/env bash
#
# run-eddi.sh <scratch-dir> <evidence-dir> [play|tdemo|all]
#
# PORT06 step "EdDI-Weg in Doom", headless QEMU raspi4b. The scaling step's
# method (measurement-2026-10-02-port06-scale/method: run-qemu.sh,
# seed-scale.sh, launch.c, the two NEWDESK.INF), unchanged, copied here.
# Every DOOM.NAP comes from the delivered sdk/apps/build-doom.sh:
#   play   the default build (DOOM_STATS=5, DOOM_ARGS "-mb","2")
#   tdemo  DOOM_ARGS '"-mb","2","-timedemo","demo1"'
# A command tail (-scale N, -noeddi) reaches DOOM through C:\LAUNCH.NAP
# (system() -> Pexec(0)), as in the scaling step.
# Screendumps go to <evidence-dir>/screendumps, serial logs to
# <evidence-dir>/serial, build logs to <evidence-dir>/method.
set -euo pipefail
SCR="${1:?usage: run-eddi.sh <scratch-dir> <evidence-dir> [play|tdemo|all]}"; EV="${2:?}"; WHAT="${3:-all}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
KERNEL="${KERNEL:-$WS/kernel8.img}"
WAD="$WS/Applications/doom-wad/doom1.wad"
mkdir -p "$SCR" "$EV/screendumps" "$EV/serial" "$EV/method"
"$WS/sdk/libc/build-libc.sh" "$HERE/launch.c" "$SCR/LAUNCH.NAP" > "$EV/method/build-launch.log" 2>&1
grep -E '^LINK' "$EV/method/build-launch.log"

# run <tag> <res> <nap> <inf> <cmd or -> <dump specs...>
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
    tr '\r' '\n' < "$EV/serial/$tag-serial.log" | grep -a -E 'LAUNCH|DOOM|timed|EXC|FAR|exited' | grep -v '^$' | head -16 || true
}

if [ "$WHAT" = play ] || [ "$WHAT" = all ]; then
    BUILD="$SCR/doom-play" "$WS/sdk/apps/build-doom.sh" > "$EV/method/build-doom-play.log" 2>&1
    grep -E "compiled OK|LINK|DOOM_|platform layer" "$EV/method/build-doom-play.log"
    P="$SCR/doom-play/DOOM.NAP"; shasum -a 256 "$P"
    L=newdesk-launch.inf
    run a-640-default    640x480   "$P" newdesk-doom.inf -                              35:a-640-default-t35.png 60:a-640-default-t60.png
    run b-1080-default   1920x1080 "$P" newdesk-doom.inf -                              35:b-1080-default-t35.png 60:b-1080-default-t60.png
    run c-1080-x1        1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -scale 1'                  35:c-1080-x1-t35.png 60:c-1080-x1-t60.png
    run d-1080-x2        1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -scale 2'                  35:d-1080-x2-t35.png 60:d-1080-x2-t60.png
    run e-640-noeddi     640x480   "$P" $L 'C:\DOOM\DOOM.NAP -noeddi'                   35:e-640-noeddi-t35.png 60:e-640-noeddi-t60.png
    run f-1080-noeddi-x1 1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -noeddi -scale 1'          35:f-1080-noeddi-x1-t35.png 60:f-1080-noeddi-x1-t60.png
    run g-1080-noeddi-x4 1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -noeddi -scale 4'          35:g-1080-noeddi-x4-t35.png 60:g-1080-noeddi-x4-t60.png
fi

if [ "$WHAT" = tdemo ] || [ "$WHAT" = all ]; then
    BUILD="$SCR/doom-tdemo" DOOM_ARGS='"-mb","2","-timedemo","demo1"' \
        "$WS/sdk/apps/build-doom.sh" > "$EV/method/build-doom-tdemo.log" 2>&1
    grep -E "compiled OK|LINK|DOOM_" "$EV/method/build-doom-tdemo.log"
    T="$SCR/doom-tdemo/DOOM.NAP"; shasum -a 256 "$T"
    L=newdesk-launch.inf
    run t1-1080-x1        1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -scale 1'         60:t1-x1-t60.png 150:t1-x1-end.png
    run t1n-1080-noeddi-x1 1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -noeddi -scale 1' 60:t1n-noeddi-x1-t60.png 150:t1n-noeddi-x1-end.png
    run t2-1080-x2        1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -scale 2'         60:t2-x2-t60.png 330:t2-x2-end.png
    run t2n-1080-noeddi-x2 1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -noeddi -scale 2' 60:t2n-noeddi-x2-t60.png 330:t2n-noeddi-x2-end.png
    run t4-1080-x4        1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -scale 4'         60:t4-x4-t60.png 1020:t4-x4-end.png
fi
