#!/usr/bin/env bash
# series-hellogem.sh -- DESK43 U9-0 (pattern of measurement-2026-10-14-desk43-u8b/method/series-hellogem.sh): HELLOGEM and
# its panic twin, started by #Z (sh_ldapp AUTORUN arm) and by F1 from the desktop (NORMAL arm via the desktop's
# shel_write), with the exit code now in the shell's log line (PROC05). Cards (seed-u6a.sh, fresh, /tmp/m5j/u90):
#   card-z.img      #Z 01 C:\HELLOGEM.NAP@      card-f1.img      F1 = C:\HELLOGEM.NAP
#   card-zpanic.img #Z 01 C:\HGPANIC.NAP@       card-f1panic.img F1 = C:\HGPANIC.NAP
# Kernels from build-hg.sh. z/zpanic in memprot on/off/strict, f1/f1panic in on. One QEMU at a time.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; C=/tmp/m5j/u90
bash "$HERE/seed-u6a.sh" "$C/card-z.img" "$HERE/newdesk-z.inf" "$B/HELLOGEM.NAP"
bash "$HERE/seed-u6a.sh" "$C/card-f1.img" "$HERE/newdesk-f1.inf" "$B/HELLOGEM.NAP"
bash "$HERE/seed-u6a.sh" "$C/card-zpanic.img" "$HERE/newdesk-zpanic.inf" "$B/HGPANIC.NAP"
bash "$HERE/seed-u6a.sh" "$C/card-f1panic.img" "$HERE/newdesk-f1panic.inf" "$B/HGPANIC.NAP"
for M in on off strict; do
  case $M in on) P="";; off) P="memprot=off";; strict) P="memprot=strict";; esac
  for c in z zpanic; do
    APPEND="$(echo "serial=on $P" | xargs)" "$HERE/run-u6a.sh" "$B/kernel8-inj-z.img" "$C/card-$c.img" "hg-inj-$M-$c" --timeout 240
  done
done
for c in f1 f1panic; do
  APPEND="serial=on" "$HERE/run-u6a.sh" "$B/kernel8-inj-f1.img" "$C/card-$c.img" "hg-inj-on-$c" --timeout 240
done
