#!/usr/bin/env bash
# card-edit.sh <src.img> <dst.img> <autorun or -> [NAME=file ...] -- LIBC-FSTAT: private copy of a series card
# (/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards/*.img), mounted on a private mount point (never /Volumes/NEWTOS),
# the listed files copied to C:\ (NAME may contain a sub-path, e.g. Apps/QED/QED.NAP; an existing file keeps its timestamp), the NEWDESK.INF "#Z 01 C:\APITEST.NCP@"
# line pointed at <autorun> ('-' = NEWDESK.INF unchanged), unmounted, fsck_msdos -n. Edit function from
# measurement-2026-10-16-proc06/method/cards.sh.
set -euo pipefail
SRC="$1"; DST="$2"; AUTO="$3"; shift 3
cp "$SRC" "$DST"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$DST" 2>/dev/null | head -1 | awk '{print $1}')
MNT="$(dirname "$DST")/mnt.$$"; mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
# a file that exists on the card keeps its directory timestamp (the desktop's text view shows it); a new file gets "now"
for kv in "$@"; do n="${kv%%=*}"; f="${kv#*=}"
  python3 -I -c 'import os,sys,shutil; d,s=sys.argv[1:]; st=os.stat(d) if os.path.exists(d) else None; shutil.copyfile(s,d); st and os.utime(d,(st.st_atime,st.st_mtime))' "$MNT/$n" "$f"
done
if [ "$AUTO" != - ]; then
  python3 -I -c 'import sys; p,a=sys.argv[1:]; b=open(p,"rb").read(); o=b"#Z 01 C:\\APITEST.NCP@"; assert b.count(o)==1, "no #Z APITEST line"; open(p,"wb").write(b.replace(o, b"#Z 01 "+a.encode()+b"@"))' "$MNT/NEWDESK.INF" "$AUTO"
fi
echo "$(basename "$DST"): $(grep -a '^#Z' "$MNT/NEWDESK.INF" | tr -d '\r' || echo 'no #Z')"
find "$MNT" -name '._*' -delete 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync; diskutil unmount "$MNT" >/dev/null; rmdir "$MNT"
echo "fsck $(basename "$DST"): $(fsck_msdos -n "${DEV}s1" 2>&1 | tr '\n' ' ')"
hdiutil detach "$DEV" >/dev/null 2>&1
