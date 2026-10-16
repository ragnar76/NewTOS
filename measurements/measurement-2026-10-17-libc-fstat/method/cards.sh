#!/usr/bin/env bash
# cards.sh -- LIBC-FSTAT cards in /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf/cards, all private copies of the series cards
# /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards/{plain,apitest,teradesk,qed}.img (sha256 f30f360a / 4c903c52 / 43a75ede / b4ad4d58, the
# cards of every series run since DESK43 U5b; their libc programs are byte-identical to relinks against the pre-LIBC-FSTAT libc.a,
# checks/cards.txt). B = relinked against the LIBC-FSTAT libc.a (build-libcprogs.sh -> $W/b), A = pre-LIBC-FSTAT (the card files,
# FSTATP + old-crt0 STDERRP from build-controls.sh -> $W/b-old).
#   series: plain.img (unchanged copy), apitest.img (+ B APITEST.NCP + C:\PTERMCH.NCP), teradesk.img (+ B DESKTOP.NAP), qed.img (+ B QED.NAP)
#   lf-all.img: apitest card with EVERY B libc program that is on the card replaced (55 files: C:\ root, C:\P10\*, Apps, DOOM)
#   B probe cards from lf-all, #Z -> the probe: fstatp (+ C:\FSTATP.NAP), stdoutdb, serr-newcrt0, serr-oldcrt0 (C:\STDERRP.NAP = old
#   crt0), stderra, ttya, doom; A probe cards from the apitest card: a-fstatp, a-stdoutdb, a-serr-newcrt0, a-serr-oldcrt0, a-stderra, a-ttya.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
W=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf; U=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards; C="$W/cards"; B="$W/b"; O="$W/b-old"
mkdir -p "$C"; E="$HERE/card-edit.sh"
cp "$U/plain.img" "$C/plain.img"; echo "plain.img: copy"
# + C:\PTERMCH.NCP (sdk/build.sh, no libc; the proc06 binary, sha256 30fa331e...), the child of APITEST case T33 -- as on proc06's
#   apitest-p06.img (without it T33.1-3 SKIP: first series pass of this run, checks/lf-vs-p06-first.txt)
bash "$E" "$U/apitest.img" "$C/apitest.img" - APITEST.NCP="$B/APITEST.NCP" PTERMCH.NCP="$HERE/../../measurement-2026-10-16-proc06/bin/PTERMCH.NCP"
bash "$E" "$U/teradesk.img" "$C/teradesk.img" - Apps/TERADESK/DESKTOP.NAP="$B/apps/DESKTOP.NAP"
bash "$E" "$U/qed.img" "$C/qed.img" - Apps/QED/QED.NAP="$B/apps/QED.NAP"
ALL=()
for f in "$B"/*.NAP "$B"/*.NCP; do n="$(basename "$f")"; case $n in FSTATP.NAP) continue;; esac; ALL+=("$n=$f"); done
for n in P10KID.NCP P10KID.NTP ONLYNCP.NCP LEGACY.PRG APP.NAP ACC.NAC; do ALL+=("P10/$n=$B/P10KID.NCP"); done
ALL+=("Apps/QED/QED.NAP=$B/apps/QED.NAP" "Apps/TERADESK/DESKTOP.NAP=$B/apps/DESKTOP.NAP" "DOOM/DOOM.NAP=$B/apps/DOOM.NAP")
echo "lf-all: ${#ALL[@]} files"
bash "$E" "$U/apitest.img" "$C/lf-all.img" - "${ALL[@]}"
bash "$E" "$C/lf-all.img" "$C/fstatp.img" 'C:\FSTATP.NAP' FSTATP.NAP="$B/FSTATP.NAP"
bash "$E" "$C/lf-all.img" "$C/stdoutdb.img" 'C:\STDOUTDB.NAP'
bash "$E" "$C/lf-all.img" "$C/serr-newcrt0.img" 'C:\STDERRP.NAP'
bash "$E" "$C/lf-all.img" "$C/serr-oldcrt0.img" 'C:\STDERRP.NAP' STDERRP.NAP="$B/oldcrt0/STDERRP.NAP"
bash "$E" "$C/lf-all.img" "$C/stderra.img" 'C:\STDERRA.NAP'
bash "$E" "$C/lf-all.img" "$C/ttya.img" 'C:\TTYA.NAP'
bash "$E" "$C/lf-all.img" "$C/doom.img" 'C:\DOOM\DOOM.NAP'
bash "$E" "$U/apitest.img" "$C/a-fstatp.img" 'C:\FSTATP.NAP' FSTATP.NAP="$O/FSTATP.NAP"
bash "$E" "$U/apitest.img" "$C/a-stdoutdb.img" 'C:\STDOUTDB.NAP'
bash "$E" "$U/apitest.img" "$C/a-serr-newcrt0.img" 'C:\STDERRP.NAP'
bash "$E" "$U/apitest.img" "$C/a-serr-oldcrt0.img" 'C:\STDERRP.NAP' STDERRP.NAP="$O/oldcrt0/STDERRP.NAP"
bash "$E" "$U/apitest.img" "$C/a-stderra.img" 'C:\STDERRA.NAP'
bash "$E" "$U/apitest.img" "$C/a-ttya.img" 'C:\TTYA.NAP'
# the source cards' own fsck (baseline for the "Warning:" line)
for c in plain apitest teradesk qed; do
  DEV=$(hdiutil attach -readonly -imagekey diskimage-class=CRawDiskImage -nomount "$U/$c.img" 2>/dev/null | head -1 | awk '{print $1}')
  echo "fsck source $c: $(fsck_msdos -n "${DEV}s1" 2>&1 | tr '\n' ' ')"; hdiutil detach "$DEV" >/dev/null 2>&1
done
shasum -a 256 "$U"/*.img "$C"/*.img
