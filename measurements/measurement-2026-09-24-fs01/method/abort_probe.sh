#!/usr/bin/env bash
# abort_probe.sh <tmpdir>
#
# Why a SINGLE orphan long-name slot only produces a warning while the
# everything-deleted card makes `fsck_msdos` die -- the question FS01 was
# asked to answer. Needs <tmpdir>/base.img (the seeded card), before.img (the
# same card emptied by the UNFIXED kernel) and fixed.img (emptied by the fixed
# one) from the two QEMU runs.
#
# The eight live orphan slots of before.img are 1, 3+4, 19+20, 26+27 and 92
# (method/lfn_orphan_scan.py). Each case below is the same image with a
# different subset of them left live, plus, in three cases, one planted LIVE
# 8.3 entry.
set -uo pipefail
T="${1:?usage: abort_probe.sh <tmpdir>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ALL="1,3,4,19,20,26,27,92"

mk() { # mk <out> <erase-list>
  python3 "$HERE/lfn_edit.py" "$T/before.img" "$T/$1" "$2" >/dev/null
}
keep() { # keep <out> <slots-to-keep-live>
  python3 - "$ALL" "$1" <<'EOF' > "$T/.erase"
import sys
a = sys.argv[1].split(','); k = sys.argv[2].split(',')
print(','.join(x for x in a if x not in k))
EOF
  mk "$2" "$(cat "$T/.erase")"
}

run() { # run <label> <image>
  echo "== $1  ($(basename "$2"))"
  DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$2" 2>/dev/null | head -1 | awk '{print $1}')
  [ -n "$DEV" ] || { echo "   attach failed"; return; }
  OUT=$(mktemp)
  script -q "$OUT" fsck_msdos -n "${DEV}s1" >/dev/null 2>&1
  RC=$?
  sed -e 's/^/   /' -e 's/\^D//' "$OUT"
  rm -f "$OUT"
  echo "   rc=$RC"
  hdiutil detach "$DEV" >/dev/null 2>&1
  echo
}

# 0. the two ends of the scale
run "seeded card, nothing deleted"                 "$T/base.img"
run "emptied by the FIXED kernel (0 orphans)"      "$T/fixed.img"
run "emptied by the UNFIXED kernel (8 orphans)"    "$T/before.img"

# 1. the counter-proof: the same unfixed image with the orphans freed by hand
mk "p-none.img" "$ALL"
run "unfixed image, all 8 orphan slots set to 0xE5" "$T/p-none.img"

# 2. is it a COUNT effect? one chain at a time, everything else freed
for k in 1 3,4 19,20 26,27 92; do
  tag="p-keep-$(echo "$k" | tr ',' '-').img"
  keep "$k" "$tag"
  run "unfixed image, ONLY the chain at slot(s) $k left" "$T/$tag"
done

# 3. a single orphan on an otherwise LIVE card: the seeded card with one 8.3
#    entry (.fseventsd, root slot 2) erased and its long-name slot left live
python3 "$HERE/lfn_edit.py" "$T/base.img" "$T/p-live-card.img" 2 >/dev/null
run "live card, ONE hand-made orphan" "$T/p-live-card.img"

# 4. the same single orphan, with a LIVE 8.3 entry after / before it
python3 "$HERE/plant83.py" "$T/p-keep-92.img" "$T/p-after.img"  93 "LIVE    TXT" >/dev/null
run "one orphan (slot 92), a LIVE entry right AFTER it (slot 93)" "$T/p-after.img"
python3 "$HERE/plant83.py" "$T/p-keep-1.img"  "$T/p-end.img"    96 "LIVE    TXT" >/dev/null
run "one orphan (slot 1), a LIVE entry at the END of the directory (slot 96)" "$T/p-end.img"
python3 "$HERE/plant83.py" "$T/p-keep-92.img" "$T/p-before.img"  6 "LIVE    TXT" >/dev/null
run "one orphan (slot 92), a LIVE entry only BEFORE it (slot 6)" "$T/p-before.img"
rm -f "$T/.erase"
