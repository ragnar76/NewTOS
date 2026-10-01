#!/usr/bin/env bash
# seed.sh <image-path>
#
# Build a fresh card image with tools/make-sd-image.sh -- the single source of
# truth for the card; never hand-craft one -- with SD_SYSINFO_AUTO=1 so that
# C:\SYSINFO.NCP is additionally installed as \AUTO\SYSINFO.NCP.
#
# Why \AUTO\: SYSINFO.NCP is a console program. In \AUTO\ it runs before the
# AES comes up, prints its lines to the serial console, and terminates -- so a
# headless QEMU run proves ACC06 part 1 without a single click. The same
# binary stays in the root for the hand-driven (and real-Pi) case.
set -euo pipefail
IMG="${1:?usage: seed.sh <image-path>}"
WS=/Volumes/Daten/Development/PiRusTOS
SD_SYSINFO_AUTO=1 "$WS/tools/make-sd-image.sh" "$IMG" 64
