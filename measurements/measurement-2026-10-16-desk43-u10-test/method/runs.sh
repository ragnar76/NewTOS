#!/usr/bin/env bash
# runs.sh <scratch-dir> -- the three QEMU raspi4b runs of this unit, one QEMU at a time; cwd for the sockets is
# <scratch-dir>/sock (relative socket paths, < 104 bytes). Kernels from build-kernels.sh, card from make-card.sh.
set -uo pipefail
W="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; M="$(cd "$HERE/.." && pwd)"
mkdir -p "$W/sock"; cd "$W/sock"
python3 "$HERE/run.py" "$W/k/kernel8-k.img" "$W/q/base.img" k-desk "$M" 'wait:\[desk\] desktop up:180' sleep:4 shot:desktop
python3 "$HERE/run.py" "$W/k/kernel8-kc.img" "$W/q/base.img" kc-fail "$M" 'wait:could not be loaded:180' \
  'wait:No desktop\: NEWDESK:30' 'wait:> :30' sleep:2 shot:emucon 'send:exit\r' \
  'wait:not started\: its load failed earlier:30' 'wait:> :30' sleep:2 shot:emucon-again 'send:dir\r' sleep:3 shot:emucon-dir
python3 "$HERE/run.py" "$W/k/kernel8-kx.img" "$W/q/base.img" kx-again "$M" 'wait:could not be loaded:180' \
  'wait:not started\: its load failed earlier:60' 'wait:No desktop\: NEWDESK[^$]*No desktop\: NEWDESK:30' 'wait:> $:30' \
  sleep:3 shot:emucon-second sleep:10 shot:end
