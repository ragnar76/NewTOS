# DRN -- NEWDESK (armed "desktop up"), card drn (desk48.inf windows: W1 C:\ (80..352 x 32..224), W2 C:\ (368..640),
# W3 C:\SUB\ (80..352 x 256..448, topped); W1 rows 18 px: FOLD 84, SUB 102, BIG 120, LONGNAME 138, NEWDESK 156, README 174;
# coordinates from measurement-2026-10-19-proc04-fdelete-test/method/inter/dcm.spec).
#   m: Ctrl-drag MOVE of C:\LONGNAME.TXT into W3 (C:\SUB\) = cross-directory Frename (desktop dir.rs Op::Rename)
#   r: File -> Show info (Ctrl+I) on C:\README.TXT, Esc clears the name field, type "renamed.txt", Return = OK
#      = same-directory Frename (main.rs do_showitem)
at 300
park
move 200 42
click                # W1 title -> top W1
wait 100
mark m0-start
move 140 138
click                # LONGNAME.TXT
wait 100
mark m1-selected
kdown 0x1D           # Ctrl held: MOVE
down
wait 40
drag 200 420         # into W3 -> MOVE C:\LONGNAME.TXT to C:\SUB\
up
kup 0x1D
wait 200
mark m2-move-box
key 0x1C             # Return = OK
wait 800
mark m3-moved
move 200 42
click                # W1 title -> top W1
wait 100
move 140 156
click                # README.TXT (one row up after the move)
wait 100
mark r1-selected
ctrl 0x17            # Ctrl+I = Show info
wait 300
mark r2-info
key 0x01             # Esc: clear the name field
wait 50
text renamed.txt
wait 50
mark r3-typed
key 0x1C             # Return = OK
wait 800
mark r4-renamed
wait 200
