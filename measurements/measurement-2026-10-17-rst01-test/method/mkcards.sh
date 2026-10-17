#!/usr/bin/env bash
# mkcards.sh -- the three base cards (APFS-cloned per run by run-qemu.sh):
#   desk.img : NEWDESK.INF = desk.inf (EmuDESK), nothing else               (keyboard runs)
#   td.img   : z-teradesk.inf (#Z autorun TeraDesk) + C:\Apps\TERADESK\*  (TeraDesk "Reboot")
#   probe.img: desk.inf + C:\AUTO\RSTPROBE.NCP                             (OSHEADER entry via Supexec)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
D="$WS/sdk/apps/teradesk/build/dist"; mkdir -p "$S/img"
bash "$HERE/mkcard.sh" "$S/img/desk.img" "$HERE/desk.inf"
bash "$HERE/mkcard.sh" "$S/img/td.img" "$HERE/z-teradesk.inf" Apps/TERADESK/DESKTOP.NAP="$D/DESKTOP.NAP" \
  Apps/TERADESK/desktop.rsc="$D/desktop.rsc" Apps/TERADESK/icons.rsc="$D/icons.rsc" Apps/TERADESK/cicons.rsc="$D/cicons.rsc" \
  Apps/TERADESK/TERADESK.INF="$HERE/TERADESK.INF"
bash "$HERE/mkcard.sh" "$S/img/probe.img" "$HERE/desk.inf" AUTO/RSTPROBE.NCP="$S/prog/RSTPROBE.NCP"
shasum -a 256 "$D"/* "$S/prog/RSTPROBE.NCP"
