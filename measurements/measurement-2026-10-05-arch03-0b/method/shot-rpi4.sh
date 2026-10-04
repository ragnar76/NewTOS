#!/bin/sh
# usage: shot-rpi4.sh <img> <append> <secs> <log> <ppm>
S=$(dirname "$4")/qmp.sock; rm -f $S
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$4" -kernel "$1" -append "$2" -qmp unix:$S,server,nowait &
pid=$!
sleep "$3"
python3 - "$S" "$5" <<'PY'
import socket,json,sys
s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f=s.makefile('rw')
f.readline()
for c in [{"execute":"qmp_capabilities"},{"execute":"screendump","arguments":{"filename":sys.argv[2]}}]:
    f.write(json.dumps(c)+"\n"); f.flush(); print(f.readline().strip())
PY
kill $pid 2>/dev/null; wait $pid 2>/dev/null
