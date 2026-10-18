#!/usr/bin/env bash
# cards-n.sh -- interactive-test base cards c-nd (desktop), c-nt (TeraDesk), c-nq (QED): folder tree C:\AFOLD\BSUB with a
# MARK.TXT (folder-specific content) and a cwdprobe program (CWDR/CWDA/CWDB) per level, a 12-char name LONGNAME.TXT.
# Private 64 MiB FAT16 images (label T113), never a real card.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; mkdir -p "$S/mk" "$S/img"
printf 'root-mark' > "$S/mk/M_R"; printf 'folder-A' > "$S/mk/M_A"; printf 'sub-B' > "$S/mk/M_B"; printf 'long name file\r\n' > "$S/mk/LN"
T="$WS/sdk/apps/teradesk/build/dist"; Q="$WS/sdk/apps/qed/build"; M="$HERE/m113"
F=(MARK.TXT="$S/mk/M_R" CWDR.NAP="$S/cwd/CWDR.NAP" AFOLD/MARK.TXT="$S/mk/M_A" AFOLD/CWDA.NAP="$S/cwd/CWDA.NAP"
   AFOLD/LONGNAME.TXT="$S/mk/LN" AFOLD/BSUB/MARK.TXT="$S/mk/M_B" AFOLD/BSUB/CWDB.NAP="$S/cwd/CWDB.NAP")
bash "$M/mkcard.sh" "$S/img/c-nd.img" "$M/desk.inf" "${F[@]}"
bash "$M/mkcard.sh" "$S/img/c-nt.img" "$M/z-teradesk.inf" "${F[@]}" Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" \
  Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" Apps/TERADESK/cicons.rsc="$T/cicons.rsc" \
  Apps/TERADESK/TERADESK.INF="$M/TERADESK.INF"
bash "$M/mkcard.sh" "$S/img/c-nq.img" "$M/z-qed.inf" "${F[@]}" Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc"
