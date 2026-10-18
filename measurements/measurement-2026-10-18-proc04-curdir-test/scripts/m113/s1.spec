# S1 -- desktop (a), accessory (b), hold-menu-open (d); pointer geometry from the x1 calibration run (640x480).
at 200
park
move 37 36
dclick
mark w1-c-window
wait 300
move 95 113
dclick
mark w2-folder-apps
wait 300
move 95 95
click
mark w3-selected
wait 300
move 136 9
mark m1-view-menu-open
wait 300
move 160 27
click
mark m2-view-as-icons
wait 300
move 136 9
move 160 43
click
mark m3-view-as-text
wait 300
move 190 9
move 240 123
mark d0-options-hover-item
wait 300
click
mark d1-desktop-config-dialog
wait 300
key 0x1C
mark d2-dialog-closed
wait 300
move 40 9
move 60 27
click
mark a1-desktop-info-dialog
wait 300
key 0x1C
mark a2-after-info
wait 300
move 59 53
click
mark c1-window-closed
wait 300
move 37 36
dclick
mark c2-window-reopened
wait 300
move 40 9
move 60 59
click
mark s1-desk-item-3
wait 300
key 0x1C
mark s2-after-return
wait 300
move 40 9
mark h0-hold-start
wait 3000
nop
mark h1-hold-end
wait 300
wait 300
move 330 300
wait 20
click
mark h2-menu-closed
wait 300
