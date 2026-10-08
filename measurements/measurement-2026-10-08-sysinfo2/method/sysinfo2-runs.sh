#!/usr/bin/env bash
# sysinfo2-runs.sh <scratch root> <label> <part...> -- QEMU raspi4b runs of
# TASK/SYSINFO02 (serial=on), one QEMU at a time. Adapted from
# measurement-2026-10-07-sysinfo/method/sysinfo-runs.sh.
#
# Card: <S>/base.img = tools/make-sd-image.sh <S>/base.img 64 (QEMU image
# only; it builds + installs C:\ACC\SYSACC.NAC and C:\SYSACC2.NAC from
# sdk/examples/sysacc.c). Per run a private APFS clone (cp -c) gets
# C:\NEWDESK.INF = newdesk-template.inf with the run's "#Z" line (or none)
# and, where named, a seeded C:\CMDLINE.TXT (cmdline-seed.txt), via mtools
# at the partition offset (LBA 63). The clone is deleted right after the run,
# after the files to check were copied out to ../checks/.
# Injector kernels: make-inj-input01.sh <S> <tree> k- <actions> -> <S>/k-kinj-<actions>.img
# Parts:
#   open      EmuDESK, desk.actions (open the dialog, Return), seeded file
#   edit      EmuDESK, edit.actions (two drop-downs, Esc test, Save, OK),
#             seeded file; copies C:\CMDLINE.TXT/.BAK out; leaves
#             <S>/after-edit.img for "reboot"
#   reboot    the card after "edit", -append = the saved CMDLINE.TXT line,
#             desk.actions (open, Return)
#   tera|qed  TeraDesk / QED (#Z), tera/qed.actions, no CMDLINE.TXT
#   ldg       LDGDEMO (#Z), ldg.actions
#   api       APITEST (#Z C:\APITEST.NCP), plain kernel <S>/kernel8-plain.img
set -uo pipefail
S="${1:?scratch root}"; L="${2:?label}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$S/base.img"; C="$S/cards"; mkdir -p "$C"
OFF=$((63 * 512))
WS="$(cd "$HERE/../../.." && pwd)"
CHK="$HERE/../checks"; mkdir -p "$CHK"

card() { # card <out> <autorun or ""> [seed: 1 = C:\CMDLINE.TXT from cmdline-seed.txt]
  local out=$1 auto=$2 seed=${3:-0}
  rm -f "$out"; cp -c "$BASE" "$out"
  if [ -n "$auto" ]; then
    sed "s|^#Z .*|#Z 01 ${auto//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  else
    grep -v '^#Z ' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  fi
  mcopy -o -i "$out@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
  [ "$seed" = 1 ] && mcopy -o -i "$out@@$OFF" "$HERE/cmdline-seed.txt" ::CMDLINE.TXT
  return 0
}
rd() { bash "$HERE/run-rpi4-drive-m3.sh" "$@"; }
pull() { # pull <card> <prefix>: C:\CMDLINE.* -> checks/<prefix>-*
  mdir -i "$1@@$OFF" :: > "$CHK/$2-rootdir.txt" 2>&1
  for f in CMDLINE.TXT CMDLINE.BAK CMDLINE.NEW; do
    if mcopy -o -i "$1@@$OFF" "::$f" "$CHK/$2-$f" 2>/dev/null; then
      { echo "== $f ($(wc -c < "$CHK/$2-$f" | tr -d ' ') bytes)"; xxd "$CHK/$2-$f"; } >> "$CHK/$2-files.txt"
    else
      echo "== $f: absent" >> "$CHK/$2-files.txt"
    fi
  done
}

for part in "$@"; do
  case "$part" in
  open)
    card "$C/open.img" "" 1
    APPEND=serial=on rd "$S/k-kinj-desk.img" "$C/open.img" "$L-open" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 120
    rm -f "$C/open.img" ;;
  edit)
    card "$C/edit.img" "" 1
    rm -f "$CHK/$L-edit-files.txt"
    APPEND=serial=on rd "$S/k-kinj-edit.img" "$C/edit.img" "$L-edit" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 200
    pull "$C/edit.img" "$L-edit"
    mv "$C/edit.img" "$S/after-edit.img" ;;
  reboot)
    A="$(tr -d '\r\n' < "$CHK/$L-edit-CMDLINE.TXT")"
    echo "reboot -append: [$A]" | tee "$CHK/$L-reboot-append.txt"
    APPEND="$A" rd "$S/k-kinj-desk.img" "$S/after-edit.img" "$L-reboot" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 120
    rm -f "$S/after-edit.img" ;;
  tera|qed)
    [ $part = tera ] && P='C:\Apps\TERADESK\DESKTOP.NAP' || P='C:\Apps\QED\QED.NAP'
    card "$C/$part.img" "$P"
    APPEND=serial=on rd "$S/k-kinj-$part.img" "$C/$part.img" "$L-$part" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 150
    rm -f "$C/$part.img" ;;
  ldg)
    card "$C/ldg.img" 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP'
    B="$WS/measurements/measurement-2026-10-06-ldg-m3/bin/LDGDEMO"
    mmd -i "$C/ldg.img@@$OFF" ::LDGDEMO ::LDGDEMO/CLIENT ::LDGDEMO/LIBS
    for f in CLIENT/LDGDEMO.NAP CLIENT/CLIENT.RSC LIBS/EX_LIB.NLD LIBS/POPUP.NLD; do
      mcopy -o -i "$C/ldg.img@@$OFF" "$B/$f" "::LDGDEMO/$f"; done
    APPEND=serial=on rd "$S/k-kinj-ldg.img" "$C/ldg.img" "$L-ldg" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150
    rm -f "$C/ldg.img" ;;
  api)
    card "$C/api.img" 'C:\APITEST.NCP'
    mcopy -o -i "$C/api.img@@$OFF" "$WS/sdk/apps/apitest/APITEST.NCP" ::APITEST.NCP
    APPEND=serial=on bash "$HERE/run-rpi4.sh" "$S/kernel8-plain.img" "$C/api.img" "$L-api" 25:t25.png 40:t40.png
    rm -f "$C/api.img" ;;
  *) echo "part? $part"; exit 2;;
  esac
done
