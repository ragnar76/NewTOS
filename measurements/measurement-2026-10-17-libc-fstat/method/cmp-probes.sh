#!/usr/bin/env bash
# cmp-probes.sh -- LIBC-FSTAT: per mode and probe, the probe's own serial lines of the B run (relinked, LIBC-FSTAT libc.a) vs the A run
# (pre-LIBC-FSTAT programs) on the same kernel, hex values >= 0x00100000 masked (addresses); verdict lines; EXC/PANIC counts.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"
pat() { case $1 in fstatp) echo 'FSTATP';; stdoutdb) echo '\[DBG\]|DBG:';; serr-*) echo 'STDERRP';; stderra) echo 'STDERRA';; ttya) echo 'TTY|kid file bytes';; esac; }
mask() { tr -d '\r' | sed -E 's/0x(00[1-9a-f][0-9a-f]{5}|0[1-9a-f][0-9a-f]{6}|[1-9a-f][0-9a-f]{7})/0xADDR/g'; }
for m in on off strict; do
  for p in fstatp stdoutdb serr-newcrt0 serr-oldcrt0 stderra ttya; do
    b="$S/p-$m-$p-serial.log"; a="$S/p-$m-a-$p-serial.log"; r="$(pat $p)"
    grep -aE "$r" "$b" | mask > "$HERE/../checks/.b"; grep -aE "$r" "$a" | mask > "$HERE/../checks/.a"
    n=$(diff "$HERE/../checks/.a" "$HERE/../checks/.b" | grep -c '^[<>]')
    echo "== $m $p: B $(wc -l < "$HERE/../checks/.b" | tr -d ' ') lines, A $(wc -l < "$HERE/../checks/.a" | tr -d ' ') lines, $n differing; exc/panic B $(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$b") A $(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$a")"
    grep -aE 'FSTATP: |TTYT[0-9]: |STDERR[AP]\[PAR\]: DONE|DBG: DONE' "$b" | tr -d '\r' | sed 's/^/   B /'
    grep -aE 'FSTATP: |TTYT[0-9]: |STDERR[AP]\[PAR\]: DONE|DBG: DONE' "$a" | tr -d '\r' | sed 's/^/   A /'
    [ "$n" -gt 0 ] && diff "$HERE/../checks/.a" "$HERE/../checks/.b" | sed 's/^/   /'
  done
  d="$S/p-$m-doom-serial.log"; echo "== $m doom: exc/panic $(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$d"); $(grep -aE 'DOOM|I_Init|V_Init|EdDI|VERIFIED' "$d" | tr -d '\r' | tail -3 | tr '\n' '|')"
done
rm -f "$HERE/../checks/.a" "$HERE/../checks/.b"
