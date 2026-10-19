#!/usr/bin/env bash
# t15-stress.sh <iterations> -- TASK/T15 item 1: run the three unlocked drawing-test binaries
# (aes_cicon_draw, aes_object_font, aes_object_nicelines) <iterations> times each, all three
# concurrently (load), --test-threads=32, and count failing iterations per binary.
# Binaries: cargo test -p tos-api --test <name> --no-run (paths resolved below).
set -uo pipefail
N="${1:-300}"; WS=/Volumes/Daten/Development/PiRusTOS; cd "$WS"
bins=$(cargo test -p tos-api --test aes_cicon_draw --test aes_object_font --test aes_object_nicelines --no-run 2>&1 | sed -n 's/.*Executable tests\/\([a-z_]*\)\.rs (\(.*\))/\1=\2/p')
for b in $bins; do
  name="${b%%=*}"; exe="${b#*=}"
  ( fails=0; for i in $(seq 1 "$N"); do "$exe" --test-threads=32 -q >/dev/null 2>&1 || fails=$((fails+1)); done
    echo "$name: $N iterations, $fails failed" ) &
done
wait
