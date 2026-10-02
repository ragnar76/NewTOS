#!/usr/bin/env bash
#
# run-scale.sh <scratch-dir> <outdir> [play|tdemo|all]
#
# PORT06 scaling step (stages x1..x4 with 4:3 correction), headless QEMU
# raspi4b. Every DOOM.NAP here comes from the delivered sdk/apps/build-doom.sh:
#   play   the delivered default build (DOOM_STATS=5, DOOM_ARGS "-mb 2"),
#          sha256 recorded in the log;
#   tdemo  the same with DOOM_ARGS '"-mb","2","-timedemo","demo1"'.
# The stage reaches DOOM as a real command tail: C:\LAUNCH.NAP (method/
# launch.c, #Z autorun) runs C:\LAUNCH.TXT with system() -> Pexec(0), the
# path the desktop's parameter box feeds too. Runs without a launcher start
# DOOM.NAP directly by #Z (no tail = the default stage).
# Screen sizes: 640x480 (QEMU's default) and 1920x1080 (the real Pi's), set
# with -global bcm2835-fb.xres/yres (method/run-qemu.sh).
set -euo pipefail
SCR="${1:?usage: run-scale.sh <scratch-dir> <outdir> [play|tdemo|all]}"; OUT="${2:?}"; WHAT="${3:-all}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
KERNEL="${KERNEL:-$WS/kernel8.img}"
WAD="$WS/Applications/doom-wad/doom1.wad"
mkdir -p "$SCR" "$OUT"
"$WS/sdk/libc/build-libc.sh" "$HERE/launch.c" "$SCR/LAUNCH.NAP" > "$OUT/build-launch.log" 2>&1
grep -E '^LINK' "$OUT/build-launch.log"

# run <tag> <res> <nap> <inf> <cmd or -> <dump specs...>
run() {
    local tag="$1" res="$2" nap="$3" inf="$4" cmd="$5"; shift 5
    if [ "$cmd" = "-" ]; then
        "$HERE/seed-scale.sh" "$SCR/$tag.img" "$nap" "$WAD" "$HERE/$inf" 2>/dev/null
    else
        "$HERE/seed-scale.sh" "$SCR/$tag.img" "$nap" "$WAD" "$HERE/$inf" "$SCR/LAUNCH.NAP" "$cmd" 2>/dev/null
    fi
    "$HERE/run-qemu.sh" "$KERNEL" "$SCR/$tag.img" "$OUT" "$tag" "$res" "$@"
    rm -f "$SCR/$tag.img"
    tr '\r' '\n' < "$OUT/$tag-serial.log" | grep -a -E 'LAUNCH|DOOM|timed|EXC|FAR|exited' | grep -v '^$' | head -14 || true
}

if [ "$WHAT" = play ] || [ "$WHAT" = all ]; then
    BUILD="$SCR/doom-play" "$WS/sdk/apps/build-doom.sh" > "$OUT/build-doom-play.log" 2>&1
    grep -E "compiled OK|LINK|DOOM_" "$OUT/build-doom-play.log"
    P="$SCR/doom-play/DOOM.NAP"; shasum -a 256 "$P"
    L=newdesk-launch.inf
    run a-640-default  640x480   "$P" newdesk-doom.inf      -                           35:a-640-default-t35.png 60:a-640-default-t60.png
    run b-640-x1       640x480   "$P" $L 'C:\DOOM\DOOM.NAP -scale 1' 35:b-640-x1-t35.png 60:b-640-x1-t60.png
    run c-640-x3       640x480   "$P" $L 'C:\DOOM\DOOM.NAP -scale 3' 35:c-640-x3-t35.png 60:c-640-x3-t60.png
    run d-1080-x1      1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -scale 1' 35:d-1080-x1-t35.png 60:d-1080-x1-t60.png
    run e-1080-x2      1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -scale 2' 35:e-1080-x2-t35.png 60:e-1080-x2-t60.png
    run f-1080-x3      1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -scale 3' 35:f-1080-x3-t35.png 60:f-1080-x3-t60.png
    run g-1080-x4      1920x1080 "$P" $L 'C:\DOOM\DOOM.NAP -scale 4' 35:g-1080-x4-t35.png 60:g-1080-x4-t60.png
    run h-1080-default 1920x1080 "$P" newdesk-doom.inf      -                           35:h-1080-default-t35.png 60:h-1080-default-t60.png
    run i-640-charmode 640x480   "$P" newdesk-doom-char.inf -                           35:i-640-charmode-t35.png 60:i-640-charmode-t60.png
fi

if [ "$WHAT" = tdemo ] || [ "$WHAT" = all ]; then
    BUILD="$SCR/doom-tdemo" DOOM_ARGS='"-mb","2","-timedemo","demo1"' \
        "$WS/sdk/apps/build-doom.sh" > "$OUT/build-doom-tdemo.log" 2>&1
    grep -E "compiled OK|LINK|DOOM_" "$OUT/build-doom-tdemo.log"
    T="$SCR/doom-tdemo/DOOM.NAP"; shasum -a 256 "$T"
    L=newdesk-launch.inf
    # The end offsets leave room for 5026 frames at the stage's frame cost
    # measured in the play runs (QEMU TCG), plus boot.
    run t1-1080-x1 1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -scale 1' 60:t1-x1-t60.png  150:t1-x1-end.png
    run t2-1080-x2 1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -scale 2' 60:t2-x2-t60.png  330:t2-x2-end.png
    run t3-1080-x3 1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -scale 3' 60:t3-x3-t60.png  600:t3-x3-end.png
    run t4-1080-x4 1920x1080 "$T" $L 'C:\DOOM\DOOM.NAP -scale 4' 60:t4-x4-t60.png 1020:t4-x4-end.png
fi
