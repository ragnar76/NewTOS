#!/usr/bin/env bash
# acct-runs.sh <scratch root> <kernel8.img> <label> <part...> -- QEMU raspi4b
# runs of TASK/ACCTITLE (serial=on), one QEMU at a time.
#
# Card: <S>/base.img = tools/make-sd-image.sh <S>/base.img 64 (QEMU image
# only; it installs C:\ACC\SYSACC.NAC and C:\SYSACC2.NAC, the two desk
# accessories, and C:\ZVIEW\). Per run a private copy gets, via mtools at the
# partition offset (LBA 63):
#   C:\NEWDESK.INF   newdesk-template.inf with "#Z 01 <autorun>@"
#   C:\ACCPROBE.NAP <S>/ACCPROBE.NAP (build-acctprobe.sh)
# Parts:
#   probe-on       ACCTPROBE (#Z), memprot on (default)
#   probe-strict   ACCTPROBE (#Z), memprot=strict
#   zview-on|zview-strict  ZVLAUNCH (#Z, ../../measurement-2026-10-06-zview-m4/
#                  bin/ZVLAUNCH.NAP) starts the product C:\ZVIEW\ZVIEW.NAP
#                  (sha256 9c6497ad..., as on the real Pi) with
#                  C:\ZVIEW\SAMPLES\T.JPG; <kernel> must be an injector
#                  kernel built from zview.actions (make-inj.sh); screendump
#                  per injector MARK (drive-m3.py)
# Regression (memprot on; <kernel> = the injector kernel of the named
# actions, make-inj.sh; api takes a plain kernel):
#   desk   EmuDESK, no #Z line, desk.actions (Desk menu, accessory, alert)
#   tera   TeraDesk (#Z C:\Apps\TERADESK\DESKTOP.NAP), tera.actions
#   qed    QED (#Z C:\Apps\QED\QED.NAP), qed.actions
#   ldg    LDGDEMO (#Z, the measurement-2026-10-06-ldg-m3/bin/LDGDEMO tree
#          copied to C:\LDGDEMO), ldg.actions
#   api    APITEST (#Z C:\APITEST.NCP, sdk/apps/apitest), dumps at 25/40 s
set -uo pipefail
S="${1:?scratch root}"; K="${2:?kernel}"; L="${3:?label}"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$S/base.img"; C="$S/cards"; mkdir -p "$C"
OFF=$((63 * 512))

card() { # card <out> <autorun>
  local out=$1 auto=$2
  cp "$BASE" "$out"
  sed "s|^#Z .*|#Z 01 ${auto//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mcopy -o -i "$out@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
  mcopy -o -i "$out@@$OFF" "$S/ACCPROBE.NAP" ::ACCPROBE.NAP
}
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive-m3.sh" "$@"; }
WS="$(cd "$HERE/../../.." && pwd)"
zcard() { # zcard <out>
  card "$1" 'C:\ZVLAUNCH.NAP'
  mcopy -o -i "$1@@$OFF" "$WS/measurements/measurement-2026-10-06-zview-m4/bin/ZVLAUNCH.NAP" ::ZVLAUNCH.NAP
  printf '%s\r\n' 'C:\ZVIEW\SAMPLES\T.JPG' > "$S/ZVARG.TXT"
  mcopy -o -i "$1@@$OFF" "$S/ZVARG.TXT" ::ZVARG.TXT
}

for part in "$@"; do
  case "$part" in
  probe-on)
    card "$C/probe.img" 'C:\ACCPROBE.NAP'
    APPEND=serial=on r4 "$K" "$C/probe.img" "$L-probe-on" 20:t20.png
    ;;
  probe-strict)
    card "$C/probe.img" 'C:\ACCPROBE.NAP'
    APPEND="serial=on memprot=strict" r4 "$K" "$C/probe.img" "$L-probe-strict" 20:t20.png
    ;;
  zview-on|zview-strict)
    M=${part#zview-}; A="serial=on"; [ "$M" != on ] && A="serial=on memprot=$M"
    zcard "$C/zview.img"
    APPEND="$A" rd "$K" "$C/zview.img" "$L-zview-$M" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 120
    ;;
  desk)
    card "$C/desk.img" 'C:\ACCPROBE.NAP'
    grep -v '^#Z ' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
    mcopy -o -i "$C/desk.img@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
    APPEND=serial=on rd "$K" "$C/desk.img" "$L-desk" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 120
    ;;
  tera|qed)
    [ $part = tera ] && P='C:\Apps\TERADESK\DESKTOP.NAP' || P='C:\Apps\QED\QED.NAP'
    card "$C/$part.img" "$P"
    APPEND=serial=on rd "$K" "$C/$part.img" "$L-$part" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 150
    ;;
  ldg)
    card "$C/ldg.img" 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP'
    B="$WS/measurements/measurement-2026-10-06-ldg-m3/bin/LDGDEMO"
    mmd -i "$C/ldg.img@@$OFF" ::LDGDEMO ::LDGDEMO/CLIENT ::LDGDEMO/LIBS
    for f in CLIENT/LDGDEMO.NAP CLIENT/CLIENT.RSC LIBS/EX_LIB.NLD LIBS/POPUP.NLD; do
      mcopy -o -i "$C/ldg.img@@$OFF" "$B/$f" "::LDGDEMO/$f"; done
    APPEND=serial=on rd "$K" "$C/ldg.img" "$L-ldg" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150
    ;;
  api)
    card "$C/api.img" 'C:\APITEST.NCP'
    mcopy -o -i "$C/api.img@@$OFF" "$WS/sdk/apps/apitest/APITEST.NCP" ::APITEST.NCP
    APPEND=serial=on r4 "$K" "$C/api.img" "$L-api" 25:t25.png 40:t40.png
    ;;
  *) echo "part? $part"; exit 2;;
  esac
done
