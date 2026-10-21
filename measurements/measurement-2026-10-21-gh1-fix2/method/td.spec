# TeraDesk: drive icon double-click -> window; title drag; Desk menu -> System Info (accessory) -> popup -> Esc -> Cancel; folder dclick; closer
at 600
park
move 195 40
wait 50
dclick
wait 250
mark t-open
move 450 153
wait 100
down
wait 50
drag 250 71
wait 50
up
wait 250
mark t-moved
move 25 9
click
wait 250
mark t-deskmenu
move 65 59
click
wait 400
mark t-sysinfo
move 480 250
wait 50
click
wait 250
mark t-popup
key 0x01
wait 250
mark t-esc
move 468 455
wait 50
click
wait 300
mark t-sysclosed
