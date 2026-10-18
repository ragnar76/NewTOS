# DSK -- NEWDESK (armed "desktop up"), card dsk (inter/desk48.inf layout, measurement-2026-10-17-desk48 cal-1-desk.png):
# W1 C:\ (80..352 x 32..224), W2 C:\ (368..640), W3 C:\SUB\ (80..352 x 256..448, topped); icons Hard disk (37,35),
# NOTE.TXT shortcut (37,128), Trash (37,215). Two deletes via the Trash: NOTE.TXT from W3, README.TXT from W1.
at 300
park
move 140 308
click                # NOTE row in W3 -> selected
wait 100
mark a1-note-selected
down
wait 40
drag 37 215          # onto the Trash
up
wait 200
mark a2-delete-box
wait 100
key 0x1C             # Return = OK of the DELETE box
wait 400
mark a3-note-deleted
move 200 42
click                # W1 title -> top W1
wait 100
move 140 120
click                # README row in W1
wait 100
mark b1-readme-selected
down
wait 40
drag 37 215          # onto the Trash
up
wait 200
mark b2-delete-box
wait 100
key 0x1C             # Return = OK
wait 400
mark b3-readme-deleted
wait 200
