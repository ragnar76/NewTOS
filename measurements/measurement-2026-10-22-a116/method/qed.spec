# QED: new text, type, Datei menu, away, Ctrl-Q -> alert -> Abbruch -> Ctrl-Q -> alert -> Verwerfen
at 600
mkey 0x1D 0x31
wait 250
mark q-new
text abc
wait 250
mark q-typed
park
move 83 9
click
wait 250
mark q-menu
move 300 300
click
wait 250
mark q-menu-away
mkey 0x1D 0x10
wait 250
mark q-alert
move 427 292
click
wait 250
mark q-cancelled
mkey 0x1D 0x10
wait 250
mark q-alert2
move 331 292
click
wait 400
mark q-quit
