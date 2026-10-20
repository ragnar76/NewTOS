#!/usr/bin/env bash
# cards.sh -- base cards of this run (private 64 MiB FAT16 images in the scratch dir, never a real card; runs clone them).
# Every card carries C:\KERNEL8.IMG = a 1-line text marker stamped 2026-10-21 10:00: the rpi4 kernel seeds its clock from the FAT
# stamp of C:\KERNEL8.IMG (kernel/src/lib.rs seed_clock_from_kernel_file); the booted kernel itself comes from QEMU -kernel.
#   st-<v>   C:\AUTO\STATT.NCP (probe, v = new|old libc), C:\DATED.TXT stamped 2026-10-17 12:00, C:\SUB\A.TXT, empty C:\TMP, sp.inf
#   qfi-<v>  QED #Z (z-qed.inf) + Apps/QED/{QED.NAP,qed.rsc} of variant v + Apps/QED/DATED.TXT stamped 2026-10-17 12:00
#   td-<v>   TeraDesk #Z (z-teradesk.inf) + Apps/TERADESK/{DESKTOP.NAP,*.rsc} of v, TERADESK.INF = TERADESK-TEXT.INF (text mode),
#            data files DATED.TXT (2026-10-17 12:00), OLD.TXT (2001-02-03 04:05), SUB/NOTE.TXT
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; M="$S/mk"; mkdir -p "$C" "$M"
printf 'clock stamp marker, not a kernel\r\n' > "$M/KERNEL8.IMG"; printf 'dated file\r\n' > "$M/DATED.TXT"
printf 'file A\r\n' > "$M/A.TXT"; printf 'old file\r\n' > "$M/OLD.TXT"; printf 'note\r\n' > "$M/NOTE.TXT"
K=KERNEL8.IMG="$M/KERNEL8.IMG@202610211000"
for v in new old; do
  if [ $v = new ]; then SD="$WS/sdk"; else SD="$S/old/sdk"; fi
  Q="$SD/apps/qed/build"; T="$SD/apps/teradesk/build/dist"
  DIRS="SUB TMP" bash "$HERE/mkcard.sh" "$C/st-$v.img" "$HERE/sp.inf" "$K" AUTO/STATT.NCP="$S/prog/STATT-$v.NCP" \
    DATED.TXT="$M/DATED.TXT" SUB/A.TXT="$M/A.TXT" > "$C/st-$v.log" 2>&1
  DIRS="" bash "$HERE/mkcard.sh" "$C/qfi-$v.img" "$HERE/z-qed.inf" "$K" Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc" \
    Apps/QED/DATED.TXT="$M/DATED.TXT" > "$C/qfi-$v.log" 2>&1
  DIRS="" bash "$HERE/mkcard.sh" "$C/td-$v.img" "$HERE/z-teradesk.inf" "$K" Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" \
    Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" Apps/TERADESK/cicons.rsc="$T/cicons.rsc" \
    Apps/TERADESK/TERADESK.INF="$HERE/TERADESK-TEXT.INF" DATED.TXT="$M/DATED.TXT" OLD.TXT="$M/OLD.TXT@200102030405" \
    SUB/NOTE.TXT="$M/NOTE.TXT" > "$C/td-$v.log" 2>&1
done
ls -la "$C"/*.img | awk '{print $5, $9}'
