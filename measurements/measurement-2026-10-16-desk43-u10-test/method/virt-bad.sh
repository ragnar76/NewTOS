#!/usr/bin/env bash
# virt-bad.sh -- item 5: qemu-virt with a corrupt (kcv, kxv) / missing (kmv) NEWDESK blob. cwd = scratch (relative sockets). One QEMU at a time.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; M="$(cd "$HERE/.." && pwd)"
export APPEND=""
python3 "$HERE/virt-run.py" "$M/bin/kernel-virt-kcv.img" kcv-virt "$M" 'wait:could not be loaded:120' 'wait:No desktop\: NEWDESK:30' sleep:3 shot:emucon \
  'send:exit\r' 'wait:not started\: its load failed earlier:30' sleep:3 shot:emucon-again 'send:dir\r' sleep:3 shot:emucon-dir
python3 "$HERE/virt-run.py" "$M/bin/kernel-virt-kxv.img" kxv-virt "$M" 'wait:could not be loaded:120' 'wait:not started\: its load failed earlier:60' \
  'wait:No desktop\: NEWDESK[^$]*No desktop\: NEWDESK:30' sleep:3 shot:emucon-second sleep:10 shot:end
python3 "$HERE/virt-run.py" "$M/bin/kernel-virt-kmv.img" kmv-virt "$M" 'wait:No desktop\: NEWDESK:120' sleep:3 shot:emucon \
  'send:exit\r' 'wait:not started\: its load failed earlier:30' sleep:3 shot:emucon-again
