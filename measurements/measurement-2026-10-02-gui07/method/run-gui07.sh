#!/usr/bin/env bash
#
# run-gui07.sh prep <scratch>
# run-gui07.sh <arm> <scratch> <evidence-dir> [desk|rast|ball|play|tdemo|all]
#
# GUI07 before/after, headless QEMU raspi4b. <arm> is a tag ("before",
# "after"); the kernel is $KERNEL (default <scratch>/kernel8-<arm>.img).
#
#   prep   builds the apps ONCE into <scratch>/apps, so both arms run the very
#          same binaries: LAUNCH.NAP (method/launch.c), DOOM.NAP play
#          (delivered sdk/apps/build-doom.sh defaults: DOOM_STATS=5, "-mb","2")
#          and DOOM.NAP tdemo (+ "-timedemo","demo1"). BALL.NAP is the
#          delivered conformant build in /Volumes/Daten/Development/ball-demo
#          (PORT07), DUMPRAST.NAP the one in tosdump/build (2026-09-21) --
#          both used as found, hashes logged.
#   desk   the desktop, no autorun, dumps at 30/45 s, 640x480 and 1920x1080
#   rast   DUMPRAST.NAP over tosdump/cases/vdi-raster-screen.tsv (62 cases,
#          R02 screen->memory, R03 memory->screen, R04 screen->screen,
#          R05 vrt_cpyfm) -> RASTER.BIN, compared with tosdiff afterwards
#   ball   BALL.NAP with "hud 1", 640x480 and 1920x1080 -> BALL.LOG
#   play   DOOM default stage (640x480 -> x2, 1920x1080 -> x4) and -scale 1/2
#          on 1920x1080: the per-frame cost probe line, many dumps
#   tdemo  timedemo demo1 at x1/x2/x4 on 1920x1080
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
WAD="$WS/Applications/doom-wad/doom1.wad"
BALLDIR=/Volumes/Daten/Development/ball-demo

if [ "${1:-}" = prep ]; then
    SCR="${2:?}"; A="$SCR/apps"; mkdir -p "$A"
    "$WS/sdk/libc/build-libc.sh" "$HERE/launch.c" "$A/LAUNCH.NAP" > "$A/build-launch.log" 2>&1
    grep -E '^LINK' "$A/build-launch.log"
    BUILD="$A/doom-play" "$WS/sdk/apps/build-doom.sh" > "$A/build-doom-play.log" 2>&1
    grep -E "compiled OK|LINK|DOOM_" "$A/build-doom-play.log"
    BUILD="$A/doom-tdemo" DOOM_ARGS='"-mb","2","-timedemo","demo1"' \
        "$WS/sdk/apps/build-doom.sh" > "$A/build-doom-tdemo.log" 2>&1
    grep -E "compiled OK|LINK|DOOM_" "$A/build-doom-tdemo.log"
    printf 'hud 1\r\n' > "$A/BALL.CFG"
    shasum -a 256 "$A/LAUNCH.NAP" "$A/doom-play/DOOM.NAP" "$A/doom-tdemo/DOOM.NAP" \
        "$BALLDIR/BALL.NAP" "$BALLDIR/BALL.NTX" "$WS/tosdump/build/DUMPRAST.NAP" \
        "$WS/tosdump/cases/vdi-raster-screen.tsv" "$WAD"
    exit 0
fi

ARM="${1:?usage: run-gui07.sh <arm> <scratch> <evidence> [what]}"; SCR="${2:?}"; EV="${3:?}"; WHAT="${4:-all}"
A="$SCR/apps"
KERNEL="${KERNEL:-$SCR/kernel8-$ARM.img}"
mkdir -p "$SCR/out" "$EV/screendumps/$ARM" "$EV/serial/$ARM" "$EV/raw/$ARM"
echo "arm $ARM kernel $(shasum -a 256 "$KERNEL")"

# run <tag> <res> <seed pairs as one string> <dump specs...>
run() {
    local tag="$1" res="$2" pairs="$3"; shift 3
    # shellcheck disable=SC2086
    "$HERE/seed.sh" "$SCR/$tag.img" $pairs 2>/dev/null
    "$HERE/run-qemu.sh" "$KERNEL" "$SCR/$tag.img" "$SCR/out" "$tag" "$res" "$@"
    cp "$SCR/out/$tag-serial.log" "$EV/serial/$ARM/" && rm -f "$SCR/out/$tag-serial.log"
    for f in "$SCR"/out/*.png; do [ -e "$f" ] && cp "$f" "$EV/screendumps/$ARM/" && rm -f "$f"; done
    tr '\r' '\n' < "$EV/serial/$ARM/$tag-serial.log" | grep -a -E 'LAUNCH|DOOM|timed|EXC|FAR|PANIC|BALL: blit|BALL: headroom|RASTER|all .* cases' | grep -v '^$' | head -16 || true
}
dumps() { # dumps <prefix> <from> <to> <step>
    local p="$1" t="$2"; while [ "$t" -le "$3" ]; do printf '%s:%s-t%03d.png ' "$t" "$p" "$t"; t=$((t + $4)); done
}

if [ "$WHAT" = desk ] || [ "$WHAT" = all ]; then
    run desk-640  640x480   "" 30:desk-640-t30.png 45:desk-640-t45.png;  rm -f "$SCR/desk-640.img"
    run desk-1080 1920x1080 "" 30:desk-1080-t30.png 45:desk-1080-t45.png; rm -f "$SCR/desk-1080.img"
fi

if [ "$WHAT" = rast ] || [ "$WHAT" = all ]; then
    run rast-640 640x480 "$WS/tosdump/build/DUMPRAST.NAP=DUMPRAST.NAP $WS/tosdump/cases/vdi-raster-screen.tsv=RASTER.TSV $HERE/newdesk-dumprast.inf=NEWDESK.INF" \
        45:rast-640-t45.png
    "$HERE/pull.sh" "$SCR/rast-640.img" RASTER.BIN "$EV/raw/$ARM/RASTER.BIN" || true
    "$HERE/pull.sh" "$SCR/rast-640.img" RASTER.LOG "$EV/raw/$ARM/RASTER.LOG" || true
    rm -f "$SCR/rast-640.img"
fi

if [ "$WHAT" = ball ] || [ "$WHAT" = all ]; then
    for res in 640x480 1920x1080; do
        tag="ball-${res#*x}"
        run "$tag" "$res" "$BALLDIR/BALL.NAP=BALL.NAP $BALLDIR/BALL.NTX=BALL.NTX $A/BALL.CFG=BALL.CFG $HERE/newdesk-ball.inf=NEWDESK.INF" \
            40:"$tag"-t40.png 70:"$tag"-t70.png
        "$HERE/pull.sh" "$SCR/$tag.img" BALL.LOG "$EV/raw/$ARM/$tag-BALL.LOG" || true
        rm -f "$SCR/$tag.img"
    done
fi

DOOMP="$A/doom-play/DOOM.NAP"; DOOMT="$A/doom-tdemo/DOOM.NAP"
doom_pairs() { # doom_pairs <nap> <inf> [<command line file>]
    local s="$1=DOOM/DOOM.NAP $WAD=DOOM/DOOM1.WAD $HERE/$2=NEWDESK.INF"
    [ -n "${3:-}" ] && s="$s $A/LAUNCH.NAP=LAUNCH.NAP $3=LAUNCH.TXT"
    echo "$s"
}
cmdfile() { local f="$SCR/cmd-$1.txt"; printf '%s\r\n' "$2" > "$f"; echo "$f"; }

if [ "$WHAT" = play ] || [ "$WHAT" = all ]; then
    # shellcheck disable=SC2046
    run a-640-default  640x480   "$(doom_pairs "$DOOMP" newdesk-doom.inf)" $(dumps a-640-default 16 60 2); rm -f "$SCR/a-640-default.img"
    # shellcheck disable=SC2046
    run b-1080-default 1920x1080 "$(doom_pairs "$DOOMP" newdesk-doom.inf)" $(dumps b-1080-default 20 70 5); rm -f "$SCR/b-1080-default.img"
    run c-1080-x1 1920x1080 "$(doom_pairs "$DOOMP" newdesk-launch.inf "$(cmdfile x1 'C:\DOOM\DOOM.NAP -scale 1')")" 35:c-1080-x1-t35.png 60:c-1080-x1-t60.png; rm -f "$SCR/c-1080-x1.img"
    run d-1080-x2 1920x1080 "$(doom_pairs "$DOOMP" newdesk-launch.inf "$(cmdfile x2 'C:\DOOM\DOOM.NAP -scale 2')")" 35:d-1080-x2-t35.png 60:d-1080-x2-t60.png; rm -f "$SCR/d-1080-x2.img"
fi

if [ "$WHAT" = tdemo ] || [ "$WHAT" = all ]; then
    # End offsets as in PORT06's EdDI run (room for 5026 frames at the
    # BEFORE cost, plus boot); kept identical for the after arm.
    run t1-1080-x1 1920x1080 "$(doom_pairs "$DOOMT" newdesk-launch.inf "$(cmdfile tx1 'C:\DOOM\DOOM.NAP -scale 1')")" 60:t1-x1-t60.png 150:t1-x1-end.png;  rm -f "$SCR/t1-1080-x1.img"
    run t2-1080-x2 1920x1080 "$(doom_pairs "$DOOMT" newdesk-launch.inf "$(cmdfile tx2 'C:\DOOM\DOOM.NAP -scale 2')")" 60:t2-x2-t60.png 330:t2-x2-end.png;  rm -f "$SCR/t2-1080-x2.img"
    run t4-1080-x4 1920x1080 "$(doom_pairs "$DOOMT" newdesk-launch.inf "$(cmdfile tx4 'C:\DOOM\DOOM.NAP -scale 4')")" 60:t4-x4-t60.png 1020:t4-x4-end.png; rm -f "$SCR/t4-1080-x4.img"
fi
