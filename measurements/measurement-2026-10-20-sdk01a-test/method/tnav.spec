# TNAV -- TeraDesk (#Z), armed up:DESKTOP, card td (tcp layout): open drive C (dclick HARD DISC icon (195,40)); the C:\ window (icon view) shows
# APPS (347,205) FOLD (427,205) SUB (507,205) BIG.TXT (587,205). TeraDesk navigates a folder IN the window (title C:\SUB\), the close box of
# the window is at (313,153). Navigation: dclick SUB, close; reopen C, dclick FOLD, dclick INNER (first icon of FOLD), close;
# launch: F1 = C:\APPS\QED\QED.NAP (TERADESK-QED.INF), QED up, Ctrl+Q ends QED back to TeraDesk; quit: Ctrl+Q in TeraDesk.
at 300
park
move 195 40
dclick
wait 400
mark t1-window-open
move 507 205
dclick               # SUB folder: shown in the same window
wait 400
mark t2-sub-shown
move 313 153
click                # close box
wait 300
mark t3-closed
move 427 205
dclick               # FOLD
wait 400
mark t5-fold-shown
move 347 205
dclick               # INNER (first icon)
wait 400
mark t6-inner-shown
move 313 153
click
wait 300
mark t7-up-again
key 0x3B             # F1 -> QED
wait 1500
mark t8-qed-up
ctrl 0x10            # Ctrl+Q in QED
wait 800
mark t9-qed-quit
ctrl 0x10            # Ctrl+Q in TeraDesk -> Quit / Shutdown Options box
wait 800
mark t10-quit-box
move 320 312
click                # Quit button
wait 1000
mark t11-teradesk-quit
wait 400
