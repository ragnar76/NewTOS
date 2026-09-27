#!/usr/bin/env sh
#
# HW03 — drive the EMMC driver's R1 evaluation in QEMU `raspi4b`.
#
# Builds the kernel with the existing (opt-in) `selftest-emmc` feature, boots
# it against a scratch SD image, and captures the serial log.  The self-test's
# new `r1_rejection_test()` reads and writes 4096 blocks past the capacity the
# card reported in its CSD — the lever `docs/api/hw/emmc-audit.md` finding 3
# drove — and prints what the card answered.
#
# The host is macOS and has no `timeout`; the run is bounded with a background
# sleep + kill.  Nothing is flashed, no real Pi is touched, and the image is a
# scratch file outside the repository (the ticket forbids *.img artefacts).
#
# Usage:  sh method/run-selftest-qemu.sh <scratch-dir> <out.log>

set -eu

SCRATCH="${1:?usage: run-selftest-qemu.sh <scratch-dir> <out.log>}"
OUT="${2:?usage: run-selftest-qemu.sh <scratch-dir> <out.log>}"
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
IMG="$SCRATCH/sd-hw03.img"

mkdir -p "$SCRATCH"
[ -f "$IMG" ] || sh "$REPO/tools/make-sd-image.sh" "$IMG" 64

cd "$REPO"
cargo run -p xtask -- build --board rpi4 --features selftest-emmc

(
    cargo run -p xtask -- run --board rpi4 --sd "$IMG" --features selftest-emmc \
        > "$OUT" 2>&1 &
    pid=$!
    sleep 75
    kill "$pid" 2>/dev/null || true
    pkill -f "qemu-system-aarch64.*sd-hw03" 2>/dev/null || true
)

echo "--- [emmc] lines of $OUT ---"
grep 'emmc\]' "$OUT" || true
