#!/usr/bin/env bash
# keyowner-runs.sh <scratch root> <head|new> <part...> -- the QEMU raspi4b runs of
# TASK/KEYOWNER, serially, one QEMU at a time. Kernels from
# make-kernels-keyowner.sh (<S>/<P>-kinj-<actions>.img, <S>/<P>-kernel8-evb.img),
# base cards from make-bases-keyowner.sh (<S>/bases). EVERY run gets a private
# APFS clone (cp -c) of its base, deleted right after the run. Tags <P>-<part>.
# -append "serial=on" (+ memprot=strict / input=aes where named); memprot default on.
# Parts (what keyboard path each one drives):
#   sikeys   EmuDESK + SYSACC drop-down: Esc / Up+Return / Down+Esc (THE fix)
#   si       EmuDESK + SYSACC dialog open + Return closes      (accessory dialog form_do)
#   si-aes   the same with input=aes ("Mouse while busy" Running now = Off)
#   sitera / siqed   SYSACC over TeraDesk / QED, Return closes
#   emucon   EmuDESK Ctrl-Z -> EmuCON "ver" "dir" "exit"      (console Cconin path, accessories alive)
#   acc-on / acc-strict   LDGDEMO popup, alert, typing (memprot on / strict)
#   desk     EmuDESK menu/window              tera / qed   TeraDesk / QED menus
#   desk3a   EmuDESK Ctrl-I info dialog typing, rename, Alt-C (no accessory)
#   desk3a-acc   the same card + C:\ACC\SYSACC.NAC (accessory alive)
#   api      APITEST (plain kernel, timed dumps)
#   zv-dclick / zv-browser / zv-quit   zView (base64 card, accessories alive)
#   zvfull   zView full screen -> Esc -> Quit
#   doom     DOOM keys via kbdvec (Esc menu, Up/Return/Y quit)
#   deskw / qedw / qedo / teraw   FS writes driven by keys (Return confirms; QED typing + save)
set -uo pipefail
S="${1:?scratch root}"; P="${2:?head|new}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
D="$S/bases"; C="$S/cards"; mkdir -p "$C"
CHK="$HERE/../checks"; mkdir -p "$CHK"
export SOCKROOT="$S"
off() { python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$1"; }
inf() { # inf <card> <autorun or "">: C:\NEWDESK.INF = template with that #Z line (or none)
  local o; o=$(off "$1")
  if [ -n "$2" ]; then sed "s|^#Z .*|#Z 01 ${2//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  else grep -v '^#Z ' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"; fi
  mcopy -o -i "$1@@$o" "$S/NEWDESK.INF" ::NEWDESK.INF
}
m3() { # m3 <base> <actions> <tag> <autorun|-|keep> <append> [drive-m3 options]: clone, run, delete
  local base=$1 act=$2 tag=$3 auto=$4 app=$5; shift 5
  local card="$C/run-$tag.img"
  rm -f "$card"; cp -c "$base" "$card"
  case "$auto" in keep) ;; -) inf "$card" "" ;; *) inf "$card" "$auto" ;; esac
  APPEND="$app" bash "$HERE/run-rpi4-drive-m3.sh" "$S/$P-kinj-$act.img" "$card" "$tag" "$@"
  shasum -a 256 "$card" | sed "s|  .*|  run-$tag.img (deleted)|" >> "$CHK/run-cards-sha256.txt"
  rm -f "$card"
}
gui() { # gui <actions> <kind> <check-kind or -> <timeout>: fs09 prep-card + run-gui (check, fsck, delete)
  local a=$1 k=$2 c=$3 t=$4 tag="$P-$1"
  BASE="$D/base64.img" bash "$HERE/prep-card-fs09.sh" "$S" "$C/run-$tag.img" "$k" > /dev/null
  if [ "$a" = qedo ]; then local o; o=$(off "$C/run-$tag.img"); mcopy -o -i "$C/run-$tag.img@@$o" "$HERE/desk/NOTE.TXT" ::Apps/QED/NOTE.TXT; fi
  if [ "$c" = - ]; then unset CHECK; else export CHECK="python3 $HERE/$c"; fi
  APPEND="serial=on" bash "$HERE/run-gui-fs09.sh" "$S" "$S/$P-kinj-$a.img" "$C/run-$tag.img" "$tag" \
    --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout "$t" | grep -E "^run |HOSTCHECK|PASS|FAIL|^==|Warning|differ|orphan|lost|Cross"
  for f in hostcheck.txt fsck.txt; do [ -f "$S/out/$tag-$f" ] && cp "$S/out/$tag-$f" "$CHK/"; done
  [ -f "$S/out/$tag.card-sha256" ] && sed "s|  .*|  run-$tag.img (deleted)|" "$S/out/$tag.card-sha256" >> "$CHK/run-cards-sha256.txt"
  unset CHECK
}
for part in "$@"; do echo "=== $P-$part"; case "$part" in
  sikeys)  m3 "$D/base64.img" desk-sikeys "$P-sikeys" - "serial=on" --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 150 ;;
  si)      m3 "$D/base64.img" desk-si "$P-si" - "serial=on" --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 120 ;;
  si-aes)  m3 "$D/base64.img" desk-si "$P-si-aes" - "serial=on input=aes" --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 120 ;;
  sitera)  m3 "$D/base64.img" si-tera "$P-sitera" 'C:\Apps\TERADESK\DESKTOP.NAP' "serial=on" --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 150 ;;
  siqed)   m3 "$D/base64.img" si-qed "$P-siqed" 'C:\Apps\QED\QED.NAP' "serial=on" --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 150 ;;
  emucon)  m3 "$D/base64.img" desk-emucon "$P-emucon" - "serial=on" --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 150 ;;
  acc-on)  m3 "$D/ldgdemo.img" evb-acc "$P-acc-on" keep "serial=on" --mark-delay 0.3 --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150 ;;
  acc-strict) m3 "$D/ldgdemo.img" evb-acc "$P-acc-strict" keep "serial=on memprot=strict" --mark-delay 0.3 --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150 ;;
  desk)    m3 "$D/desk.img" evb-desk "$P-desk" keep "serial=on" --mark-delay 0.3 --end '\[m3\] MARK END' --end-delay 1 --timeout 150 ;;
  tera)    m3 "$D/teradesk.img" evb-tera "$P-tera" keep "serial=on" --mark-delay 0.3 --end '\[m3\] MARK END' --end-delay 1 --timeout 150 ;;
  qed)     m3 "$D/qed.img" evb-qed "$P-qed" keep "serial=on" --mark-delay 0.3 --end '\[m3\] MARK END' --end-delay 1 --timeout 150 ;;
  desk3a)  m3 "$D/desk3a.img" evb-desk3a "$P-desk3a" keep "serial=on" --mark-delay 0.5 --end '\[m3\] MARK H_DRIVE_C' --end-delay 1 --timeout 150 ;;
  desk3a-acc) m3 "$D/desk3a-acc.img" evb-desk3a "$P-desk3a-acc" keep "serial=on" --mark-delay 0.5 --end '\[m3\] MARK H_DRIVE_C' --end-delay 1 --timeout 150 ;;
  api)     APPEND="serial=on" bash "$HERE/run-rpi4.sh" "$S/$P-kernel8-evb.img" "$D/apitest.img" "$P-api" 25:t25.png 40:t40.png ;;
  zv-dclick|zv-browser|zv-quit)
           m3 "$D/base64.img" "$part" "$P-$part" 'C:\ZVIEW\ZVIEW.NAP' "serial=on" --end '\[m3\] MARK END' --end-delay 2 --timeout 240 ;;
  doom)    m3 "$D/doom.img" doom "$P-doom" 'C:\DOOM\DOOM.NAP' "serial=on" --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 400 ;;
  zvfull)  gui zvfull zview - 300 ;;
  deskw)   gui deskw desk "card-check.py desk" 240 ;;
  qedw)    gui qedw qed "card-check.py qed" 240 ;;
  teraw)   gui teraw tera "card-check.py tera" 240 ;;
  qedo)    gui qedo qed qedo-check.py 240 ;;
  *) echo "part? $part"; exit 2;;
esac; done
