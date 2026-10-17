# TD -- TeraDesk (autorun #Z), armed launch:DESKTOP; geometry from the plain-kernel TeraDesk dump (640x480).
at 3000
park
move 195 40
dclick
mark t1-drive-window
wait 400
move 88 9
mark t2-file-menu
wait 400
move 330 300
wait 20
click
mark t3-menu-closed
wait 400
ctrl 0x11
mark t3b-after-ctrl-w
wait 400
ctrl 0x10
mark t4-quit-dialog
wait 400
move 320 312
wait 20
click
mark t5-after-quit
wait 400
