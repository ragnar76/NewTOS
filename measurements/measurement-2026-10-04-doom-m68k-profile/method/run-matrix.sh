#!/bin/sh
# run-matrix.sh <DOOM.PRG> <scratch-dir> [list]   (3 Hatari runs in parallel)
# Output: ../raw/<name>/ (hatari.log, profile-raw.txt.addr, *.ini, cmdline.txt)
PRG=$1; SCR=$2; S=$(cd "$(dirname "$0")" && pwd); RAW=$S/../raw
FRAMES=${FRAMES:-300}; export FRAMES
LIST=${3:-"a-plain b-enh c-detail d-detail-enh e-size8 f-size8-enh g-size5 h-size5-enh i-detail-size8 j-detail-size8-enh"}
run() { # name enh detail blocks
  DETAIL=$3 BLOCKS=$4 "$S/run-profile.sh" tt306 "$PRG" "$RAW/$1" "$SCR/$1" $2 > "$SCR/$1.out" 2>&1
}
n=0
for c in $LIST; do
  case $c in
    a-plain)            run $c "" 0 10 & ;;
    b-enh)              run $c enhancer 0 10 & ;;
    c-detail)           run $c "" 1 10 & ;;
    d-detail-enh)       run $c enhancer 1 10 & ;;
    e-size8)            run $c "" 0 8 & ;;
    f-size8-enh)        run $c enhancer 0 8 & ;;
    g-size5)            run $c "" 0 5 & ;;
    h-size5-enh)        run $c enhancer 0 5 & ;;
    i-detail-size8)     run $c "" 1 8 & ;;
    j-detail-size8-enh) run $c enhancer 1 8 & ;;
  esac
  n=$((n+1)); [ $((n % 3)) -eq 0 ] && wait
done
wait
