#!/usr/bin/env bash
# series-apps.sh -- the application runs (after app-builds.sh and cards.sh), one QEMU at a time:
#   QED new/old  (kernel qfi = HEAD + IKBD injector up:QED, qfi.script of measurement-2026-10-21-sdk01f, the kernel's
#                 embedded script; copied unchanged): open C:\Apps\QED\DATED.TXT, Textinfo, edit, save twice, Textinfo
#   TeraDesk new (kernel tdv, tdv.script): dclick drive C -> the C:\ window in text mode. TeraDesk's DESKTOP.NAP is
#                 byte-identical before/after SDK01 J (checks/apps-*.sha256), so there is no "old" run.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for v in new old; do
  LSDIRS="/ /Apps/QED" MTYPE="/Apps/QED/DATED.TXT" DL=1.0 bash "$HERE/drive.sh" "$S/bin/kernel8-qfi.img" "$S/cards/qfi-$v.img" qfi-$v inj up:QED="$HERE/qfi.script"
done
LSDIRS="/" DL=1.0 bash "$HERE/drive.sh" "$S/bin/kernel8-tdv.img" "$S/cards/td-new.img" td-new inj up:DESKTOP="$HERE/tdv.script"
