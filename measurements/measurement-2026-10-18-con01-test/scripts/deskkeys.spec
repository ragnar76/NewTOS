# DESKKEYS -- NEWDESK keyboard (armed "desktop up"): Alt+C opens drive C, Down/Up/Right arrows (window scroll), Esc (refresh), Ctrl+I (item info, needs a selection: click first),
# Enter closes the dialog, CR on the desktop (deselect), Ctrl+W (close), F-key (F1 not installed).
at 200
park
mkey 0x38 0x2E
mark d1-alt-c-open-c
wait 400
key 0x50
key 0x50
key 0x48
key 0x4D
mark d2-arrows
wait 300
key 0x01
mark d3-esc-refresh
wait 300
move 95 113
click
mark d4-click-select
wait 300
mkey 0x1D 0x17
mark d5-ctrl-i
wait 300
key 0x1C
mark d6-enter-closes
wait 300
key 0x1C
mark d7-cr-deselect
wait 300
mkey 0x1D 0x11
mark d8-ctrl-w
wait 300
mkey 0x1D 0x16
mark d9-ctrl-u
wait 300
