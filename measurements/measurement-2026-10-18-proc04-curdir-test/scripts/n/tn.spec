# TN -- TeraDesk (#Z), armed up:DESKTOP: drive window C, folder AFOLD, subfolder BSUB, launch CWDB from BSUB, back (closer), launch CWDA, back, launch CWDR.
at 300
park
move 195 40
dclick
mark t1-drive-window
wait 400
move 348 205
dclick
mark t2-in-afold
wait 400
move 348 205
dclick
mark t3-in-bsub
wait 400
move 348 205
dclick
mark t4-launch-cwdb
wait 900
nop
mark t4b-after-cwdb
wait 300
move 316 152
click
mark t5-closer-to-afold
wait 400
move 428 205
dclick
mark t6-launch-cwda
wait 900
nop
mark t6b-after-cwda
wait 300
move 316 152
click
mark t7-closer-to-root
wait 400
move 508 205
dclick
mark t8-launch-cwdr
wait 900
nop
mark t8b-after-cwdr
wait 300
