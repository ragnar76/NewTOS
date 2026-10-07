#!/usr/bin/env bash
# evb-runs.sh <scratch root> [part...] -- (ZVIEW02: accept honours KP too; SOCKROOT=<S> required)
# ZVIEW04 copy: api runs ${KP}kernel8-evb.img (head- = HEAD 55eddb8f, fix- = this change), tag ${KP}api
# QEMU raspi4b runs of TASK/LDG-M3-B1
# (-append serial=on; memprot default = on, strict where named). Kernels:
# make-kernels.sh (same scratch root). Cards: seed-card.sh (from
# measurement-2026-10-06-ldg-m3, unchanged), private 64 MiB images in
# <S>/cards, never tools/make-sd-image.sh. Program set BIN = the committed
# measurement-2026-10-06-ldg-m3/bin (read only: LDGDEMO tree, DTACHILD.NCP).
# Parts:
#   accept  LDGDEMO (WinDom client), evb-acc.actions, memprot on and strict
#   desk    EmuDESK (no #Z), evb-desk.actions
#   deskdrag EmuDESK (no #Z), evb-desk-drag.actions (K1 run: added -- the
#           drag outline on the AES's workstation; kernels with
#           ONLY=evb-desk-drag make-kernels.sh)
#   tera    TeraDesk (#Z), evb-tera.actions
#   qed     QED (#Z), evb-qed.actions
#   api     APITEST (#Z), no input, screendumps at 25/40 s
#   desk3a  EmuDESK, the ARCH03 stage-3a desktop script (folder double click,
#           closer, select, Ctrl-I info dialog + typing (objc_edit), rename,
#           Alt-C) = evb-desk3a.actions (copy of desk3a/desk3a.actions), card by
#           seed-card-a4.sh with the arch03-4 program set (read only)
# KP=head- runs the baseline injector kernels (<S>/head-kinj-*.img, built by
# make-kernels.sh with SRC=<git archive HEAD> PFX=head-); tags get the prefix.
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
B="$WS/measurements/measurement-2026-10-06-ldg-m3/bin"; C="$S/cards"; mkdir -p "$C"
DEMO="LDGDEMO/CLIENT/LDGDEMO.NAP=$B/LDGDEMO/CLIENT/LDGDEMO.NAP LDGDEMO/CLIENT/CLIENT.RSC=$B/LDGDEMO/CLIENT/CLIENT.RSC LDGDEMO/LIBS/EX_LIB.NLD=$B/LDGDEMO/LIBS/EX_LIB.NLD LDGDEMO/LIBS/POPUP.NLD=$B/LDGDEMO/LIBS/POPUP.NLD"
sd() { [ -f "$C/$1.img" ] || EXTRA="${3:-}" BIN="$B" bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
rd() { bash "$HERE/run-rpi4-drive-m3.sh" "$@"; }
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
for part in "$@"; do case "$part" in
accept)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  for M in on strict; do
    A="serial=on"; [ $M != on ] && A="serial=on memprot=$M"
    APPEND="$A" rd "$S/${KP:-}kinj-evb-acc.img" "$C/ldgdemo.img" "${KP:-}acc-$M" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150
  done
  ;;
desk)
  [ -f "$C/desk.img" ] || INF="$HERE/newdesk-evb-desk.inf" BIN="$B" bash "$HERE/seed-card.sh" "$C/desk.img" - > /dev/null
  APPEND=serial=on rd "$S/${KP:-}kinj-evb-desk.img" "$C/desk.img" ${KP:-}desk --mark-delay 0.3 \
    --end '\[m3\] MARK END' --end-delay 1 --timeout 150
  ;;
deskdrag)
  [ -f "$C/desk.img" ] || INF="$HERE/newdesk-evb-desk.inf" BIN="$B" bash "$HERE/seed-card.sh" "$C/desk.img" - > /dev/null
  APPEND=serial=on rd "$S/${KP:-}kinj-evb-desk-drag.img" "$C/desk.img" ${KP:-}deskdrag --mark-delay 0.3 \
    --end '\[m3\] MARK END' --end-delay 1 --timeout 150
  ;;
tera)
  sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'
  APPEND=serial=on rd "$S/${KP:-}kinj-evb-tera.img" "$C/teradesk.img" ${KP:-}tera --mark-delay 0.3 \
    --end '\[m3\] MARK END' --end-delay 1 --timeout 150
  ;;
qed)
  sd qed 'C:\Apps\QED\QED.NAP'
  APPEND=serial=on rd "$S/${KP:-}kinj-evb-qed.img" "$C/qed.img" ${KP:-}qed --mark-delay 0.3 \
    --end '\[m3\] MARK END' --end-delay 1 --timeout 150
  ;;
desk3a)
  A4="$WS/measurements/measurement-2026-10-06-arch03-4"
  [ -f "$C/desk3a.img" ] || BIN="$A4/bin" INF="$HERE/desk3a/newdesk-desk3a.inf" \
    EXTRA="D3A/SUB3A/INSIDE.TXT=$HERE/desk3a/INSIDE.TXT D3A/NOTE.TXT=$HERE/desk3a/NOTE.TXT D3A/HELLO.NCP=$A4/bin/HELLO.NCP" \
    bash "$HERE/seed-card-a4.sh" "$C/desk3a.img" - > /dev/null
  APPEND=serial=on rd "$S/${KP:-}kinj-evb-desk3a.img" "$C/desk3a.img" ${KP:-}desk3a --mark-delay 0.5 \
    --end '\[m3\] MARK H_DRIVE_C' --end-delay 1 --timeout 150
  ;;
api)
  sd apitest 'C:\APITEST.NCP'
  APPEND=serial=on r4 "$S/${KP#head-}kernel8-evb.img" "$C/apitest.img" ${KP:-}api 25:t25.png 40:t40.png
  ;;
*) echo "part? $part"; exit 2;;
esac; done
