# DCM -- NEWDESK desktop copy / move / delete (armed "desktop up"), card dcm (desk48.inf windows: W1 C:\ (80..352 x 32..224),
# W2 C:\ (368..640), W3 C:\SUB\ (80..352 x 256..448, topped); rows 18 px: C:\ lists FOLD 84, SUB 102, BIG 120, LONGNAME 138,
# NEWDESK 156, README 174; C:\SUB\ NOTE 308, OTHER 326; Trash icon (37,215)).
at 300
park
move 200 42
click                # W1 title -> top W1
wait 100
mark s0-start
move 140 174
click                # README.TXT in W1
wait 100
mark c1-readme-selected
down
wait 40
drag 410 102         # onto the SUB folder row of W2 -> COPY README.TXT to C:\SUB\
up
wait 200
mark c2-copy-box
key 0x1C             # Return = OK
wait 500
mark c3-copied
move 140 120
click                # BIG.TXT (several clusters)
wait 100
down
wait 40
drag 410 102         # onto the SUB folder row of W2 -> COPY BIG.TXT
up
wait 200
mark d1-big-box
key 0x1C
wait 800
mark d2-big-copied
move 140 84
click                # FOLD (a folder with contents)
wait 100
mark e1-fold-selected
down
wait 40
drag 200 410         # into the empty area of W3 (C:\SUB\) -> COPY the folder tree to C:\SUB\FOLD
up
wait 200
mark e2-folder-copy-box
key 0x1C
wait 1000
mark e3-folder-copied
move 200 42
click                # W1 title -> top
wait 100
move 140 138
click                # LONGNAME.TXT (12 chars)
wait 100
kdown 0x1D           # Ctrl held: MOVE
down
wait 40
drag 200 420         # into W3 -> MOVE C:\LONGNAME.TXT to C:\SUB\
up
kup 0x1D
wait 200
mark f1-move-box
key 0x1C
wait 800
mark f2-moved
move 140 84
click                # FOLD in W1 again (selected, rebuilt window)
wait 100
down
wait 40
drag 37 215          # onto the Trash -> delete the folder with its contents
up
wait 300
mark g1-delete-folder-box
key 0x1C
wait 800
mark g2-after-ok
key 0x1C             # a second box if there is one (harmless Return otherwise)
wait 800
mark g3-fold-deleted
wait 300
