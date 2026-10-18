# TKEYS -- TeraDesk (#Z), armed up:DESKTOP: open drive window C by double-click, Ctrl+W closes it, open again, Ctrl+W again, then Ctrl+Q (Quit dialog).
at 300
park
move 195 40
dclick
mark k1-window-open
wait 400
ctrl 0x11
mark k2-after-ctrl-w
wait 400
move 195 40
dclick
mark k3-window-open-again
wait 400
ctrl 0x11
mark k4-after-ctrl-w2
wait 400
ctrl 0x10
mark k5-ctrl-q-dialog
wait 600
