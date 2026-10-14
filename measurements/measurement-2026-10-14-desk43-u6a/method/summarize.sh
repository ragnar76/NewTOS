#!/usr/bin/env bash
# summarize.sh -- one line per run from ../serial/*-serial.log: memprot mode
# line, the program's own result line or panic line, the shell's exit line,
# "desktop up" count, count of kernel [EXC]/[PANIC]/crash-prompt lines.
cd "$(dirname "$0")/../serial"
for f in *-serial.log; do
  t="${f%-serial.log}"
  mp=$(grep -a -o -m1 -E "memory protection (ON|OFF)[^:]*" "$f" | head -1)
  res=$(grep -a -m1 -E "^\[HELLOGEM\]|\*\*\* Rust panic" "$f" | cut -c1-200)
  ex=$(grep -a -o -m1 -E "sh_ldapp: 'C:[^']*' exited with rc=-?[0-9]+" "$f")
  up=$(grep -a -c "desktop up" "$f")
  exc=$(grep -a -c -E "\[EXC\]|\[PANIC\]|Press any key" "$f")
  echo "$t | $mp | $ex | desktop-up=$up | [EXC]/[PANIC]=$exc"
  echo "    $res"
done
