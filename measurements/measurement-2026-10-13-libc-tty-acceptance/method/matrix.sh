#!/usr/bin/env bash
# matrix.sh -- per item and mode: the verdict markers found in serial/<label>-<mode>-rpi4-<item>-serial.log
cd "$(dirname "${BASH_SOURCE[0]}")/../serial"
v() { tr -d '\r' < "$1" 2>/dev/null | grep -a -o -E "$2" | sort -u | tr '\n' '|'; }
row() { # row <label> <item> <regex>
  printf '%-18s' "$2"
  for m in on off strict on-asid; do printf ' %s=[%s]' "$m" "$(v "$1-$m-rpi4-$2-serial.log" "$3")"; done; echo
}
for it in fseekcon atexit e9mem e9plain accfree vforkt-alone envrn ppzj kbdvec-emucon; do
  row cm $it '(FSEEKCON|ATEXIT|E9MEM|E9PLAIN|ACCFREE|VFORK|ENVRN|PPARENT [A-Z]+|S2ALL|MEM03|KBDVEC|port10): ?(PASS|FAIL|ALL PASS \([0-9]+ checks\))'
done
for it in apitest; do row cm $it 'TOTAL +[0-9]+ +[0-9]+ +[0-9]+'; done
for it in doom; do row cm $it 'VERIFIED via [A-Za-z+_]+'; done
for it in crashtst uacc3a mprot e10 mdprobe emucon9 asidstr asidperf; do
  row lm $it '(CRASHTST|UACC3A|MPROT3B|E10USR|ASIDSTR|ASIDPERF|E9MEM|E9PLAIN|KBDVEC): (PASS|FAIL)|MDPROBE: DONE'
done
row lm pparent 'PPARENT [A-Z]+: (PASS|FAIL)|(AUTO|ACC|Z|KID): (PASS|FAIL)'
row lm ppdeskj '(DESK|KID): (PASS|FAIL)'
row lm s2all 'S2ALL: (PASS|FAIL)|MEM03: (PASS|FAIL)'
row lm port10 'port10: (ALL PASS \([0-9]+ checks\)|[0-9]+ of [0-9]+ checks FAILED)'
for m in on off on-asid; do printf 's2allm %-8s' $m; v cm-$m-rpi4-s2allm-serial.log 'S2ALL: (PASS|FAIL)|MEM03: (PASS|FAIL)'; echo; done
printf 's2allsm strict  '; v cm-strict-rpi4-s2allsm-serial.log 'S2ALL: (PASS|FAIL)|MEM03: (PASS|FAIL)'; echo
printf 's2alls (crash kernel) strict  '; v lm-strict-rpi4-s2alls-crashkernel-serial.log 'S2ALL: (PASS|FAIL)|MEM03: (PASS|FAIL)'; echo
