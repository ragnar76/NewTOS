#!/usr/bin/env bash
# cards.sh -- PROC06: the two cards of this run, each a private copy of the APITEST card of the series
# (/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards/apitest.img, sha256 4c903c52..., the card every series run since
# DESK43 U5b used), mounted on a private mount point (never /Volumes/NEWTOS), changed, unmounted:
#   apitest-p06.img  C:\APITEST.NCP replaced by the PROC06 build (case T33), + C:\PTERMCH.NCP; NEWDESK.INF (#Z C:\APITEST.NCP) unchanged
#   ctrlc.img        + C:\PTERMCH.NCP, C:\CTRLCPAR.NCP; NEWDESK.INF #Z line -> C:\CTRLCPAR.NCP
# Programs from $W/b (sdk/build.sh sdk/examples/{ptermch,ctrlcpar}.c, sdk/apps/apitest/build-apitest.sh).
set -euo pipefail
W=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06; SRC=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards/apitest.img
mkdir -p "$W/cards"
edit() { # edit <img> <shell-snippet using $MNT>
  local IMG="$1" DEV MNT; DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
  MNT="$W/cards/mnt.$$"; mkdir -p "$MNT"
  diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
  export COPYFILE_DISABLE=1
  eval "$2"
  find "$MNT" -name '._*' -delete 2>/dev/null || true
  rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
  sync; diskutil unmount "$MNT" >/dev/null; hdiutil detach "$DEV" >/dev/null; rmdir "$MNT"
}
[ "${FSCK_ONLY:-0}" = 1 ] || {
cp "$SRC" "$W/cards/apitest-p06.img"
edit "$W/cards/apitest-p06.img" 'echo "card APITEST.NCP vs pre-PROC06 build: $(cmp -s "$MNT/APITEST.NCP" "$W/b/APITEST-before.NCP" && echo identical || echo differs)";
  cp -X "$W/b/APITEST.NCP" "$MNT/APITEST.NCP"; cp -X "$W/b/PTERMCH.NCP" "$MNT/PTERMCH.NCP"; grep -a "^#Z" "$MNT/NEWDESK.INF"; ls "$MNT"'
cp "$SRC" "$W/cards/ctrlc.img"
edit "$W/cards/ctrlc.img" 'cp -X "$W/b/PTERMCH.NCP" "$MNT/PTERMCH.NCP"; cp -X "$W/b/CTRLCPAR.NCP" "$MNT/CTRLCPAR.NCP";
  python3 -I -c "import sys; p=sys.argv[1]; b=open(p,\"rb\").read(); assert b.count(b\"#Z 01 C:\\\\APITEST.NCP@\")==1; open(p,\"wb\").write(b.replace(b\"#Z 01 C:\\\\APITEST.NCP@\", b\"#Z 01 C:\\\\CTRLCPAR.NCP@\"))" "$MNT/NEWDESK.INF";
  grep -a "^#Z" "$MNT/NEWDESK.INF" | od -c | head -3'
}
for c in apitest-p06 ctrlc; do  # fsck the FAT16 partition (the raw image starts with the MBR)
  DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$W/cards/$c.img" | head -1 | awk '{print $1}')
  echo "fsck $c: $(fsck_msdos -n "${DEV}s1" 2>&1 | tr '\n' ' ')"; hdiutil detach "$DEV" >/dev/null
done
shasum -a 256 "$W/cards/"*.img
