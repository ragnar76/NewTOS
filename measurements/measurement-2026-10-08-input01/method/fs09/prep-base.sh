#!/usr/bin/env bash
# prep-base.sh <scratch> -- ONE base card for all FS08-EMMC QEMU runs
# (user rule 2026-10-07: one base image per card content, per-run APFS clone).
#   1. FSBENCH test files (gen-fsbench-files.py, deterministic) -> <scratch>/files
#   2. FSBENCH.NCP (build-fsbench.sh)                          -> <scratch>/bin
#   3. tools/make-sd-image.sh 256 MiB with SD_FSBENCH_D         -> <scratch>/base.img
#      (C: NEWTOS + D: NEWTOS1\FSBENCH\, both FAT16; default C: content)
#   4. C:\AUTO\FSBENCH.NCP + C:\FSBNOKEY.FLG via mtools         -> <scratch>/base-fsb.img
#      base.img stays without FSBENCH (for the boot/write runs).
set -euo pipefail
S="${1:?scratch dir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
mkdir -p "$S/bin"
python3 "$WS/sdk/examples/fsbench/gen-fsbench-files.py" "$S/files" --header "$S/files/fsbench-manifest.h" >/dev/null
cmp "$S/files/fsbench-manifest.h" "$WS/sdk/examples/fsbench/fsbench-manifest.h"
"$WS/sdk/examples/fsbench/build-fsbench.sh" "$S/bin/FSBENCH.NCP"
SD_FSBENCH_D="$S/files" "$WS/tools/make-sd-image.sh" "$S/base.img" 256 > "$S/mkimg.log" 2>&1
cp -c "$S/base.img" "$S/base-fsb.img"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$S/base-fsb.img")
mmd -i "$S/base-fsb.img@@$OFF" ::AUTO 2>/dev/null || true
mcopy -o -i "$S/base-fsb.img@@$OFF" "$S/bin/FSBENCH.NCP" ::AUTO/FSBENCH.NCP
printf 'skip FSBENCH key waits\r\n' > "$S/FSBNOKEY.FLG"
mcopy -o -i "$S/base-fsb.img@@$OFF" "$S/FSBNOKEY.FLG" ::FSBNOKEY.FLG
shasum -a 256 "$S/base.img" "$S/base-fsb.img" "$S/bin/FSBENCH.NCP"
