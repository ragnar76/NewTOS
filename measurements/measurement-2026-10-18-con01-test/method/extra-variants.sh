# sourced by build-all.sh: extra kernel variants (uses $I, $SC, $HERE, $WS, $T)
extra_variant() {
  case "$1" in
    kqt) $I --tag kn up:QED="$SC/qtype.script";;
    ktt) $I --tag kn up:DESKTOP="$SC/tkeys.script";;
    kcad) $I --tag kd desktop="$SC/cad.script";;
    kdoom) $I --tag kd launch:DOOM="$SC/doomkeys.script";;
    kdk) $I --tag kd desktop="$SC/deskkeys.script";;
    kapip) $I --tag kd launch:APITEST="$SC/apiprobe.script";;
    kapik) $I --tag kd launch:APITEST="$SC/apikey.script";;
    *) echo "unknown variant $1"; exit 2;;
  esac
}
