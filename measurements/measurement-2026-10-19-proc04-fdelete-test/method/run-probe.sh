#!/usr/bin/env bash
# run-probe.sh <side> <mode...> -- the P4F probe card on kernel8-<side>-plain, one QEMU per mode (on | strict | off | verify),
# 90 s max, ends on "desktop up" (the probe's "FD RESULT" comes before it); serial -> ../serial/fd-<side>-<mode>.log; the
# clone is fsck'ed (fsck_msdos -n, ../serial/fd-<side>-<mode>-fsck.txt) and listed (mtools) before it is deleted
# (regress/run.sh FSCK=1 CARDLS=1). Copied from measurement-2026-10-18-proc04-redir/method/run-probe.sh, probe renamed.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?head|wt}"; shift
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; verify) A="serial=on memprot=verify";; esac
  FSCK=1 CARDLS=1 bash "$HERE/regress/run.sh" "$S/bin/kernel8-$SIDE-plain.img" "$S/cards/fd.img" "fd-$SIDE-$m" "$A" 'desktop up' 90
  L="$HERE/../serial/fd-$SIDE-$m.log"
  echo "  $(grep -a 'FD RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[FD\] FAIL' "$L")  desktop: $(grep -ac 'desktop up' "$L")  fsck: $(grep -E 'rc=|files|clusters|ERR|Lost|lost' "$HERE/../serial/fd-$SIDE-$m-fsck.txt" | tr '\n' ' ')"
done
