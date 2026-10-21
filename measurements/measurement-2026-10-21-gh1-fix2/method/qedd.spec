# QED diagnostic (GH1 regression candidate): same prefix as qed.spec, then long idle waits and a plain key before Ctrl-Q
at 600
mkey 0x1D 0x31
wait 250
mark d-new
text abc
wait 250
mark d-typed
park
move 83 9
click
wait 250
mark d-menu
move 300 300
click
wait 100
mark d-away1
wait 150
mark d-away2
wait 800
mark d-away3
key 0x2D
wait 250
mark d-x
mkey 0x1D 0x10
wait 400
mark d-ctrlq
wait 800
mark d-ctrlq-late
