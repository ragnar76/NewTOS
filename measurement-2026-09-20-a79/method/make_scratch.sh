#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <look: tos4gray|literal>
# Copies the crates into a scratch dir (never the shared working tree), applies
# the H02/A46/A78 injector scaffolding + the H02 serial AES trace (scratch copy
# only), and sets the A79 FRAME_LOOK default: "tos4gray" leaves the tree's
# default (Tos4Gray), "literal" flips the one AtomicU8 initialiser to Literal.
# The two scratch kernels differ in nothing else.
set -euo pipefail
WS="$1"; SCR="$2"; LOOK="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
W="$SCR/tos-api/src/aes/window.rs"
DEF='pub(crate) static FRAME_LOOK: AtomicU8 = AtomicU8::new(FrameLook::Tos4Gray as u8);'
LIT='pub(crate) static FRAME_LOOK: AtomicU8 = AtomicU8::new(FrameLook::Literal as u8); // A79-TEMP scratch: literal look'
if [ "$(grep -c "$DEF" "$W")" != "1" ]; then echo "FRAME_LOOK default line not found exactly once" >&2; exit 1; fi
case "$LOOK" in
  tos4gray) echo "FRAME_LOOK default = Tos4Gray (tree)";;
  literal)  python3 - "$W" "$DEF" "$LIT" <<'PY'
import sys
p, a, b = sys.argv[1:4]
s = open(p).read(); assert s.count(a) == 1; open(p, 'w').write(s.replace(a, b, 1)); print("FRAME_LOOK default = Literal (scratch patch)")
PY
  ;;
  *) echo "look must be tos4gray|literal" >&2; exit 1;;
esac
python3 "$HERE/a79_patch.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$HERE/a79_scripts.rs"
python3 "$HERE/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
