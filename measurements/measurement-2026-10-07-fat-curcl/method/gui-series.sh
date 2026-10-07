#!/usr/bin/env bash
# gui-series.sh <scratch> <prefix> -- the four GUI runs of FS08 part 2 with the
# injector kernels <scratch>/<prefix>kinj-{deskw,qedw,teraw,zvw}.img
# (make-inj.sh; fx- = this change, hd- = a `git archive HEAD` extract), each
# on a fresh APFS clone of base.img (prep-card.sh), memprot on (default),
# card checked on the host (card-check.py), fsck'd, hashed and deleted
# (run-gui.sh). One QEMU at a time.
set -uo pipefail
S="$1"; P="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TMPDIR="$S"
run() { # run <actions> <kind> <check-kind or -> <timeout>
  local a=$1 k=$2 c=$3 t=$4
  bash "$HERE/prep-card.sh" "$S" "$S/cards/$P$a.img" "$k" > /dev/null
  if [ "$c" = - ]; then unset CHECK; else export CHECK="python3 $HERE/card-check.py $c"; fi
  echo "=== $P$a"
  bash "$HERE/run-gui.sh" "$S" "$S/${P}kinj-$a.img" "$S/cards/$P$a.img" "$P$a" \
    --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout "$t" 2>&1 | grep -E "^run |HOSTCHECK|^==|Warning|differ|orphan|lost|Cross"
  grep -cE "\[EXC\]|PANIC|Memory violation" "$S/serial/$P$a-serial.log" | sed 's/^/exceptions in serial: /'
}
run deskw desk desk 240
run qedw qed qed 240
run teraw tera tera 240
run zvw zview - 300
