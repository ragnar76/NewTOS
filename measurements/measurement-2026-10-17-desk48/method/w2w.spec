# w2w.spec -- DESK48 place 4: fun_win2win copies with fun_rebld(path). Layout of NEWDESK.INF (cal-1-desk.png):
# W1 C:\ (80..352, top 32), W2 C:\ (368..640), W3 C:\SUB\ (80..352, top 256, topped).
park
move 140 308
click            # NOTE row in W3 (top) -> selected
wait 100
mark a-selected
wait 200
press
drag 200 170     # into the empty area of W1 (another window on C:\ is W2)
release          # fun_win2win: copy C:\SUB\NOTE.TXT -> C:\
wait 200
mark b-dialog
wait 200
key 0x1C 0x9C    # Return = OK of the COPY FILE(S) box
wait 400
mark b-dropped
wait 600
mark c-dropped-late
move 200 42
click            # W1 title -> top W1
wait 100
move 140 138
click            # README row in W1 (4th row after the first copy added NOTE)
wait 300
press
drag 410 84      # onto the SUB folder row of W2
release          # fun_win2win into a folder: copy C:\README.TXT -> C:\SUB\ (W3 shows C:\SUB\)
wait 200
mark d-dialog
wait 200
key 0x1C 0x9C    # Return = OK
wait 400
mark d-folder
wait 600
mark e-folder-late
wait 100
