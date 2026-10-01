#!/usr/bin/env bash
# manifest.sh <dir> -- write manifest.sha256 for everything under method/,
# screendumps/, serial/ (never *.img).
set -euo pipefail
DIR="${1:?usage: manifest.sh <dir>}"
cd "$DIR"
find method screendumps serial RUNINFO.TXT -type f 2>/dev/null | sort | xargs shasum -a 256 > manifest.sha256
echo "wrote $DIR/manifest.sha256"
