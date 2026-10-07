#!/usr/bin/env bash
# gui-stdio.sh <scratch> <label> -- the GUI regression runs of TASK/FS08 part 4
# with the injector kernels <scratch>/k-kinj-<actions>.img (make-inj.sh, the
# UNCHANGED kernel tree + the scripted IKBD injector), each on a fresh APFS
# clone of <scratch>/base-<label>.img (prep-card.sh; label before = the program
# builds of HEAD f14942c, after = rebuilt against the new libc), memprot on
# (default), SD trace counted (run-gui.sh TRACE=1), card checked on the host
# (card-check.py), fsck'd, hashed and deleted. One QEMU at a time.
#   zvw     zView (ZVLAUNCH) opens C:\PHOTO\BIG.JPG (2 273 801 B)   fat-curcl
#   zvs     zView browser -> C:\ZVIEW\SAMPLES thumbnails (15 files) zview02
#   qedw    QED new text, save as QEDT.TXT, edit, save again, quit  fat-curcl
#   teraw   TeraDesk copies DUMPVDI.NAP and the COREMARK folder      fat-curcl
#   ldg     LDGDEMO (WinDom client + EX_LIB.NLD + POPUP.NLD): Alt+C, long and
#           short click on Compute (popup path), typing, Close        zview02
# LDG_DIR_<label> names the LDGDEMO tree of that label.
set -uo pipefail
S="$1"; L="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TMPDIR="$S" TRACE=1 BASE="$S/base-$L.img" APPEND="serial=on memprot=on"
mkdir -p "$S/cards"
run() { # run <kernel actions> <kind> <check-kind or -> <timeout> <end marker> [mark-delay]
  local a=$1 k=$2 c=$3 t=$4 e=$5 md=${6:-0.5} tag="$L-$1"
  bash "$HERE/prep-card.sh" "$S" "$S/cards/$tag.img" "$k" > /dev/null
  if [ "$c" = - ]; then unset CHECK; else export CHECK="python3 $HERE/card-check.py $c"; fi
  echo "=== $tag"
  bash "$HERE/run-gui.sh" "$S" "$S/k-kinj-$a.img" "$S/cards/$tag.img" "$tag" \
    --mark-delay "$md" --end "\[m3\] MARK $e" --end-delay 1 --timeout "$t" 2>&1 \
    | grep -E "^run |HOSTCHECK|^==|Warning|differ|orphan|lost|Cross|read commands|blocks read"
  grep -cE "\[EXC\]|PANIC|Memory violation" "$S/serial/$tag-serial.log" | sed 's/^/exceptions in serial: /'
}
run zvw zview - 300 END
run zv-samples zvs - 240 END
run qedw qed qed 240 END
run teraw tera tera 240 END
LV="LDG_DIR_$L"; export LDG_DIR="${!LV:?set $LV}"
run evb-acc ldg - 150 END_CLOSED 0.3
