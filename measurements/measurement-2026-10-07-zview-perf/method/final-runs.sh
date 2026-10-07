#!/usr/bin/env bash
# final-runs.sh <S> -- the two measured QEMU runs (one QEMU at a time).
set -uo pipefail
S="$1"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$HERE/mk-inj.sh" "$S" zp-vga
bash "$HERE/zp-runs.sh" "$S" "$S/kinj-zp-vga.img" vga --stamp
bash "$HERE/mk-inj.sh" "$S" zp-hd
QEMU_EXTRA="-global bcm2835-fb.xres=1920 -global bcm2835-fb.yres=1080" bash "$HERE/zp-runs.sh" "$S" "$S/kinj-zp-hd.img" hd photos --stamp
