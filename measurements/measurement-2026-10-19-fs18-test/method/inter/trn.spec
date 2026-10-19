# TRN -- TeraDesk (#Z), armed up:DESKTOP, card trn (= the tdd card of measurement-2026-10-19-proc04-fdelete-test: same layout):
# open drive C by double-click on its HARD DISC icon (195,40), select DELME.TXT in the C:\ window (428,205), Show info
# (Ctrl+I, TERADESK.INF show=0x0249), Esc clears the name field, type "renamed.txt", Return = OK -> TeraDesk showinfo.c
# frename -> x_rename -> Frename(0, C:\DELME.TXT, C:\RENAMED.TXT) (same directory).
at 300
park
move 195 40
dclick
wait 400
mark t1-window-open
move 428 205
click                # DELME.TXT
wait 100
mark t2-selected
ctrl 0x17            # Ctrl+I = Show info
wait 300
mark t3-info
key 0x01             # Esc: clear the name field
wait 50
text renamed.txt
wait 50
mark t4-typed
key 0x1C             # Return = OK
wait 600
mark t5-renamed
key 0x1C             # a following dialog (next item / close), harmless otherwise
wait 400
mark t6-after
wait 200
