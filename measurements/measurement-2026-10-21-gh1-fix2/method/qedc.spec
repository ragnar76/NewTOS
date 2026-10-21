# QED diagnostic C (GH1 regression candidate): NO menu - a plain click in the text area, then idle, plain key x, Ctrl-Q
at 600
mkey 0x1D 0x31
wait 250
mark c-new
text abc
wait 250
mark c-typed
park
move 300 300
click
wait 250
mark c-clicked
wait 400
mark c-idle
key 0x2D
wait 250
mark c-x
mkey 0x1D 0x10
wait 400
mark c-ctrlq
