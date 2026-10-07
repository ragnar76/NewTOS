#!/usr/bin/env bash
# sysinfo-runs.sh <scratch root> <kernel8.img> <label> <part...> -- QEMU
# raspi4b runs of TASK/SYSINFO01 (serial=on), one QEMU at a time. Adapted from
# measurement-2026-10-07-acctitle/method/acct-runs.sh (probe/zview parts
# dropped, the accessory probe no longer copied).
#
# Card: <S>/base.img = tools/make-sd-image.sh <S>/base.img 64 (QEMU image
# only; it builds + installs C:\ACC\SYSACC.NAC and C:\SYSACC2.NAC from
# sdk/examples/sysacc.c). Per run a private copy (cp -c) gets C:\NEWDESK.INF
# = newdesk-template.inf with the run's "#Z" line (or none), via mtools at
# the partition offset (LBA 63).
# Parts (all but api need the injector kernel of the named actions,
# make-inj.sh):
#   desk        EmuDESK, no #Z, desk.actions, default boot arguments
#   desk-alt    the same, -append "serial=on memprot=strict bcache=0
#               emmc=compat cpuclk=max"
#   desk-alt2   the same, -append "serial=on memprot=off asid=tiny
#               bcache=8192 bcachepct=20"
#   desk-de     the same, default arguments, NEWDESK.INF with "#K de"
#   tera        TeraDesk (#Z C:\Apps\TERADESK\DESKTOP.NAP), tera.actions
#   qed         QED (#Z C:\Apps\QED\QED.NAP), qed.actions
#   ldg         LDGDEMO (#Z, measurement-2026-10-06-ldg-m3/bin/LDGDEMO),
#               ldg.actions
#   api         APITEST (#Z C:\APITEST.NCP), plain kernel, dumps at 25/40 s
set -uo pipefail
S="${1:?scratch root}"; K="${2:?kernel}"; L="${3:?label}"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$S/base.img"; C="$S/cards"; mkdir -p "$C"
OFF=$((63 * 512))
WS="$(cd "$HERE/../../.." && pwd)"

card() { # card <out> <autorun or ""> [extra NEWDESK.INF line]
  local out=$1 auto=$2 extra=${3:-}
  rm -f "$out"; cp -c "$BASE" "$out"
  if [ -n "$auto" ]; then
    sed "s|^#Z .*|#Z 01 ${auto//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  else
    grep -v '^#Z ' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  fi
  [ -n "$extra" ] && printf '%s\r\n' "$extra" >> "$S/NEWDESK.INF"
  mcopy -o -i "$out@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
}
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive-m3.sh" "$@"; }
deskrun() { # deskrun <tag> <append> [extra]
  card "$C/desk.img" "" "${3:-}"
  APPEND="$2" rd "$K" "$C/desk.img" "$L-$1" --mark-delay 0.5 \
    --end '\[m3\] MARK END' --end-delay 1 --timeout 120
  rm -f "$C/desk.img"
}

for part in "$@"; do
  case "$part" in
  desk)      deskrun desk "serial=on" ;;
  desk-alt)  deskrun desk-alt "serial=on memprot=strict bcache=0 emmc=compat cpuclk=max" ;;
  desk-alt2) deskrun desk-alt2 "serial=on memprot=off asid=tiny bcache=8192 bcachepct=20" ;;
  desk-de)   deskrun desk-de "serial=on" "#K de" ;;
  tera|qed)
    [ $part = tera ] && P='C:\Apps\TERADESK\DESKTOP.NAP' || P='C:\Apps\QED\QED.NAP'
    card "$C/$part.img" "$P"
    APPEND=serial=on rd "$K" "$C/$part.img" "$L-$part" --mark-delay 0.5 \
      --end '\[m3\] MARK END' --end-delay 1 --timeout 150
    rm -f "$C/$part.img"
    ;;
  ldg)
    card "$C/ldg.img" 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP'
    B="$WS/measurements/measurement-2026-10-06-ldg-m3/bin/LDGDEMO"
    mmd -i "$C/ldg.img@@$OFF" ::LDGDEMO ::LDGDEMO/CLIENT ::LDGDEMO/LIBS
    for f in CLIENT/LDGDEMO.NAP CLIENT/CLIENT.RSC LIBS/EX_LIB.NLD LIBS/POPUP.NLD; do
      mcopy -o -i "$C/ldg.img@@$OFF" "$B/$f" "::LDGDEMO/$f"; done
    APPEND=serial=on rd "$K" "$C/ldg.img" "$L-ldg" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150
    rm -f "$C/ldg.img"
    ;;
  api)
    card "$C/api.img" 'C:\APITEST.NCP'
    mcopy -o -i "$C/api.img@@$OFF" "$WS/sdk/apps/apitest/APITEST.NCP" ::APITEST.NCP
    APPEND=serial=on r4 "$K" "$C/api.img" "$L-api" 25:t25.png 40:t40.png
    rm -f "$C/api.img"
    ;;
  *) echo "part? $part"; exit 2;;
  esac
done
