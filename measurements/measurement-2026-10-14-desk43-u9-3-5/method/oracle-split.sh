#!/usr/bin/env bash
# oracle-split.sh -- DESK43 U9-5, the G2 oracle evaluated correctly (seq.sh's K-vs-U<n> diff is not the comparison: with
# desk=kernel the caller column K holds BOTH the kernel's own shell/boot calls and the built-in desktop's, with the EL0
# desktop only the former). sequence-all-el1 = K lines only; sequence-all-el0 = K lines, then the NEWDESK slot U0.
#   (a) the first N lines of el1 (N = number of K lines in el0) vs el0's K lines: the kernel-context part;
#   (b) the rest of el1 vs el0's U0 lines (caller column dropped): the desktop's own call sequence, both link targets.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; TR="$HERE/../trace"; OUT="$HERE/../checks/u935-oracle-split.txt"
N=$(grep -c '^K ' "$TR/sequence-all-el0.txt")
{
  echo "el1 lines $(wc -l < "$TR/sequence-all-el1.txt"); el0 K lines $N, U0 lines $(grep -c '^U0 ' "$TR/sequence-all-el0.txt"); other callers el0: $(grep -vc '^\(K\|U0\) ' "$TR/sequence-all-el0.txt")"
  echo "(a) kernel-context part, el1[1..$N] vs el0 K:"; diff <(head -n "$N" "$TR/sequence-all-el1.txt") <(grep '^K ' "$TR/sequence-all-el0.txt") && echo "    identical"
  echo "(b) desktop part, el1[$((N+1))..] (K) vs el0 U0, '<' = EL1 only, '>' = EL0 only:"
  diff <(tail -n +"$((N+1))" "$TR/sequence-all-el1.txt" | sed 's/^K //') <(grep '^U0 ' "$TR/sequence-all-el0.txt" | sed 's/^U0 //')
  echo "(b') the same without results:"
  diff <(tail -n +"$((N+1))" "$TR/sequence-all-el1.txt" | sed -E 's/^K //; s/ -> .*$//') <(grep '^U0 ' "$TR/sequence-all-el0.txt" | sed -E 's/^U0 //; s/ -> .*$//')
} > "$OUT"
cat "$OUT"
