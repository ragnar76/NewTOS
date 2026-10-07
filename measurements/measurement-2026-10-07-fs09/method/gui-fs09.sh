#!/usr/bin/env bash
# gui-fs09.sh <scratch> <label> [extra boot args] -- FS09 GUI write/read
# regression with the injector kernels <scratch>/fx-kinj-<actions>.img
# (make-inj.sh over the tree WITH the block cache), each on a fresh APFS clone
# of <scratch>/base.img (prep-card.sh), memprot on, SD trace counted per run
# and per [m3] MARK segment (run-gui.sh TRACE=1, drive-m3.py --stamp),
# card checked on the host (card-check.py), fsck'd, hashed and deleted.
# Label "on" = the default cache (32 MiB, 5 %); "off" = "bcache=0" (the
# pre-FS09 path); "tiny" = "bcache=2048 bcachepct=100" (2 MiB, every run
# inserted: heavy eviction during the writes). One QEMU at a time.
#   deskw   EmuDESK copy / move / delete / folder copy / folder delete   fat-curcl
#   qedw    QED new text, save as QEDT.TXT, edit, save again, quit        fat-curcl
#   teraw   TeraDesk copies DUMPVDI.NAP and the COREMARK folder          fat-curcl
#   zvw     zView (ZVLAUNCH) opens C:\PHOTO\BIG.JPG (2 273 801 B)         fat-curcl
#   deskzv2 zView started twice from EmuDESK (codec loading twice)        FS09
set -uo pipefail
S="$1"; L="$2"; EXTRA="${3:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TMPDIR="$S" TRACE=1 APPEND="serial=on memprot=on${EXTRA:+ $EXTRA}"
mkdir -p "$S/cards"
run() { # run <kernel actions> <kind> <check-kind or -> <timeout>
  local a=$1 k=$2 c=$3 t=$4 tag="$L-$1"
  bash "$HERE/prep-card.sh" "$S" "$S/cards/$tag.img" "$k" > /dev/null
  if [ "$c" = - ]; then unset CHECK; else export CHECK="python3 $HERE/card-check.py $c"; fi
  echo "=== $tag ($APPEND)"
  bash "$HERE/run-gui.sh" "$S" "$S/fx-kinj-$a.img" "$S/cards/$tag.img" "$tag" --stamp \
    --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout "$t" 2>&1 \
    | grep -E "^run |HOSTCHECK|^==|Warning|differ|orphan|lost|Cross|read commands|blocks read"
  grep -a "\[bcache\]" "$S/serial/$tag-serial.log"
  grep -caE "\[EXC\]|PANIC|Memory violation" "$S/serial/$tag-serial.log" | sed 's/^/exceptions in serial: /'
}
run deskw desk desk 240
run qedw qed qed 240
run teraw tera tera 240
run zvw zview - 300
run deskzv2 zv2 - 300
