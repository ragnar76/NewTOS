#!/usr/bin/env bash
# seq.sh -- DESK43 U9-5, the G2 oracle: ONE kernel with the svc-seq-trace sequence tracer and inj-desk-seq.py's Alt-C
# (bin/kernel8-ks.img), run with the EL0 NEWDESK and with `desk=kernel` (memprot on). C:\SVCSEQ.LOG read back from each
# run's card copy, turned into sequences with tools/svc-seq.py: the desktop's calls only (EL1: caller K; EL0: the
# NEWDESK slot U<n>), caller column dropped -> the two link targets compared line by line (../trace/).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"
K="$M/bin/kernel8-ks.img"; W=/tmp/m5j/a15; TR="$M/trace"; mkdir -p "$W/seq" "$TR"; cp /tmp/m5j/u5b/cards/plain.img "$W/seq/plain.img"
getlog() {
  DEV=$(hdiutil attach -readonly -nomount -imagekey diskimage-class=CRawDiskImage "$1" 2>/dev/null | head -1 | awk '{print $1}')
  MNT="$W/seq/mnt"; mkdir -p "$MNT"; diskutil mount readOnly -mountPoint "$MNT" "${DEV}s1" > /dev/null
  cp "$MNT/SVCSEQ.LOG" "$2" || echo "no SVCSEQ.LOG"
  diskutil unmount "$MNT" > /dev/null; hdiutil detach "$DEV" > /dev/null 2>&1
}
for p in el0 el1; do
  if [ $p = el1 ]; then export APPEND="serial=on desk=kernel"; else export APPEND="serial=on"; fi
  KEEP=1 bash "$HERE/run-rpi4.sh" "$K" "$W/seq/plain.img" "seq-$p-on-rpi4" 20:t20.png 45:t45.png
  getlog "$W/seq/run-seq-$p-on-rpi4.img" "$TR/SVCSEQ-$p.LOG"; rm -f "$W/seq/run-seq-$p-on-rpi4.img"
  python3 "$WS/tools/svc-seq.py" "$TR/SVCSEQ-$p.LOG" > "$TR/sequence-all-$p.txt"
done
# the desktop's caller column: K for EL1; for EL0 the slot of NEWDESK ("[launch] EL0: slot N")
N=$(grep -a '\[launch\] EL0: slot' "$M/serial/seq-el0-on-rpi4-serial.log" | head -1 | sed -E 's/.*slot ([0-9]+).*/\1/')
python3 "$WS/tools/svc-seq.py" --caller K --no-caller "$TR/SVCSEQ-el1.LOG" > "$TR/desktop-shape-el1.txt"
python3 "$WS/tools/svc-seq.py" --caller "U$N" --no-caller "$TR/SVCSEQ-el0.LOG" > "$TR/desktop-shape-el0.txt"
python3 "$WS/tools/svc-seq.py" --caller K --no-caller --no-results "$TR/SVCSEQ-el1.LOG" > "$TR/desktop-shape-noresults-el1.txt"
python3 "$WS/tools/svc-seq.py" --caller "U$N" --no-caller --no-results "$TR/SVCSEQ-el0.LOG" > "$TR/desktop-shape-noresults-el0.txt"
echo "EL0 desktop caller U$N; lines el1 $(wc -l < "$TR/desktop-shape-el1.txt") el0 $(wc -l < "$TR/desktop-shape-el0.txt")"
diff "$TR/desktop-shape-el1.txt" "$TR/desktop-shape-el0.txt" > "$M/checks/acc-oracle-diff.txt"; echo "with results: $(grep -c '^[<>]' "$M/checks/acc-oracle-diff.txt") differing lines"
diff "$TR/desktop-shape-noresults-el1.txt" "$TR/desktop-shape-noresults-el0.txt" > "$M/checks/acc-oracle-diff-noresults.txt"; echo "without results: $(grep -c '^[<>]' "$M/checks/acc-oracle-diff-noresults.txt") differing lines"
