# TDD -- TeraDesk (#Z), armed up:DESKTOP, card tdd: open drive C by double-click on its HARD DISC icon (195,40; layout of
# measurement-2026-10-18-con01-test tkeys-wt-k1-window-open.png), select DELME.TXT in the C:\ window, drag it onto TRASH (35,128),
# confirm with Return.
at 300
park
move 195 40
dclick
wait 400
mark t1-window-open
move 428 205
click                # DELME.TXT
wait 100
mark t2-selected
down
wait 40
drag 35 128          # onto TRASH
up
wait 300
mark t3-confirm
key 0x1C             # Return
wait 500
mark t4-deleted
wait 200
