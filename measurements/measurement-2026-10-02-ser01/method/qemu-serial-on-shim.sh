#!/usr/bin/env bash
#
# qemu-system-aarch64 shim — SER01 (2026-10-02).
#
# Since SER01 the board-rpi4 kernel keeps UART0 silent unless the boot
# arguments contain `serial=on`; on QEMU raspi4b those arguments are the
# `-append` string (answered through the firmware model's "get command line"
# mailbox tag). Measurement scripts written before that date start
# `qemu-system-aarch64 -M raspi4b … -kernel …` without `-append` and would
# now record an empty serial log. They are evidence and stay unedited;
# instead run them with this directory first on PATH:
#
#     PATH="/path/to/repo/tools/qemu-serial-on:$PATH" bash measurements/<run>/method/<script>.sh …
#
# The shim adds `-append serial=on` when the machine is raspi4b and the
# command line carries no `-append` of its own, then execs the first
# *other* qemu-system-aarch64 on PATH. Everything else passes through
# unchanged (qemu-virt keeps its serial output on regardless).
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

REAL=""
IFS=: read -r -a dirs <<< "$PATH"
for d in "${dirs[@]}"; do
  [ -z "$d" ] && continue
  c="$d/qemu-system-aarch64"
  if [ -x "$c" ] && [ "$(cd "$d" 2>/dev/null && pwd)" != "$SELF_DIR" ]; then
    REAL="$c"; break
  fi
done
if [ -z "$REAL" ]; then
  echo "qemu-serial-on: no real qemu-system-aarch64 on PATH" >&2
  exit 127
fi

raspi=0; has_append=0; prev=""
for a in "$@"; do
  case "$prev" in
    -M|-machine) case "$a" in raspi4b|raspi4b,*) raspi=1 ;; esac ;;
  esac
  case "$a" in -append) has_append=1 ;; esac
  prev="$a"
done

if [ "$raspi" = 1 ] && [ "$has_append" = 0 ]; then
  exec "$REAL" "$@" -append serial=on
fi
exec "$REAL" "$@"
