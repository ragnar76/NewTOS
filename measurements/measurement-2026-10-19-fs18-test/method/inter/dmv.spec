# DMV -- NEWDESK (armed "desktop up"), card drn/dmv (desk48.inf windows: W1 C:\ (80..352 x 32..224), W2 C:\ (368..640),
# W3 C:\SUB\ (80..352 x 256..448, topped); W1 rows 18 px: FOLD 84, SUB 102, BIG 120, LONGNAME 138, NEWDESK 156, README 174).
#   f: Ctrl-drag MOVE of the FOLDER C:\FOLD (A.TXT B.TXT INNER\C.TXT) into W3 (C:\SUB\) = cross-directory Frename of a directory
#   b: File -> Show info (Ctrl+I) on BIG.TXT (multi-cluster, 40 KB): rename to big2.txt (same-dir Frename)
#   d: Show info on the folder SUB: rename to newsub (same-dir Frename of a directory that holds the moved FOLD tree)
at 300
park
move 200 42
click                # W1 title -> top W1
wait 100
mark f0-start
move 140 84
click                # FOLD
wait 100
mark f1-selected
kdown 0x1D           # Ctrl held: MOVE
down
wait 40
drag 200 420         # into W3 -> MOVE C:\FOLD to C:\SUB\FOLD
up
kup 0x1D
wait 200
mark f2-move-box
key 0x1C             # Return = OK
wait 1000
mark f3-moved
key 0x1C             # a possible second box
wait 300
move 200 42
click                # W1 title -> top W1
wait 100
move 140 102
click                # BIG.TXT (rows shift up by one after FOLD left: SUB 84, BIG 102)
wait 100
mark b1-selected
ctrl 0x17            # Ctrl+I = Show info
wait 300
mark b2-info
key 0x01             # Esc: clear the name field
wait 50
text big2.txt
wait 50
mark b3-typed
key 0x1C             # Return = OK
wait 800
mark b4-renamed
move 140 84
click                # folder SUB
wait 100
mark d1-selected
ctrl 0x17
wait 300
mark d2-info
key 0x01
wait 50
text newsub
wait 50
mark d3-typed
key 0x1C
wait 800
mark d4-renamed
key 0x1C
wait 400
mark d5-after
wait 200
