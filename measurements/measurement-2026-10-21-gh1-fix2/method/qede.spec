# QED diagnostic E: menu opened (Datei), then Esc instead of a click-away, then plain key x, Ctrl-Q
at 600
mkey 0x1D 0x31
wait 250
mark e-new
text abc
wait 250
mark e-typed
park
move 83 9
click
wait 250
mark e-menu
key 0x01
wait 250
mark e-esc
key 0x2D
wait 250
mark e-x
mkey 0x1D 0x10
wait 400
mark e-ctrlq
