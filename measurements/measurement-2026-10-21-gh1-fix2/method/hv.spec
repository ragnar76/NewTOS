# HypView with default doc: Ctrl-O file selector -> drive click -> Cancel; scroll arrow x3; close box
at 600
park
wait 250
mark h-loaded
mkey 0x1D 0x18
wait 400
mark h-fsel
move 392 220
wait 50
click
wait 250
mark h-fsel-drive
move 325 400
wait 50
click
wait 300
mark h-fsel-cancel
move 628 444
click
click
click
wait 250
mark h-scroll
move 9 29
click
wait 400
mark h-closed
