# mode.sh -- sourced by the ARCH03-4 run scripts with M set to the run mode:
#   <base>[-asid|-tiny]   base = on | off | strict (memory protection: none,
#   memprot=off, memprot=strict), suffix -asid = asid=on, -tiny = asid=tiny.
# Sets BASE, A4 (raspi4b -append, always serial=on), AV (virt -append) and
# exports APPEND=$A4.
BASE="${M%%-*}"
case "$M" in
  *-asid) X="asid=on";;
  *-tiny) X="asid=tiny";;
  "$BASE") X="";;
  *) echo "mode? $M"; exit 2;;
esac
case "$BASE" in
  on) P="";;
  off) P="memprot=off";;
  strict) P="memprot=strict";;
  verify) P="memprot=verify";;  # MEM05 run: the verification mode (copy extended)
  *) echo "mode? $M"; exit 2;;
esac
AV="$(echo "$P $X" | xargs)"
A4="$(echo "serial=on $P $X" | xargs)"
export APPEND="$A4"
