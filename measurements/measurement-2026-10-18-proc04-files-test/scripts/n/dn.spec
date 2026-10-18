# DN -- NEWDESK: root window, folder AFOLD, subfolder BSUB, launch CWDB from BSUB, close back, launch CWDA, close back, launch CWDR.
at 200
park
move 37 36
dclick
mark n1-root-window
wait 300
move 95 95
dclick
mark n2-in-afold
wait 300
move 95 95
dclick
mark n3-in-bsub
wait 300
move 95 95
dclick
mark n4-launch-cwdb
wait 900
nop
mark n4b-after-cwdb
wait 300
move 59 44
click
mark n5-closed-to-afold
wait 300
move 95 102
dclick
mark n6-launch-cwda
wait 900
nop
mark n6b-after-cwda
wait 300
move 59 44
click
mark n7-closed-to-root
wait 300
move 95 102
dclick
mark n8-launch-cwdr
wait 900
nop
mark n8b-after-cwdr
wait 300
