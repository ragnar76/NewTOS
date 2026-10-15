#!/usr/bin/env bash
# cmpprobes.sh -- probe programs: verdict line(s) per run, and the program output (kernel-side prefixed lines removed, hex-normalised) of
# p-el0-<mode> vs p-el1-<mode> and of p-el0-<mode> vs the MEM05 acceptance reference (acc-<mode>-rpi4-<name>-serial.log, 2026-10-13).
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; R="$HERE/../../measurement-2026-10-13-mem05-acceptance/serial"
flt() { tr -d '\r' < "$1" | grep -avE '^\[(mm|emmc|bcache|fb|proc|aes|launch|shell|desk|exec|clock|pcie|input|bios|fat|boot|kernel|bdos|asid|memprot|cpu|irq|usb|gic|timer|vdi|ikbd|time|sd|serial|drive|boot|ser|net|mbox|rom|cookie|u6a|kp)[]: ]|^\[[a-z0-9-]*\] (EXC|crash)' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/[0-9a-f]{8,}/H/g; s/\([0-9.]+ ?s\)//g; s/ in [0-9.]+ ?(ms|s)//g'; }
verd() { tr -d '\r' < "$1" | grep -aE 'PASS|FAIL|DONE|checks|TOTAL' | grep -avE '^\[desk-' | sed -E 's/0x[0-9a-fA-F]+/0xX/g' | tail -${V:-3} | cut -c1-110 | tr '\n' '|'; }
for f in "$S"/p-el0-*-rpi4-*-serial.log; do t=$(basename "$f" -serial.log); m=${t#p-el0-}; m=${m%%-rpi4-*}; n=${t#*-rpi4-}
  rn=$n; [ "$n" = port10 ] && rn=port10-rerun
  r="$R/acc-$m-rpi4-$rn-serial.log"; e1="$S/p-el1-$m-rpi4-$n-serial.log"
  if [ -f "$r" ]; then a=$(diff <(flt "$r") <(flt "$f") | grep -c '^[<>]'); else a=n/a; fi
  if [ -f "$e1" ]; then b=$(diff <(flt "$e1") <(flt "$f") | grep -c '^[<>]'); else b=n/a; fi
  echo "$m $n: EXC/PANIC-in-log=$(grep -acE '\[EXC\]|PANIC' "$f") | verdict: $(verd "$f") | out-diff vs EL1: $b, vs mem05-ref: $a"
done
