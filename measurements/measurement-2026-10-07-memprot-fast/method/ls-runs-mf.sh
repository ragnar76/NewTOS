#!/usr/bin/env bash
# ls-runs-mf.sh <scratch root> <mode> [part...] -- MEMPROT-FAST: the LDG and
# zView runs of the protection series (ls-runs.sh of
# measurement-2026-10-06-libshare, same cards/scripts/programs) in one mode,
# with the verification mode on: mode = on | strict | off | on-asid ->
# -append "serial=on [memprot=strict|memprot=off] [asid=on] memprot=verify".
# Parts:
#   ldg     \AUTO\LDGTEST.NCP (69 checks)
#   popup   LDGDEMO.NAP + m3b.actions (Alt+F -> POPUP.NLD, Sinus/Cosinus, close)
#   accept2 LDGDEMO.NAP + evb-acc.actions (post-B1 acceptance script)
#   zview   #Z C:\ZVIEW\ZVIEW.NAP on the tools/make-sd-image.sh card
#           (<S>/base.img): ten codecs loaded (Pexec 3/4/5 + Mshrink each),
#           screendumps at 60/120 s
set -uo pipefail
S="${1:?scratch root}"; MODE="${2:?mode}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; C="$S/cards"; mkdir -p "$C"
case "$MODE" in on) A="";; strict) A="memprot=strict";; off) A="memprot=off";; on-asid) A="asid=on";; *) echo "mode? $MODE"; exit 2;; esac
export APPEND="$(echo "serial=on $A memprot=verify" | xargs)"
export SOCKROOT="${SOCKROOT:-$S/sk}"; mkdir -p "$SOCKROOT"
DEMO="LDGDEMO/CLIENT/LDGDEMO.NAP=$B/LDGDEMO/CLIENT/LDGDEMO.NAP LDGDEMO/CLIENT/CLIENT.RSC=$B/LDGDEMO/CLIENT/CLIENT.RSC LDGDEMO/LIBS/EX_LIB.NLD=$B/LDGDEMO/LIBS/EX_LIB.NLD LDGDEMO/LIBS/POPUP.NLD=$B/LDGDEMO/LIBS/POPUP.NLD"
sd() { [ -f "$C/$1.img" ] || EXTRA="${3:-}" BIN="$B" bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
rd() { bash "$HERE/run-rpi4-drive-m3.sh" "$@"; }
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
T="mf-$MODE"
for part in "$@"; do case "$part" in
ldg)
  sd ldgauto - "LDG/EX_LIB.NLD=$B/EX_LIB.NLD AUTO/LDGTEST.NCP=$B/LDGTEST.NCP"
  rd "$S/kernel8-ls.img" "$C/ldgauto.img" "$T-ldgauto" --end 'ldgtest: (ALL PASS|FAIL \()' --end-delay 2 --timeout 400
  ;;
popup)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  rd "$S/kinj-m3b.img" "$C/ldgdemo.img" "$T-popup" --mark-delay 0.3 --end '\[m3\] MARK END_CLOSED' --end-delay 3 --timeout 300
  ;;
accept2)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  rd "$S/kinj-evb-acc.img" "$C/ldgdemo.img" "$T-acc2" --mark-delay 0.3 --end '\[m3\] MARK END_CLOSED' --end-delay 3 --timeout 300
  ;;
zview)
  z="$C/zview.img"
  if [ ! -f "$z" ]; then
    cp -c "$S/base.img" "$z"
    sed "s|^#Z .*|#Z 01 C:\\\\ZVIEW\\\\ZVIEW.NAP@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK-ZV.INF"
    mcopy -o -i "$z@@$((63*512))" "$S/NEWDESK-ZV.INF" ::NEWDESK.INF
  fi
  r4 "$S/kernel8-ls.img" "$z" "$T-zview" 60:t60.png 120:t120.png
  ;;
*) echo "part? $part"; exit 2;;
esac; done
