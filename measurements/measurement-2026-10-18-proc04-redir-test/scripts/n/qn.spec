# QN -- QED (#Z), armed up:QED: Ctrl-O file selector, drive button C (-> C:\), click AFOLD, click BSUB (a click on a folder
# enters it), double-click MARK.TXT (QED loads C:\AFOLD\BSUB\MARK.TXT).
at 300
park
ctrl 0x18
mark q1-fsel-open
wait 400
move 394 221
click
mark q2-drive-c
wait 400
move 235 221
click
mark q3-in-afold
wait 400
move 235 221
click
mark q4-in-bsub
wait 400
move 235 238
dclick
mark q5-open-mark-txt
wait 600
