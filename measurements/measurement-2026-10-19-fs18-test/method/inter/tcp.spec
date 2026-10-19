# TCP -- TeraDesk (#Z) copy and delete (armed up:DESKTOP), card tcp: open drive C (dclick HARD DISC icon (195,40)); the C:\ window
# (icon view, right part off screen) shows APPS (347,205) FOLD (427,205) SUB (507,205) BIG.TXT (587,205); TRASH icon (35,128).
# copy BIG.TXT onto the SUB folder icon, copy the folder FOLD (with contents) onto SUB, delete the original FOLD via TRASH.
at 300
park
move 195 40
dclick
wait 400
mark t1-window-open
move 587 205
click                # BIG.TXT
wait 100
mark t2-big-selected
down
wait 40
drag 507 205         # onto the SUB folder icon -> Copy
up
wait 300
mark t3-copy-big-dialog
key 0x1C             # Return = OK
wait 600
mark t4-big-copied
key 0x1C             # a possible second box (harmless otherwise)
wait 300
move 427 205
click                # FOLD
wait 100
mark t5-fold-selected
down
wait 40
drag 507 205         # onto SUB -> Copy folder tree
up
wait 300
mark t6-copy-fold-dialog
key 0x1C
wait 800
mark t7-fold-copied
key 0x1C
wait 300
move 427 205
click                # FOLD (original)
wait 100
down
wait 40
drag 35 128          # onto TRASH
up
wait 300
mark t8-delete-dialog
key 0x1C
wait 800
mark t9-deleted
key 0x1C
wait 300
mark t10-end
wait 200
