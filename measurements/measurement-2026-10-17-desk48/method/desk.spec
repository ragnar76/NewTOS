# desk.spec -- DESK48 place 1 (fun_desk2desk) and the extra fun_del_desktop place. Layout of NEWDESK.INF (cal-1-desk.png):
# desktop icons Hard disk (37,35), NOTE.TXT shortcut to C:\SUB\NOTE.TXT (37,128), Trash (37,215); W1/W2 C:\, W3 C:\SUB\ (topped).
park
move 37 128
click            # NOTE.TXT shortcut -> selected
wait 100
mark a-selected
wait 200
press
drag 37 35       # onto the Hard disk icon
release          # fun_desk2desk: copy C:\SUB\NOTE.TXT -> C:\
wait 200
mark b-dialog
wait 200
key 0x1C 0x9C    # Return = OK of the COPY FILE(S) box
wait 400
mark c-copied
wait 600
move 37 128
click            # the shortcut again
wait 200
key 0x1D 0x20 0xA0 0x9D   # Ctrl+D -> DELTITEM -> fun_del, desktop case
wait 200
mark d-alert
wait 400
move 318 268
click            # "Files" (wants_to_delete_files -> TRUE)
wait 200
mark e-confirm
wait 200
key 0x1C 0x9C    # Return = OK of the DELETE FILE(S) box -> deletes C:\SUB\NOTE.TXT
wait 400
mark f-deleted
wait 600
