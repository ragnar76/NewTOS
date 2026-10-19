# TMV -- TeraDesk (#Z), armed up:DESKTOP, card tcp (same layout as tcp.spec): open drive C (dclick HARD DISC icon (195,40)); the C:\ window
# (icon view) shows APPS (347,205) FOLD (427,205) SUB (507,205) BIG.TXT (587,205) ...
#   m1: Ctrl-drag BIG.TXT onto SUB = MOVE (cross-directory Frename of a 39200-byte multi-cluster file)
#   m2: Ctrl-drag the folder FOLD (A.TXT B.TXT INNER\C.TXT) onto SUB = MOVE of a folder tree (cross-directory Frename of a directory)
#   r : Show info (Ctrl+I) on SUB (icons shifted: APPS 347, SUB 427), rename to newsub (same-directory Frename of a directory)
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
kdown 0x1D           # Ctrl held: MOVE
down
wait 40
drag 507 205         # onto the SUB folder icon
up
kup 0x1D
wait 300
mark t3-move-big-dialog
key 0x1C             # Return = OK
wait 800
mark t4-big-moved
key 0x1C             # a possible second box (harmless otherwise)
wait 300
mark t4b-after
move 427 205
click                # FOLD
wait 100
mark t5-fold-selected
kdown 0x1D
down
wait 40
drag 507 205         # onto SUB -> move folder tree
up
kup 0x1D
wait 300
mark t6-move-fold-dialog
key 0x1C
wait 1000
mark t7-fold-moved
key 0x1C
wait 400
mark t7b-after
move 427 205
click                # SUB (now the 2nd icon)
wait 100
mark t8-sub-selected
ctrl 0x17            # Ctrl+I = Show info
wait 300
mark t9-info
key 0x01             # Esc: clear the name field
wait 50
text newsub
wait 50
mark t10-typed
key 0x1C             # Return = OK
wait 800
mark t11-renamed
key 0x1C
wait 400
mark t12-after
wait 200
