# TDR -- TeraDesk (autorun #Z), armed launch:DESKTOP: Quit dialog via Ctrl+Q, click "Reboot"
# (button geometry from measurement-2026-10-17-x01/screendumps/td-t4-quit-dialog.png: Reboot centre ~ (320,280), 640x480).
at 3000
park
ctrl 0x10
mark q1-quit-dialog
wait 400
move 320 281
wait 20
click
mark q2-after-reboot-click
wait 400
