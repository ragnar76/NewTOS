# EC -- EmuCON read-only session (armed launch:P4Z2: P4Z2 shel_write(EMUCON); EmuCON starts in C:\SUB, card con01):
# dir, cat (= type) a small file, cd .., dir of C:\, cat again, cat the 400 KB LONG.TXT head (read-heavy; Ctrl-C not needed, dump taken mid-run).
at 1600
text dir
key 0x1C
wait 300
mark e1-dir-sub
text cat tabs.txt
key 0x1C
wait 300
mark e2-cat-tabs
text cd ..
key 0x1C
wait 200
text dir
key 0x1C
wait 300
mark e3-dir-root
text cat tabs.txt
key 0x1C
wait 300
mark e4-cat-root-tabs
text cat long.txt
key 0x1C
wait 600
mark e5-cat-long
wait 400
